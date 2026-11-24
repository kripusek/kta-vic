//! Draws on the host GPU (decision 0007, step 3). A list keeps the state
//! the title sets for its draws (pipeline, descriptor heaps, root
//! arguments, vertex and index buffers, targets); each draw then brings the
//! host list up to date: the host pipeline and our root signature, the
//! user data the translated shaders read, the host copies of the buffers
//! and descriptors they read, and the states of the textures involved.
//! State the host keeps across draws (viewports, scissors, blend factor,
//! stencil reference, topology, targets) is set on the host list as the
//! title sets it.

use std::sync::atomic::{AtomicU64, Ordering};

use victory_shader::{ResourceType, RootType, UsedResource};

use crate::descriptor::View;
use crate::descriptor::host::DynamicRange;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::core::Interface;

use crate::host::HostList;
use crate::object::This;
use crate::pipeline::host::HostPipeline;

/// A root argument as the title set it.
#[derive(Debug, Clone, Default)]
enum Root {
    #[default]
    None,
    /// A descriptor table's first descriptor (an address in a heap).
    Table(u64),
    Constants(Vec<u32>),
    /// A root view's GPU address.
    View(u64),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct VertexView {
    address: u64,
    size: u32,
    stride: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct IndexView {
    address: u64,
    size: u32,
    format: u32,
}

/// What one title list has set, and what its host list was last given.
#[cfg(feature = "diagnostics")]
#[derive(Clone, Copy)]
enum FrameOutcome {
    Ran,
    PreparationFailed(&'static str),
    Dropped(&'static str),
}

#[cfg(feature = "diagnostics")]
impl FrameOutcome {
    fn reason(self, unavailable: impl FnOnce() -> Option<String>) -> Option<String> {
        match self {
            Self::Ran => None,
            Self::PreparationFailed(reason) => unavailable().or_else(|| Some(reason.to_owned())),
            Self::Dropped(reason) => Some(reason.to_owned()),
        }
    }
}

#[derive(Default)]
pub(crate) struct State {
    pipeline: usize,
    /// View heap, sampler heap.
    heaps: [usize; 2],
    heaps_dirty: bool,
    graphics: Vec<Root>,
    compute: Vec<Root>,
    vertex: [VertexView; 32],
    index: IndexView,
    /// Bound render target and depth resources, for their host states.
    targets: Vec<(usize, Option<std::ops::Range<u32>>)>,
    depth: usize,
    /// The title's primitive topology (the host's may differ).
    topology: u32,
    /// What the host list has: pipeline, root signature, heaps.
    host_pipeline: usize,
    /// Graphics, compute.
    host_roots: [usize; 2],
    host_heaps: [usize; 2],
    /// Tables made afresh in this list: (host heap, table, what it holds).
    /// Allocations return to their original heaps on reset or release.
    fresh_tables: std::collections::HashMap<(usize, u64, u64), DynamicRange>,
}

/// Draws recorded on the host GPU since the last frame line.
static DRAWN: AtomicU64 = AtomicU64::new(0);

/// Draws run on the host so far.
pub(crate) fn drawn() -> u64 {
    DRAWN.load(Ordering::Relaxed)
}

/// The draws traced since the last present (diagnostics builds, with
/// debug logging for this module): logged when the frame comes out black.
#[cfg(feature = "diagnostics")]
static FRAME: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// The draws traced since the last call.
#[cfg(feature = "diagnostics")]
pub(crate) fn take_frame() -> Vec<String> {
    std::mem::take(&mut *FRAME.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Draws, mesh dispatches and dispatches recorded but not run on the host
/// (no host pipeline) since the last call.
pub(crate) fn take_dropped() -> u64 {
    DROPPED.swap(0, Ordering::Relaxed)
}
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Marks the last traced draw as not run on the host.
#[cfg(feature = "diagnostics")]
fn dropped(pipeline: usize) {
    crate::pipeline::note_drop(pipeline as This);
    if let Some(last) = FRAME.lock().unwrap_or_else(|e| e.into_inner()).last_mut() {
        last.push_str(" DROPPED");
    }
}

/// Dispatches run on the host so far.
static DISPATCHED: AtomicU64 = AtomicU64::new(0);

pub(crate) fn dispatched() -> u64 {
    DISPATCHED.load(Ordering::Relaxed)
}

/// The host address of a root view: its buffer's host copy, or for a
/// constant buffer not 256-byte aligned there (the console needs 16) a
/// copy in upload memory.
/// shortcut: a root view has no size; constant buffers are taken as 64 KiB,
/// other views as 1 MiB (both cut at their buffer's end).
fn root_view(list: &HostList, address: u64, srv: bool) -> Option<u64> {
    let shadow = crate::shadow::find(address, 1)?;
    let len = shadow
        .left(address)
        .min(if srv { 1 << 20 } else { 1 << 16 });
    let gpu = shadow.gpu(address);
    if srv || gpu % 256 == 0 {
        shadow.refresh(list, address, len);
        return Some(gpu);
    }
    let (gpu, to) = list.scratch(len)?;
    list.defer_fill(to, address as *const u8, len as usize);
    Some(gpu)
}

macro_rules! once {
    ($($arg:tt)*) => {{
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| tracing::warn!($($arg)*));
    }};
}

fn set(roots: &mut Vec<Root>, slot: u32, root: Root) {
    let slot = slot as usize;
    if roots.len() <= slot {
        roots.resize(slot + 1, Root::None);
    }
    roots[slot] = root;
}

impl State {
    #[cfg(test)]
    pub(crate) fn current_pipeline(&self) -> This {
        self.pipeline as This
    }

    pub(crate) fn reset(&mut self, pipeline: This) {
        *self = State::default();
        self.pipeline = pipeline as usize;
    }

    /// The title's topology; console-only ones become host lists (see
    /// [`crate::primitive`]).
    pub(crate) fn set_topology(&mut self, list: &HostList, topology: u32) {
        self.topology = topology;
        let host = match topology {
            RECTANGLES => 4,
            TRIANGLE_FAN | LINE_LOOP | QUADS | QUAD_STRIP => {
                once!(topology, "console topology drawn as a triangle list");
                4
            }
            t => t,
        };
        // SAFETY: an open list.
        unsafe {
            list.list
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY(host as i32))
        };
    }

    pub(crate) fn set_pipeline(&mut self, pipeline: This) {
        self.pipeline = pipeline as usize;
    }

    pub(crate) fn set_heaps(&mut self, heaps: &[This]) {
        self.heaps = [0; 2];
        for &h in heaps {
            if !h.is_null() {
                let samplers = crate::descriptor::heap(h).samplers();
                self.heaps[usize::from(samplers)] = h as usize;
            }
        }
        self.heaps_dirty = true;
    }

    fn roots(&mut self, compute: bool) -> &mut Vec<Root> {
        if compute {
            &mut self.compute
        } else {
            &mut self.graphics
        }
    }

    pub(crate) fn set_table(&mut self, compute: bool, slot: u32, handle: u64) {
        set(self.roots(compute), slot, Root::Table(handle));
    }

    pub(crate) fn set_view(&mut self, compute: bool, slot: u32, address: u64) {
        set(self.roots(compute), slot, Root::View(address));
    }

    pub(crate) fn set_constants(&mut self, compute: bool, slot: u32, values: &[u32], offset: u32) {
        let roots = self.roots(compute);
        let s = slot as usize;
        if !matches!(roots.get(s), Some(Root::Constants(_))) {
            set(roots, slot, Root::Constants(Vec::new()));
        }
        if let Root::Constants(c) = &mut roots[s] {
            let end = offset as usize + values.len();
            if c.len() < end {
                c.resize(end, 0);
            }
            c[offset as usize..end].copy_from_slice(values);
        }
    }

    pub(crate) fn set_vertex_buffers(&mut self, start: u32, views: &[VertexView]) {
        for (i, v) in views.iter().enumerate() {
            if let Some(slot) = self.vertex.get_mut(start as usize + i) {
                *slot = *v;
            }
        }
    }

    pub(crate) fn clear_vertex_buffers(&mut self, start: u32, count: u32) {
        for slot in self.vertex.iter_mut().skip(start as usize).take(count as usize) {
            *slot = VertexView::default();
        }
    }

    pub(crate) fn set_index_buffer(&mut self, view: Option<&IndexView>) {
        self.index = view.copied().unwrap_or_default();
    }

    /// Render targets and depth, set on the host list at once.
    pub(crate) fn set_targets(&mut self, list: &HostList, handles: &[u64], depth: u64) {
        let Some(host) = crate::host::host() else {
            return;
        };
        self.targets.clear();
        let mut rtvs = Vec::new();
        for &h in handles {
            let Some(record) = crate::descriptor::rtv_record(h) else {
                once!(
                    handle = format_args!("{h:#x}"),
                    "render target without a recorded view"
                );
                continue;
            };
            if record.resource.is_null() {
                // A typed null RTV discards writes without changing the output slot.
                if let Some(desc) = record.desc() {
                    rtvs.push(host.rtv(None, Some(&desc)));
                }
                continue;
            }
            let Some(texture) = crate::resource::host_resource(record.resource) else {
                continue;
            };
            let desc = record.desc();
            self.targets.push((
                record.resource as usize,
                crate::descriptor::rtv_mips(texture, desc.as_ref()),
            ));
            #[cfg(feature = "diagnostics")]
            crate::capture::note_target(record.resource);
            rtvs.push(host.rtv(Some(texture), desc.as_ref()));
        }
        self.depth = 0;
        let dsv = crate::descriptor::dsv_record(depth).and_then(|record| {
            let texture = crate::resource::host_resource(record.resource)?;
            self.depth = record.resource as usize;
            #[cfg(feature = "diagnostics")]
            crate::capture::note_target(record.resource);
            Some(host.dsv(texture, record.depth_desc().as_ref()))
        });
        // SAFETY: an open list; the views were just made.
        unsafe {
            list.list.OMSetRenderTargets(
                rtvs.len() as u32,
                (!rtvs.is_empty()).then_some(rtvs.as_ptr()),
                false,
                dsv.as_ref().map(std::ptr::from_ref),
            )
        };
    }

    /// The user-data value of a root argument, as translated shaders read
    /// it: a table's byte offset in its heap, a constant's value.
    fn user_value(&self, compute: bool, slot: u8, root: RootType, dword: u32) -> u32 {
        match (self.roots_ref(compute).get(usize::from(slot)), root) {
            (Some(Root::Table(handle)), RootType::Table) => match self.heap_of(*handle) {
                Some(heap) => (handle - heap.start) as u32,
                None => {
                    once!(
                        handle = format_args!("{handle:#x}"),
                        "descriptor table outside the bound heaps"
                    );
                    0
                }
            },
            (Some(Root::Constants(c)), RootType::Constants) => {
                c.get(dword as usize).copied().unwrap_or(0)
            }
            _ => {
                once!(
                    slot,
                    ?root,
                    "a shader reads a root argument the title did not set"
                );
                0
            }
        }
    }

    /// The bound heap holding `handle`.
    fn heap_of(&self, handle: u64) -> Option<&'static crate::descriptor::Heap> {
        self.heaps
            .iter()
            .filter(|&&h| h != 0)
            .map(|&h| crate::descriptor::heap(h as This))
            .find(|h| h.index(handle).is_some())
    }

    /// Brings the host list up to date for a draw; false if the draw cannot
    /// run on the host.
    fn prepare(&mut self, list: &HostList, indexed: bool) -> bool {
        self.prepare_for(list, indexed, false)
    }

    /// As [`State::prepare`], for draws (`mesh` false) or mesh dispatches.
    fn prepare_for(&mut self, list: &HostList, indexed: bool, mesh: bool) -> bool {
        let Some(p) = crate::pipeline::host_pipeline(self.pipeline as This) else {
            return false;
        };
        if p.compute || p.mesh != mesh {
            once!(mesh, "a draw or mesh dispatch with another kind of pipeline");
            return false;
        }
        if !self.bind(list, p, false) {
            return false;
        }
        let l = &list.list;
        // Vertex buffers the input layout reads.
        let mut views = [D3D12_VERTEX_BUFFER_VIEW::default(); 32];
        let mut count = 0;
        for (slot, (view, &v)) in views.iter_mut().zip(&self.vertex).enumerate() {
            if p.slots & 1 << slot == 0 {
                continue;
            }
            count = slot + 1;
            if v.address == 0 || v.size == 0 {
                continue;
            }
            let Some(shadow) = crate::shadow::find(v.address, u64::from(v.size)) else {
                continue;
            };
            shadow.refresh(list, v.address, u64::from(v.size));
            *view = D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: shadow.gpu(v.address),
                SizeInBytes: v.size,
                StrideInBytes: v.stride,
            };
        }
        // SAFETY: an open list; the views name live host buffers.
        unsafe {
            if count > 0 {
                l.IASetVertexBuffers(0, Some(&views[..count]));
            }
        }
        if indexed {
            let v = self.index;
            let Some(shadow) = crate::shadow::find(v.address, u64::from(v.size)) else {
                return false;
            };
            shadow.refresh(list, v.address, u64::from(v.size));
            let view = D3D12_INDEX_BUFFER_VIEW {
                BufferLocation: shadow.gpu(v.address),
                SizeInBytes: v.size,
                Format: DXGI_FORMAT(v.format as i32),
            };
            // SAFETY: an open list; the view names a live host buffer.
            unsafe { l.IASetIndexBuffer(Some(&view)) };
        }
        const RENDER_TARGET: u32 = 0x4;
        const DEPTH_WRITE: u32 = 0x10;
        for (resource, mips) in &self.targets {
            crate::list::ensure_mips(list, *resource as This, RENDER_TARGET, mips.clone());
        }
        if self.depth != 0 {
            crate::list::ensure_state(list, self.depth as This, DEPTH_WRITE);
        }
        true
    }

    /// A descriptor a shader reads through a table: its host slot made if
    /// it changed, constant data copied, textures made readable.
    /// What draws and dispatches share: heaps, our root signature, the
    /// pipeline, and per stage the user data, root views and the
    /// descriptors its tables hold.
    fn bind(&mut self, list: &HostList, p: &HostPipeline, compute: bool) -> bool {
        let l = &list.list;
        // Heaps first: a root signature with directly indexed heaps must be
        // set after them.
        if self.heaps_dirty {
            let heaps = self.heaps.map(|h| {
                (h != 0)
                    .then(|| crate::descriptor::heap(h as This).host())
                    .flatten()
            });
            let native = heaps.map(|h| h.map_or(0, |heap| heap.heap.as_raw() as usize));
            if self.host_heaps != native {
                list.bind_heaps(heaps);
                self.host_heaps = native;
                self.host_roots = [0; 2];
            }
            self.heaps_dirty = false;
        }
        let root = p.root.as_raw() as usize;
        if self.host_roots[usize::from(compute)] != root {
            // SAFETY: an open list and a live root signature.
            unsafe {
                if compute {
                    l.SetComputeRootSignature(&p.root);
                } else {
                    l.SetGraphicsRootSignature(&p.root);
                }
            }
            self.host_roots[usize::from(compute)] = root;
        }
        // By host object: a title pipeline released and another made at its
        // address is another host pipeline (the list keeps the old one).
        if self.host_pipeline != p.pso.as_raw() as usize {
            // SAFETY: an open list and a live pipeline.
            unsafe { l.SetPipelineState(&p.pso) };
            self.host_pipeline = p.pso.as_raw() as usize;
            list.used_pipeline(&p.name);
            // The title may release the pipeline before it submits the list.
            list.keep(&p.pso);
            list.keep(&p.root);
        }
        for stage in &p.stages {
            let t = stage.translated();
            let Some(made) = self.tables(list, &t.used, compute) else {
                return false;
            };
            if let Some((param, dwords)) = stage.user_data {
                let mut values = vec![0u32; dwords as usize];
                for u in &t.host.user_data {
                    let fresh = made
                        .iter()
                        .find(|m| m.0 == u.slot && u.root == RootType::Table);
                    values[u.index as usize] = match fresh {
                        Some(&(_, offset)) => offset,
                        None => self.user_value(compute, u.slot, u.root, u.dword),
                    };
                }
                // SAFETY: an open list; `values` covers the parameter.
                unsafe {
                    if compute {
                        l.SetComputeRoot32BitConstants(param, dwords, values.as_ptr().cast(), 0);
                    } else {
                        l.SetGraphicsRoot32BitConstants(param, dwords, values.as_ptr().cast(), 0);
                    }
                }
            }
            for (views, srv) in [(&stage.root_cbvs, false), (&stage.root_srvs, true)] {
                for &(slot, param) in views {
                    let Some(Root::View(address)) = self.roots_ref(compute).get(usize::from(slot))
                    else {
                        once!(slot, srv, "a shader reads a root view the title did not set");
                        return false;
                    };
                    let Some(gpu) = root_view(list, *address, srv) else {
                        once!(
                            slot,
                            srv,
                            address = format_args!("{address:#x}"),
                            "a shader reads a root view outside title buffers"
                        );
                        return false;
                    };
                    // SAFETY: an open list; the address is in live memory.
                    unsafe {
                        match (compute, srv) {
                            (false, false) => l.SetGraphicsRootConstantBufferView(param, gpu),
                            (false, true) => l.SetGraphicsRootShaderResourceView(param, gpu),
                            (true, false) => l.SetComputeRootConstantBufferView(param, gpu),
                            (true, true) => l.SetComputeRootShaderResourceView(param, gpu),
                        }
                    }
                }
            }
        }
        true
    }

    /// Runs a dispatch on the host.
    pub(crate) fn dispatch(&mut self, list: &HostList, x: u32, y: u32, z: u32) {
        let Some(p) = crate::pipeline::host_pipeline(self.pipeline as This) else {
            once!(
                pipeline = format_args!("{:#x}", self.pipeline),
                "a dispatch without a host pipeline dropped"
            );
            DROPPED.fetch_add(1, Ordering::Relaxed);
            #[cfg(feature = "diagnostics")]
            {
                self.frame_op(
                    format_args!("dispatch {x}x{y}x{z}"),
                    true,
                    FrameOutcome::PreparationFailed("no host pipeline"),
                );
                crate::pipeline::note_drop(self.pipeline as This);
            }
            return;
        };
        if !p.compute {
            once!("a dispatch with a graphics pipeline");
            #[cfg(feature = "diagnostics")]
            self.frame_op(
                format_args!("dispatch {x}x{y}x{z}"),
                true,
                FrameOutcome::Dropped("graphics pipeline bound to dispatch"),
            );
            return;
        }
        if !self.bind(list, p, true) {
            #[cfg(feature = "diagnostics")]
            self.frame_op(
                format_args!("dispatch {x}x{y}x{z}"),
                true,
                FrameOutcome::Dropped("compute binding failed"),
            );
            DROPPED.fetch_add(1, Ordering::Relaxed);
            return;
        }
        #[cfg(feature = "diagnostics")]
        list.note_op(|| self.op_text(format!("dispatch {x}x{y}x{z}"), true));
        // SAFETY: an open list with everything set.
        unsafe { list.list.Dispatch(x, y, z) };
        DISPATCHED.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "diagnostics")]
        self.frame_op(
            format_args!("dispatch {x}x{y}x{z}"),
            true,
            FrameOutcome::Ran,
        );
    }

    #[cfg(feature = "diagnostics")]
    fn frame_op(&self, kind: std::fmt::Arguments<'_>, compute: bool, outcome: FrameOutcome) {
        let Some(frame) = crate::capture::frame_op() else {
            return;
        };
        let reason = outcome.reason(|| crate::pipeline::unavailable_reason(self.pipeline as This));
        let color = if compute { &[][..] } else { &self.targets };
        let depth = if compute { 0 } else { self.depth };
        let color = color
            .iter()
            .map(|(resource, _)| format!("{resource:#x}"))
            .collect::<Vec<_>>()
            .join(",");
        tracing::info!(
            frame,
            operation = %kind,
            pipeline = format_args!("{:#x}", self.pipeline),
            topology = self.topology,
            shaders = %crate::pipeline::captured_shaders(self.pipeline as This),
            color = %color,
            depth = format_args!("{depth:#x}"),
            ran = reason.is_none(),
            drop_reason = reason.as_deref(),
            "captured frame operation"
        );
    }

    /// What a draw or dispatch runs, for the device-removed report
    /// (diagnostics): the pipeline, then per stage the user data it gets
    /// (with the view at that heap index, if one is), the host addresses
    /// of its root views and the views its tables name.
    #[cfg(feature = "diagnostics")]
    fn op_text(&self, kind: String, compute: bool) -> String {
        use std::fmt::Write;
        let Some(p) = crate::pipeline::host_pipeline(self.pipeline as This) else {
            return kind;
        };
        let mut text = format!("{kind} {}", p.name);
        let views = (self.heaps[0] != 0)
            .then(|| crate::descriptor::heap(self.heaps[0] as This).host())
            .flatten();
        for stage in &p.stages {
            let t = stage.translated();
            let _ = write!(text, " | {}:", stage.shader.hash);
            for u in &t.host.user_data {
                let v = self.user_value(compute, u.slot, u.root, u.dword);
                let _ = write!(text, " u{}={v:#x}", u.index);
                let index = if u.root == RootType::Table { v / 32 } else { v };
                if let Some(slot) = views.and_then(|h| h.describe_slot(index)) {
                    let _ = write!(text, " [heap {index}: {slot}]");
                }
            }
            for &(slot, _) in stage.root_cbvs.iter().chain(&stage.root_srvs) {
                if let Some(Root::View(address)) = self.roots_ref(compute).get(usize::from(slot)) {
                    let host = crate::shadow::find(*address, 1)
                        .map(|s| (s.gpu(*address), s.left(*address)));
                    let _ = write!(text, " root{slot}={address:#x} host {host:#x?}");
                }
            }
        }
        let _ = write!(text, " tables {:?}", self.reads(compute));
        text
    }

    /// Runs a mesh dispatch of `x` groups on the host (the title's mesh
    /// programs are one-dimensional).
    pub(crate) fn dispatch_mesh(&mut self, list: &HostList, x: u32) {
        #[cfg(feature = "diagnostics")]
        self.trace("mesh", x, 0);
        let ran = self.prepare_for(list, false, true)
            && match list.list.cast::<ID3D12GraphicsCommandList6>() {
                Ok(l6) => {
                    #[cfg(feature = "diagnostics")]
                    list.note_op(|| self.op_text(format!("mesh {x}"), false));
                    // SAFETY: an open list with everything set.
                    unsafe { l6.DispatchMesh(x, 1, 1) };
                    DRAWN.fetch_add(1, Ordering::Relaxed);
                    true
                }
                Err(_) => false,
            };
        #[cfg(feature = "diagnostics")]
        self.frame_op(
            format_args!("mesh {x}"),
            false,
            if ran {
                FrameOutcome::Ran
            } else {
                FrameOutcome::PreparationFailed("mesh preparation or command-list interface failed")
            },
        );
        if !ran {
            DROPPED.fetch_add(1, Ordering::Relaxed);
            #[cfg(feature = "diagnostics")]
            dropped(self.pipeline);
        }
    }

    fn roots_ref(&self, compute: bool) -> &Vec<Root> {
        if compute {
            &self.compute
        } else {
            &self.graphics
        }
    }

    fn table_entry(&self, list: &HostList, u: &UsedResource, compute: bool) {
        let Some(Root::Table(table)) = self.roots_ref(compute).get(usize::from(u.slot)) else {
            return;
        };
        let handle = table + u64::from(u.table_offset_dwords) * 4;
        let Some(heap) = self.heap_of(handle) else {
            return;
        };
        let (Some(host), Some(index)) = (heap.host(), heap.index(handle)) else {
            return;
        };
        if let Some(view) = host.mirror(index, handle, true) {
            readable(list, u, &view);
        }
    }

    /// Gives tables with shader resources or constants slots owned by the
    /// command list. Later title descriptor writes cannot change what a
    /// recorded draw samples. Constants also get 256-byte-aligned storage,
    /// filled at submission. Identical descriptors share slots within a list.
    fn tables(&mut self, list: &HostList, used: &[UsedResource], compute: bool) -> Option<Vec<(u8, u32)>> {
        let tables = |u: &&UsedResource| u.root == RootType::Table;
        let mut fresh: Vec<u8> = used
            .iter()
            .filter(tables)
            .filter(|u| matches!(u.kind, ResourceType::Cbv | ResourceType::Srv))
            .map(|u| u.slot)
            .collect();
        // Tables reaching past a host heap smaller than the title's (the
        // host's limit, 1,000,000 views on NVIDIA) are made afresh too: the
        // title's slots there have no host slot of their own.
        for u in used.iter().filter(tables) {
            if fresh.contains(&u.slot) {
                continue;
            }
            let Some(&Root::Table(table)) = self.roots_ref(compute).get(usize::from(u.slot)) else {
                continue;
            };
            let end = table + u64::from(u.table_offset_dwords) * 4;
            let Some(heap) = self.heap_of(table) else {
                continue;
            };
            let (Some(host), Some(index)) = (heap.host(), heap.index(end)) else {
                continue;
            };
            if index >= host.count() {
                fresh.push(u.slot);
            }
        }
        fresh.sort_unstable();
        fresh.dedup();
        for u in used.iter().filter(tables) {
            if !fresh.contains(&u.slot) {
                self.table_entry(list, u, compute);
            }
        }
        let mut made = Vec::new();
        for slot in fresh {
            let Some(&Root::Table(table)) = self.roots_ref(compute).get(usize::from(slot)) else {
                continue;
            };
            let entries: Vec<&UsedResource> = used
                .iter()
                .filter(tables)
                .filter(|u| u.slot == slot)
                .collect();
            let Some(heap) = self.heap_of(table) else {
                continue;
            };
            let Some(host) = heap.host() else {
                continue;
            };
            let n = entries
                .iter()
                .map(|u| u.table_offset_dwords / 8 + 1)
                .max()
                .unwrap_or(1);
            let last = table + u64::from(n - 1) * 32;
            if heap.index(last).is_none() {
                for u in entries {
                    self.table_entry(list, u, compute);
                }
                continue;
            }
            // The table and what its entries hold (record words).
            let key = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                for k in 0..n {
                    let at = table + u64::from(k) * 32;
                    // SAFETY: a descriptor slot in title memory.
                    unsafe { (at as *const [u32; 3]).read_unaligned() }.hash(&mut h);
                }
                (host.heap.as_raw() as usize, table, h.finish())
            };
            if let Some(range) = self.fresh_tables.get(&key) {
                for u in entries.iter().filter(|u| u.kind != ResourceType::Cbv) {
                    let handle = table + u64::from(u.table_offset_dwords) * 4;
                    if let Some(view) = crate::descriptor::view(handle) {
                        readable(list, u, &view);
                    }
                }
                made.push((slot, range.first * 32));
                continue;
            }
            let range = host.reserve_dynamic(n)?;
            let first = range.first;
            self.fresh_tables.insert(key, range);
            // The whole range: a shader may index past the entries its
            // used-resource list names (descriptor arrays).
            for k in 0..n {
                let index = first + k;
                let used = entries.iter().find(|u| u.table_offset_dwords / 8 == k);
                match crate::descriptor::view(table + u64::from(k) * 32) {
                    Some(View::Constant(address, size)) => {
                        let Some((gpu, to)) = list.scratch(u64::from(size)) else {
                            host.make_null_kind(index, ResourceType::Cbv, 4);
                            continue;
                        };
                        list.defer_fill(to, address as *const u8, size as usize);
                        host.constant_at(index, gpu, size);
                    }
                    Some(view) => {
                        host.make_at(index, &view);
                        if let Some(u) = used {
                            readable(list, u, &view);
                        }
                    }
                    None => {
                        if let Some(u) = used {
                            host.make_null_kind(index, u.kind, u.dwords);
                            once!(slot, "a table entry without a view");
                        } else {
                            host.make_null(index);
                        }
                    }
                }
            }
            made.push((slot, first * 32));
        }
        Some(made)
    }

    /// The first draws, at debug level: what they bind and read
    /// (diagnostics builds).
    #[cfg(feature = "diagnostics")]
    fn trace(&self, kind: &str, count: u32, first: u32) {
        static TRACED: AtomicU64 = AtomicU64::new(0);
        if !tracing::enabled!(tracing::Level::DEBUG) {
            return;
        }
        let listed = crate::capture::listed(crate::queue::presented() + 1);
        let logged = listed || TRACED.fetch_add(1, Ordering::Relaxed) < 64;
        let targets: Vec<String> = self
            .targets
            .iter()
            .filter_map(|&(resource, _)| crate::resource::host_resource(resource as This))
            .map(|t| {
                // SAFETY: a live resource.
                let d = unsafe { t.GetDesc() };
                format!("{}x{} f{}", d.Width, d.Height, d.Format.0)
            })
            .collect();
        let v = self.vertex[0];
        let line = format!(
            "{kind} {count} from {first} topology {} pipeline {:#x} targets {targets:?} vb {:#x}+{} stride {} reads {:?}",
            self.topology,
            self.pipeline,
            v.address,
            v.size,
            v.stride,
            self.reads(false)
        );
        if logged {
            tracing::debug!("draw {line}");
        }
        let mut frame = FRAME.lock().unwrap_or_else(|e| e.into_inner());
        if frame.len() < 256 {
            frame.push(line);
        }
    }

    /// What a draw's shaders read, one line per used resource (diagnostics
    /// builds).
    #[cfg(feature = "diagnostics")]
    fn reads(&self, compute: bool) -> Vec<String> {
        let Some(p) = crate::pipeline::host_pipeline(self.pipeline as This) else {
            return vec!["no host pipeline".into()];
        };
        let mut out = Vec::new();
        for stage in &p.stages {
            let t = stage.translated();
            for u in &t.used {
                let what = match (u.root, self.roots_ref(compute).get(usize::from(u.slot))) {
                    (RootType::Table, Some(Root::Table(table))) => {
                        let handle = table + u64::from(u.table_offset_dwords) * 4;
                        describe(crate::descriptor::view(handle).as_ref())
                    }
                    (root, _) => format!("{root:?}"),
                };
                out.push(format!(
                    "{} {:?} s{}+{}: {what}",
                    stage.shader.hash, u.kind, u.slot, u.table_offset_dwords
                ));
            }
        }
        out
    }

    pub(crate) fn draw(
        &mut self,
        list: &HostList,
        vertices: u32,
        instances: u32,
        first: u32,
        first_instance: u32,
    ) {
        #[cfg(feature = "diagnostics")]
        self.trace("draw", vertices, first);
        let rects = self.topology == RECTANGLES;
        let ran = self.prepare(list, false) && (!rects || self.rectangles(list, vertices, first));
        if ran {
            // Rectangles were expanded into their own vertex buffers.
            let (count, from) = if rects {
                (vertices / 3 * 6, 0)
            } else {
                (vertices, first)
            };
            #[cfg(feature = "diagnostics")]
            list.note_op(|| self.op_text(format!("draw {count}x{instances}"), false));
            // SAFETY: an open list with everything set.
            unsafe {
                list.list
                    .DrawInstanced(count, instances, from, first_instance)
            };
            DRAWN.fetch_add(1, Ordering::Relaxed);
        }
        #[cfg(feature = "diagnostics")]
        self.frame_op(
            format_args!("draw {vertices}x{instances} from {first}"),
            false,
            if ran {
                FrameOutcome::Ran
            } else {
                FrameOutcome::PreparationFailed("draw preparation or rectangle expansion failed")
            },
        );
        if !ran {
            DROPPED.fetch_add(1, Ordering::Relaxed);
            #[cfg(feature = "diagnostics")]
            dropped(self.pipeline);
        }
    }

    /// Binds vertex buffers holding the rectangles of vertices `first..first
    /// + count` as triangles, made when the list is submitted.
    fn rectangles(&self, list: &HostList, count: u32, first: u32) -> bool {
        let Some(p) = crate::pipeline::host_pipeline(self.pipeline as This) else {
            return false;
        };
        let rects = u64::from(count / 3);
        for slot in 0..32 {
            let elements: Vec<(u32, u32)> = p
                .inputs
                .iter()
                .filter(|e| e.slot == slot && !e.per_instance)
                .map(|e| (e.offset, e.format))
                .collect();
            if elements.is_empty() {
                continue;
            }
            let v = self.vertex[slot as usize];
            let stride = u64::from(v.stride);
            let from = v.address + u64::from(first) * stride;
            if v.address == 0
                || stride == 0
                || u64::from(first + count) * stride > u64::from(v.size)
                || crate::shadow::find(from, rects * 3 * stride).is_none()
            {
                once!(slot, "rectangle vertices outside their buffer");
                return false;
            }
            if !crate::primitive::rectangle_elements_fit(v.stride, &elements) {
                once!(slot, "rectangle vertex elements exceed their stride");
                return false;
            }
            let Some((gpu, to)) = list.scratch(rects * 6 * stride) else {
                return false;
            };
            let (from, to, stride) = (from as usize, to as usize, stride as usize);
            let n = rects as usize;
            list.defer_host(move || {
                // SAFETY: the vertices lie in a title buffer (checked when
                // recorded) and the scratch memory is the list's.
                let (src, dst) = unsafe {
                    (
                        std::slice::from_raw_parts(from as *const u8, n * 3 * stride),
                        std::slice::from_raw_parts_mut(to as *mut u8, n * 6 * stride),
                    )
                };
                crate::primitive::expand_rects(src, dst, stride, &elements);
            });
            let view = D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: gpu,
                SizeInBytes: (rects * 6 * stride as u64) as u32,
                StrideInBytes: v.stride,
            };
            // SAFETY: an open list; the view names live scratch memory.
            unsafe { list.list.IASetVertexBuffers(slot, Some(&[view])) };
        }
        true
    }

    pub(crate) fn draw_indexed(
        &mut self,
        list: &HostList,
        indices: u32,
        instances: u32,
        first: u32,
        base: i32,
        first_instance: u32,
    ) {
        #[cfg(feature = "diagnostics")]
        self.trace("indexed", indices, first);
        if self.topology == RECTANGLES {
            once!("indexed rectangles not drawn");
        }
        let ran = self.topology != RECTANGLES && self.prepare(list, true);
        if ran {
            #[cfg(feature = "diagnostics")]
            list.note_op(|| self.op_text(format!("indexed {indices}x{instances}"), false));
            // SAFETY: an open list with everything set.
            unsafe {
                list.list
                    .DrawIndexedInstanced(indices, instances, first, base, first_instance)
            };
            DRAWN.fetch_add(1, Ordering::Relaxed);
        }
        #[cfg(feature = "diagnostics")]
        self.frame_op(
            format_args!("indexed {indices}x{instances} from {first}"),
            false,
            if ran {
                FrameOutcome::Ran
            } else {
                FrameOutcome::PreparationFailed(if self.topology == RECTANGLES {
                    "indexed rectangles unsupported"
                } else {
                    "indexed draw preparation failed"
                })
            },
        );
        if !ran {
            DROPPED.fetch_add(1, Ordering::Relaxed);
            #[cfg(feature = "diagnostics")]
            dropped(self.pipeline);
        }
    }
}

impl State {
    /// Indirect execution of a one-argument signature of `kind` (PC
    /// numbering: 0 draw, 1 indexed draw, 2 dispatch, 10 mesh dispatch) with
    /// records `stride` bytes apart at title address `arguments`, and the
    /// record count at `count` if the title gives one: the GPU reads them
    /// from the buffer's host copy, where earlier dispatches may have
    /// written them. False if it cannot run on the host.
    pub(crate) fn execute_indirect(
        &mut self,
        list: &HostList,
        (kind, stride, max): (u32, u32, u32),
        arguments: u64,
        count: Option<u64>,
    ) -> bool {
        const DRAW: u32 = 0;
        const INDEXED: u32 = 1;
        const DISPATCH: u32 = 2;
        const MESH: u32 = 10;
        #[cfg(feature = "diagnostics")]
        let why = |what: &str| {
            crate::pipeline::note_why(format!("indirect kind {kind}: {what}"));
        };
        #[cfg(feature = "diagnostics")]
        let trace = |state: &Self, failure| {
            state.frame_op(
                format_args!("indirect kind {kind} max {max} at {arguments:#x}"),
                kind == DISPATCH,
                failure,
            )
        };
        #[cfg(not(feature = "diagnostics"))]
        let why = |_: &str| {};
        let len = u64::from(stride) * u64::from(max.max(1));
        let Some(args) = crate::shadow::find(arguments, len) else {
            why("arguments outside title buffers");
            #[cfg(feature = "diagnostics")]
            trace(
                self,
                FrameOutcome::Dropped("arguments outside title buffers"),
            );
            return false;
        };
        let counter = match count {
            Some(at) => match crate::shadow::find(at, 4) {
                Some(s) => Some((s, at)),
                None => {
                    why("count outside title buffers");
                    #[cfg(feature = "diagnostics")]
                    trace(self, FrameOutcome::Dropped("count outside title buffers"));
                    return false;
                }
            },
            None => None,
        };
        let Some(signature) = matches!(kind, DRAW | INDEXED | DISPATCH | MESH)
            .then(|| host_signature(kind, stride))
            .flatten()
        else {
            why("no host signature");
            #[cfg(feature = "diagnostics")]
            trace(self, FrameOutcome::Dropped("no host signature"));
            return false;
        };
        args.refresh(list, arguments, len);
        if let Some((s, at)) = &counter {
            s.refresh(list, *at, 4);
        }
        // Bounded first: the clamp binds a compute pipeline of its own.
        if !crate::indirect::bound(list, &args.buffer, args.gpu(arguments), (kind, stride, max)) {
            why("not bounded (no clamp, or arguments not 4-byte aligned)");
            #[cfg(feature = "diagnostics")]
            trace(
                self,
                FrameOutcome::Dropped("not bounded (no clamp, or arguments not 4-byte aligned)"),
            );
            return false;
        }
        self.host_pipeline = 0;
        self.host_roots[1] = 0;
        let ran = match kind {
            DRAW | INDEXED => self.topology != RECTANGLES && self.prepare(list, kind == INDEXED),
            MESH => self.prepare_for(list, false, true),
            DISPATCH => match crate::pipeline::host_pipeline(self.pipeline as This) {
                Some(p) if p.compute => self.bind(list, p, true),
                _ => false,
            },
            _ => false,
        };
        if !ran {
            #[cfg(feature = "diagnostics")]
            trace(
                self,
                FrameOutcome::PreparationFailed("indirect preparation or binding failed"),
            );
            crate::indirect::done(list, &args.buffer);
            #[cfg(feature = "diagnostics")]
            match crate::pipeline::host_pipeline(self.pipeline as This) {
                None => crate::pipeline::note_drop(self.pipeline as This),
                Some(p) => why(&format!("prepare failed (compute {}, mesh {})", p.compute, p.mesh)),
            }
            return false;
        }
        #[cfg(feature = "diagnostics")]
        list.note_op(|| {
            self.op_text(format!("indirect kind {kind} max {max} at {arguments:#x}"), kind == DISPATCH)
        });
        #[cfg(feature = "diagnostics")]
        if kind == MESH && let Some((frame, sequence)) = crate::indirect::capture::claim() {
            use crate::indirect::capture::{Call, record};
            record(
                list,
                Call {
                    frame,
                    sequence,
                    pipeline: self.pipeline,
                    shaders: crate::pipeline::captured_shaders(self.pipeline as This),
                    arguments,
                    max,
                    stride,
                },
                (&args.buffer, args.offset(arguments)),
                counter.as_ref().map(|(s, at)| (&s.buffer, s.offset(*at))),
            );
        }
        // SAFETY: an open list with everything set; the buffers are host
        // copies the list keeps alive (refresh), the signature is cached.
        unsafe {
            list.list.ExecuteIndirect(
                &signature,
                max,
                &args.buffer,
                args.offset(arguments),
                counter.as_ref().map(|(s, _)| &s.buffer),
                counter.as_ref().map_or(0, |(s, at)| s.offset(*at)),
            )
        };
        crate::indirect::done(list, &args.buffer);
        if kind == DISPATCH {
            DISPATCHED.fetch_add(1, Ordering::Relaxed);
        } else {
            DRAWN.fetch_add(1, Ordering::Relaxed);
        }
        #[cfg(feature = "diagnostics")]
        trace(self, FrameOutcome::Ran);
        true
    }
}

/// The host command signature for one argument of `kind` with records
/// `stride` bytes apart (no root arguments, so no root signature), made
/// once.
fn host_signature(kind: u32, stride: u32) -> Option<ID3D12CommandSignature> {
    static MADE: std::sync::Mutex<Vec<((u32, u32), ID3D12CommandSignature)>> =
        std::sync::Mutex::new(Vec::new());
    let mut made = MADE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, s)) = made.iter().find(|(k, _)| *k == (kind, stride)) {
        return Some(s.clone());
    }
    let argument = D3D12_INDIRECT_ARGUMENT_DESC {
        Type: D3D12_INDIRECT_ARGUMENT_TYPE(kind as i32),
        ..Default::default()
    };
    let desc = D3D12_COMMAND_SIGNATURE_DESC {
        ByteStride: stride,
        NumArgumentDescs: 1,
        pArgumentDescs: &argument,
        NodeMask: 0,
    };
    let mut signature: Option<ID3D12CommandSignature> = None;
    // SAFETY: a description of one argument; no root arguments.
    let result = unsafe {
        crate::host::host()?.device.CreateCommandSignature(
            &desc,
            None::<&ID3D12RootSignature>,
            &mut signature,
        )
    };
    if let Err(e) = result {
        once!(error = %e, kind, stride, "host command signature refused");
        return None;
    }
    let signature = signature?;
    made.push(((kind, stride), signature.clone()));
    Some(signature)
}

/// Gets what a shader reads or writes through a view ready: buffer data
/// copied, textures moved to the state they need.
fn readable(list: &HostList, u: &UsedResource, view: &View) {
    const SHADER_RESOURCE: u32 = 0x40 | 0x80;
    const UNORDERED_ACCESS: u32 = 0x8;
    let placed = crate::descriptor::host::placed_texture(view);
    let view = placed.as_ref().unwrap_or(view);
    // Buffer views read title memory: their range is copied at submission;
    // what shaders write through unordered access goes back to it after.
    if let Some((address, len)) = crate::descriptor::host::buffer_range(view)
        && len > 0
        && let Some(shadow) = crate::shadow::find(address, len)
    {
        shadow.refresh(list, address, len);
        if u.kind == ResourceType::Uav {
            shadow.gpu_writes(list, address, len);
        }
    }
    match (view, u.kind) {
        (View::Shader(resource, desc), ResourceType::Srv) => {
            let Some(t) = crate::resource::host_resource(*resource as This) else {
                return;
            };
            if crate::texture::title_written(t) {
                crate::texture::upload(list, *resource as This, t);
            }
            let mips = desc.as_ref().and_then(|d| crate::descriptor::host::srv_mips(t, d));
            crate::list::ensure_mips(list, *resource as This, SHADER_RESOURCE, mips);
            #[cfg(feature = "diagnostics")]
            {
                // SAFETY: a live resource.
                let d = unsafe { t.GetDesc() };
                if d.Flags.0 & 7 == 0 {
                    tracing::debug!(
                        resource = format_args!("{resource:#x}"),
                        ?d,
                        "a shader reads a texture the title fills"
                    );
                }
            }
        }
        (View::Unordered(resource, _, _), ResourceType::Uav) => {
            #[cfg(feature = "diagnostics")]
            crate::capture::note_target(*resource as This);
            crate::list::ensure_state(list, *resource as This, UNORDERED_ACCESS);
        }
        _ => {}
    }
}

/// A view in a few words (diagnostics builds).
#[cfg(feature = "diagnostics")]
fn describe(view: Option<&View>) -> String {
    let texture = |r: This| {
        crate::resource::host_resource(r).map(|t| {
            // SAFETY: a live resource.
            let d = unsafe { t.GetDesc() };
            format!("{}x{} f{} flags {:#x}", d.Width, d.Height, d.Format.0, d.Flags.0)
        })
    };
    match view {
        None => "no view".into(),
        Some(v @ View::Shader(r, _)) => texture(*r as This)
            .unwrap_or_else(|| format!("buffer {:?}", crate::descriptor::host::buffer_range(v))),
        // A buffer has no host texture: its title range, as for a shader view.
        Some(v @ View::Unordered(r, ..)) => texture(*r as This).map_or_else(
            || format!("uav buffer {:?}", crate::descriptor::host::buffer_range(v)),
            |t| format!("uav {t}"),
        ),
        Some(View::Placed(u, r, d)) => format!(
            "placed uav={u} dim {} {}x{} f{} layout {:#x} at {:#x}",
            r[0],
            u64::from(r[4]) | u64::from(r[5]) << 32,
            r[6],
            r[8],
            r[11],
            u64::from(d[10]) | u64::from(d[11]) << 32
        ),
        // The first four values, as a colour rectangle's constants are.
        Some(View::Constant(address, size))
            if *size >= 16 && crate::texture::committed(*address as *const u8, 16) =>
        {
            // SAFETY: 16 committed bytes of title memory.
            let v = unsafe { (*address as *const [f32; 4]).read_unaligned() };
            format!("Constant({address:#x}, {size}) {v:?}")
        }
        Some(v) => format!("{v:?}").chars().take(60).collect(),
    }
}

/// The console's own primitive topologies.
const TRIANGLE_FAN: u32 = 6;
const RECTANGLES: u32 = 17;
const LINE_LOOP: u32 = 18;
const QUADS: u32 = 19;
const QUAD_STRIP: u32 = 20;

/// Fixed-function state, straight onto the host list.
pub(crate) fn viewports(list: &HostList, viewports: &[D3D12_VIEWPORT]) {
    // SAFETY: an open list.
    unsafe { list.list.RSSetViewports(viewports) };
}

pub(crate) fn scissors(list: &HostList, rects: &[RECT]) {
    // SAFETY: an open list.
    unsafe { list.list.RSSetScissorRects(rects) };
}

pub(crate) fn blend_factor(list: &HostList, factor: Option<&[f32; 4]>) {
    // SAFETY: an open list.
    unsafe { list.list.OMSetBlendFactor(factor) };
}

pub(crate) fn stencil_ref(list: &HostList, reference: u32) {
    // SAFETY: an open list.
    unsafe { list.list.OMSetStencilRef(reference) };
}

#[cfg(all(test, feature = "diagnostics"))]
mod tests {
    use super::*;

    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
    use windows::core::s;

    #[cfg(feature = "diagnostics")]
    #[test]
    fn frame_operation_reports_command_failures_before_pipeline_status() {
        let unavailable = || panic!("concrete command outcomes do not query the pipeline");
        assert_eq!(FrameOutcome::Ran.reason(unavailable), None);
        assert_eq!(
            FrameOutcome::Dropped("count outside title buffers")
                .reason(unavailable)
                .as_deref(),
            Some("count outside title buffers")
        );
        let preparation = FrameOutcome::PreparationFailed("draw preparation failed");
        assert_eq!(
            preparation
                .reason(|| Some("pipeline is still queued".to_owned()))
                .as_deref(),
            Some("pipeline is still queued")
        );
        assert_eq!(
            preparation.reason(|| None).as_deref(),
            Some("draw preparation failed")
        );
    }

    struct Texture(This);

    impl Drop for Texture {
        fn drop(&mut self) {
            let release = crate::object::slot::<extern "system" fn(This) -> u32>(self.0, 0x10);
            release(self.0);
        }
    }

    fn graphics_fixture(
        device: &ID3D12Device,
        source: &str,
        formats: &[DXGI_FORMAT],
    ) -> (ID3D12RootSignature, ID3D12PipelineState) {
        let compile = |entry, target| {
            let mut code = None;
            // SAFETY: source and shader names are live, and code is our output pointer.
            unsafe {
                D3DCompile(
                    source.as_ptr().cast(),
                    source.len(),
                    s!("rtv-slot-test"),
                    None,
                    None,
                    entry,
                    target,
                    0,
                    0,
                    &mut code,
                    None,
                )
                .expect("compile fixture shader");
            }
            code.expect("fixture bytecode")
        };
        let vs = compile(s!("vs"), s!("vs_5_1"));
        let ps = compile(s!("ps"), s!("ps_5_1"));
        // SAFETY: compiled fixture shaders and their embedded root signature are live.
        unsafe {
            let bytecode = |blob: &ID3DBlob| D3D12_SHADER_BYTECODE {
                pShaderBytecode: blob.GetBufferPointer(),
                BytecodeLength: blob.GetBufferSize(),
            };
            let code = bytecode(&vs);
            let root = device
                .CreateRootSignature(
                    0,
                    std::slice::from_raw_parts(code.pShaderBytecode.cast(), code.BytecodeLength),
                )
                .expect("graphics root signature");
            let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                VS: code,
                PS: bytecode(&ps),
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC {
                    FillMode: D3D12_FILL_MODE_SOLID,
                    CullMode: D3D12_CULL_MODE_NONE,
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: formats.len() as u32,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                ..Default::default()
            };
            desc.RTVFormats[..formats.len()].copy_from_slice(formats);
            desc.BlendState.RenderTarget[0].RenderTargetWriteMask =
                D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let pso = device
                .CreateGraphicsPipelineState(&desc)
                .expect("graphics pipeline");
            (root, pso)
        }
    }

    fn color_texture(side: u32, mips: u16) -> Texture {
        fixture_texture(
            side,
            mips,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        )
    }

    fn fixture_texture(
        side: u32,
        mips: u16,
        format: DXGI_FORMAT,
        flags: D3D12_RESOURCE_FLAGS,
        state: D3D12_RESOURCE_STATES,
    ) -> Texture {
        let heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            ..Default::default()
        };
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Width: u64::from(side),
            Height: side,
            DepthOrArraySize: 1,
            MipLevels: mips,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Flags: flags,
            ..Default::default()
        };
        let mut target = std::ptr::null();
        let null = std::ptr::null();
        let iid = ID3D12Resource::IID;
        assert_eq!(
            crate::resource::create_committed(
                null,
                (&raw const heap).cast(),
                0,
                (&raw const desc).cast(),
                state.0 as u32,
                null.cast(),
                (&raw const iid).cast(),
                &mut target,
            ),
            crate::object::S_OK
        );
        Texture(target)
    }

    fn assert_mip_zero_pixels(
        host: &crate::host::Host,
        list: &HostList,
        target: This,
        side: u32,
        expected: [u8; 4],
    ) {
        crate::list::ensure_state(list, target, D3D12_RESOURCE_STATE_COPY_SOURCE.0 as u32);
        let color = crate::resource::host_resource(target).expect("color target");
        let (readback, footprint) =
            crate::capture::read_back(&host.device, &list.list, color).expect("color readback");
        list.close();
        assert_eq!(host.execute_checked(&[list]), None);
        let mut mapped = std::ptr::null_mut();
        let mut pixels = Vec::new();
        // SAFETY: execution has completed and all pixels lie in the mapped footprint.
        unsafe {
            readback
                .Map(0, None, Some(&mut mapped))
                .expect("map readback");
            for y in 0..side {
                for x in 0..side {
                    let at = footprint.Offset as usize
                        + (y * footprint.Footprint.RowPitch + x * 4) as usize;
                    pixels.push(mapped.cast::<u8>().add(at).cast::<[u8; 4]>().read());
                }
            }
            readback.Unmap(0, None);
        }
        assert_eq!(pixels, vec![expected; (side * side) as usize]);
    }

    #[test]
    fn first_copy_uploads_guest_texels_and_preserves_the_gpu_copy() {
        let Some(host) = crate::host::host() else {
            assert!(std::env::var_os("VICTORY_D3D12_WARP").is_none());
            return;
        };
        let make_texture = || {
            fixture_texture(
                4,
                1,
                DXGI_FORMAT_R8G8B8A8_UNORM,
                D3D12_RESOURCE_FLAG_NONE,
                D3D12_RESOURCE_STATE_COMMON,
            )
        };
        let (source, middle, target) = (make_texture(), make_texture(), make_texture());
        let expected = [0x16u8, 0x5a, 0xae, 0xff];
        let (address, desc) = crate::resource::texture_memory(source.0).unwrap();
        let texture = crate::resource::texture_layout(&desc).unwrap();
        let layout = &texture.planes[0].layout;
        // SAFETY: source owns this writable allocation for its entire tiled footprint.
        let bytes =
            unsafe { std::slice::from_raw_parts_mut(address.cast_mut(), layout.size as usize) };
        layout
            .tile(bytes, 0, 0, &expected.repeat(16), 16, 64)
            .unwrap();

        let null = std::ptr::null();
        let mut allocator = null;
        let mut list = null;
        let alloc_iid = ID3D12CommandAllocator::IID;
        let list_iid = ID3D12GraphicsCommandList::IID;
        assert_eq!(
            crate::list::create_allocator(null, 0, (&raw const alloc_iid).cast(), &mut allocator,),
            crate::object::S_OK
        );
        assert_eq!(
            crate::list::create_list(
                null,
                0,
                0,
                allocator,
                null,
                (&raw const list_iid).cast(),
                &mut list,
            ),
            crate::object::S_OK
        );
        let copy = crate::object::slot::<extern "system" fn(This, This, This)>(list, 0x88);
        copy(list, middle.0, source.0);
        // The middle texture's guest backing is still zero. Its GPU copy owns the pixels.
        copy(list, target.0, middle.0);
        assert_mip_zero_pixels(
            host,
            crate::list::host_list(list).unwrap(),
            target.0,
            4,
            expected,
        );
        for object in [list, allocator] {
            crate::object::slot::<extern "system" fn(This) -> u32>(object, 0x10)(object);
        }
    }

    #[test]
    fn typed_null_rtv_keeps_the_next_color_slot() {
        let Some(host) = crate::host::host() else {
            return; // no host GPU here
        };
        let device = &host.device;
        let list = host.new_list().expect("host command list");
        let l = &list.list;
        let format = DXGI_FORMAT_R8G8B8A8_UNORM;
        let source = r#"
            [RootSignature("RootFlags(ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT)")]
            float4 vs(uint id : SV_VertexID) : SV_Position {
                return float4(id == 2 ? 3 : -1, id == 1 ? -3 : 1, 0, 1);
            }
            struct Colors { float4 red : SV_Target0; float4 green : SV_Target1; };
            Colors ps() {
                Colors c;
                c.red = float4(1, 0, 0, 1);
                c.green = float4(0, 1, 0, 1);
                return c;
            }
        "#;
        // D3D12 requires UNKNOWN in the PSO slot that binds a null RTV.
        let (root, pso) = graphics_fixture(device, source, &[DXGI_FORMAT(0), format]);

        const SIDE: u32 = 4;
        let fixture = color_texture(SIDE, 1);
        let target = fixture.0;
        let null = std::ptr::null();
        let color = crate::resource::host_resource(target).expect("color target");
        let mut slots = [[0u64; 16]; 2];
        let handles = slots.each_mut().map(|slot| slot.as_mut_ptr() as u64);
        let null_desc = [
            format.0 as u32,
            D3D12_RTV_DIMENSION_TEXTURE2D.0 as u32,
            0,
            0,
            0,
            0,
        ];
        crate::descriptor::create_rtv(null, null, &null_desc, handles[0]);
        crate::descriptor::create_rtv(null, target, null.cast(), handles[1]);
        crate::list::ensure_state(&list, target, D3D12_RESOURCE_STATE_RENDER_TARGET.0 as u32);
        // SAFETY: open list, live target and graphics pipeline.
        unsafe {
            l.ClearRenderTargetView(host.rtv(Some(color), None), &[0.0, 0.0, 1.0, 1.0], None);
            l.SetGraphicsRootSignature(&root);
            l.SetPipelineState(&pso);
            l.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        }
        viewports(
            &list,
            &[D3D12_VIEWPORT {
                Width: SIDE as f32,
                Height: SIDE as f32,
                MaxDepth: 1.0,
                ..Default::default()
            }],
        );
        scissors(
            &list,
            &[RECT {
                right: SIDE as i32,
                bottom: SIDE as i32,
                ..Default::default()
            }],
        );
        State::default().set_targets(&list, &handles, 0);
        // SAFETY: complete graphics state on the open list.
        unsafe { l.DrawInstanced(3, 1, 0, 0) };

        assert_mip_zero_pixels(host, &list, target, SIDE, [0, 255, 0, 255]);
    }

    #[test]
    fn array_views_sample_and_render_distinct_color_mips() {
        let Some(host) = crate::host::host() else {
            assert!(std::env::var_os("VICTORY_D3D12_WARP").is_none());
            return;
        };
        let device = &host.device;
        let format = DXGI_FORMAT_R8G8B8A8_UNORM;
        let (root, pso) = graphics_fixture(
            device,
            r#"
                [RootSignature("RootFlags(ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT), DescriptorTable(SRV(t0))")]
                float4 vs(uint id : SV_VertexID) : SV_Position {
                    return float4(id == 2 ? 3 : -1, id == 1 ? -3 : 1, 0, 1);
                }
                Texture2DArray<float4> image : register(t0);
                float4 ps() : SV_Target0 { return image.Load(int4(0, 0, 0, 0)).brga; }
            "#,
            &[format],
        );
        const SIDE: u32 = 4;
        let fixture = color_texture(SIDE, 2);
        let target = fixture.0;
        let color = crate::resource::host_resource(target).expect("color target");
        let list = host.new_list().unwrap();
        let l = &list.list;
        let heap = std::sync::Arc::new(crate::descriptor::host::HostHeap::new(false, 2).unwrap());
        list.bind_heaps([Some(&heap), None]);
        let target_state = D3D12_RESOURCE_STATE_RENDER_TARGET.0 as u32;
        crate::list::ensure_state(&list, target, target_state);
        // SAFETY: these graphics objects and the texture outlive submission.
        let (heap_start, step) = unsafe {
            l.ClearRenderTargetView(host.rtv(Some(color), None), &[1.0, 0.0, 0.0, 1.0], None);
            l.SetGraphicsRootSignature(&root);
            l.SetPipelineState(&pso);
            l.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            (
                heap.heap.GetGPUDescriptorHandleForHeapStart().ptr,
                u64::from(
                    device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV),
                ),
            )
        };
        let used = UsedResource {
            kind: ResourceType::Srv,
            root: RootType::Table,
            table_offset_dwords: 0,
            lower_bound: 0,
            stride: 8,
            dwords: 8,
            user_data: 0,
            slot: 0,
        };
        let mut state = State::default();
        let mut slots = [[0u64; 16]; 2];
        let handles = slots.each_mut().map(|slot| slot.as_mut_ptr() as u64);
        for (source, destination, levels) in [(0, 1, 1), (1, 0, u32::MAX)] {
            let srv = View::Shader(
                target as usize,
                Some([
                    format.0 as u32,
                    D3D12_SRV_DIMENSION_TEXTURE2DARRAY.0 as u32,
                    D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    0, // Padding before the descriptor union.
                    source,
                    levels,
                    0,
                    1,
                    0,
                    0,
                ]),
            );
            let rtv = [
                format.0 as u32,
                D3D12_RTV_DIMENSION_TEXTURE2DARRAY.0 as u32,
                destination,
                0,
                1,
                0,
            ];
            let handle = handles[destination as usize];
            crate::descriptor::create_rtv(std::ptr::null(), target, &rtv, handle);
            state.set_targets(&list, &[handle], 0);
            if source == 0 {
                // Initialize the destination so dropping either draw fails the readback check.
                let rtv = crate::descriptor::rtv_record(handle)
                    .unwrap()
                    .desc()
                    .unwrap();
                // SAFETY: the initial whole-resource render-target state includes mip 1.
                unsafe {
                    l.ClearRenderTargetView(
                        host.rtv(Some(color), Some(&rtv)),
                        &[0.0, 0.0, 0.0, 1.0],
                        None,
                    );
                }
            }
            heap.make_at(source, &srv);
            readable(&list, &used, &srv);
            for (image, mips) in &state.targets {
                crate::list::ensure_mips(&list, *image as This, target_state, mips.clone());
            }
            let side = SIDE >> destination;
            viewports(
                &list,
                &[D3D12_VIEWPORT {
                    Width: side as f32,
                    Height: side as f32,
                    MaxDepth: 1.0,
                    ..Default::default()
                }],
            );
            scissors(
                &list,
                &[RECT {
                    right: side as i32,
                    bottom: side as i32,
                    ..Default::default()
                }],
            );
            // SAFETY: the target and single SRV view address distinct mips.
            unsafe {
                l.SetGraphicsRootDescriptorTable(
                    0,
                    D3D12_GPU_DESCRIPTOR_HANDLE {
                        ptr: heap_start + u64::from(source) * step,
                    },
                );
                l.DrawInstanced(3, 1, 0, 0);
            }
        }
        // Both draws must run to rotate the source red through green to blue.
        assert_mip_zero_pixels(host, &list, target, SIDE, [0, 0, 255, 255]);
    }
    #[test]
    fn depth_and_stencil_shader_views_read_their_own_planes() {
        use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_D24_UNORM_S8_UINT;

        let Some(host) = crate::host::host() else {
            assert!(std::env::var_os("VICTORY_D3D12_WARP").is_none());
            return;
        };
        let device = &host.device;
        let (root, pso) = graphics_fixture(
            device,
            r#"
                [RootSignature("RootFlags(ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT), DescriptorTable(SRV(t0, numDescriptors=2))")]
                float4 vs(uint id : SV_VertexID) : SV_Position {
                    return float4(id == 2 ? 3 : -1, id == 1 ? -3 : 1, 0, 1);
                }
                Texture2D<float4> depth : register(t0);
                Texture2D<uint4> stencil : register(t1);
                float4 ps() : SV_Target0 {
                    return float4(depth.Load(int3(0, 0, 0)).r,
                                  float(stencil.Load(int3(0, 0, 0)).g) / 255.0, 0, 1);
                }
            "#,
            &[DXGI_FORMAT_R8G8B8A8_UNORM],
        );
        const SIDE: u32 = 4;
        let depth = fixture_texture(
            SIDE,
            1,
            DXGI_FORMAT_D24_UNORM_S8_UINT,
            D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
            D3D12_RESOURCE_STATE_DEPTH_WRITE,
        );
        let target = color_texture(SIDE, 1);
        let image = crate::resource::host_resource(depth.0).expect("depth texture");
        let color = crate::resource::host_resource(target.0).expect("color target");
        let list = host.new_list().unwrap();
        let l = &list.list;
        let heap = std::sync::Arc::new(crate::descriptor::host::HostHeap::new(false, 2).unwrap());
        list.bind_heaps([Some(&heap), None]);
        let dsv = D3D12_DEPTH_STENCIL_VIEW_DESC {
            Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
            ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D,
            ..Default::default()
        };
        let rtv = host.rtv(Some(color), None);
        crate::list::ensure_state(&list, depth.0, D3D12_RESOURCE_STATE_DEPTH_WRITE.0 as u32);
        crate::list::ensure_state(&list, target.0, D3D12_RESOURCE_STATE_RENDER_TARGET.0 as u32);
        // SAFETY: the two textures and their descriptors outlive the recorded commands.
        unsafe {
            l.ClearDepthStencilView(
                host.dsv(image, Some(&dsv)),
                D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL,
                0.25,
                173,
                None,
            );
            l.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0], None);
        }
        for (slot, format) in [DXGI_FORMAT_D24_UNORM_S8_UINT.0 as u32, 120].into_iter().enumerate() {
            heap.make_at(
                slot as u32,
                &View::Shader(
                    depth.0 as usize,
                    Some([
                        format,
                        D3D12_SRV_DIMENSION_TEXTURE2D.0 as u32,
                        D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                        0,
                        0,
                        1,
                        0,
                        0,
                        0,
                        0,
                    ]),
                ),
            );
        }
        assert_eq!(host.log_messages(), None, "depth/stencil SRV creation");
        crate::list::ensure_state(
            &list,
            depth.0,
            (D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE
                | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE).0 as u32,
        );
        viewports(&list, &[D3D12_VIEWPORT {
            Width: SIDE as f32,
            Height: SIDE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]);
        scissors(&list, &[RECT {
            right: SIDE as i32,
            bottom: SIDE as i32,
            ..Default::default()
        }]);
        // SAFETY: all referenced graphics and descriptor objects are live.
        unsafe {
            l.SetGraphicsRootSignature(&root);
            l.SetPipelineState(&pso);
            l.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            l.OMSetRenderTargets(1, Some(&rtv), false, None);
            l.SetGraphicsRootDescriptorTable(0, heap.heap.GetGPUDescriptorHandleForHeapStart());
            l.DrawInstanced(3, 1, 0, 0);
        }
        assert_mip_zero_pixels(host, &list, target.0, SIDE, [64, 173, 0, 255]);
    }

}
