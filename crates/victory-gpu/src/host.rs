//! The host GPU: one D3D12 device and direct queue (decision 0007), and a
//! window to present on.
//!
//! shortcut: every submission is waited for, so CPU and GPU never overlap;
//! overlap them with per-submission fences once more than clears run.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, RECT, WAIT_OBJECT_0, WPARAM};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_12_0;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Interface, PCWSTR, w};

mod compositor;
use compositor::Compositor;
pub(crate) use compositor::PresentGeometry;

/// Resource states without the console's own bits (compression hints from
/// bit 25 up), which the host rejects.
pub(crate) fn pc_state(state: u32) -> u32 {
    state & 0x01ff_ffff
}

/// A console state as a host texture state: PC bits only, and none of the
/// buffer-only ones the console allows on textures (vertex, constant and
/// index buffer, stream out, indirect arguments, as in "generic read").
pub(crate) fn texture_state(state: u32) -> u32 {
    const BUFFER_ONLY: u32 = 0x1 | 0x2 | 0x100 | 0x200;
    pc_state(state) & !BUFFER_ONLY
}

/// Render target views live only until their command is recorded, so a
/// ring of host descriptors is enough.
const RTV_RING: u32 = 1024;

pub(crate) struct Host {
    pub(crate) device: ID3D12Device,
    /// Wave sizes reported by `D3D12_FEATURE_D3D12_OPTIONS1` (`None` if unavailable).
    pub(crate) wave_range: Option<victory_shader::emit::WaveRange>,
    queue: ID3D12CommandQueue,
    fence: ID3D12Fence,
    /// The last value signalled on `fence`, behind the submission lock.
    submitted: Mutex<u64>,
    event: HANDLE,
    rtvs: ID3D12DescriptorHeap,
    rtv_size: u32,
    /// A ring of depth views, used as `rtvs`.
    dsvs: ID3D12DescriptorHeap,
    dsv_size: u32,
    next_dsv: AtomicU32,
    /// With `VICTORY_D3D12_DEBUG` set (diagnostics builds): the debug
    /// layer's messages.
    messages: Option<ID3D12InfoQueue>,
    next_rtv: AtomicU32,
    presentation: Mutex<Presentation>,
}

// SAFETY: D3D12 devices, queues, fences and descriptor heaps are
// free-threaded; the event handle is only waited on under `submitted`.
unsafe impl Send for Host {}
// SAFETY: as above.
unsafe impl Sync for Host {}

/// A host resource waiting to be dropped after in-flight GPU work finishes.
struct Retired {
    frame: u64,
    #[allow(dead_code)]
    resource: ID3D12Resource,
}

// SAFETY: D3D12 resources are free-threaded and may be released on any thread.
unsafe impl Send for Retired {}

/// Host resources retired on worker threads (for example when a descriptor
/// slot is overwritten or a title resource is released while a submission is
/// executing on the GPU). Kept alive for several presented frames because a
/// single title frame spans many submissions and worker threads may record
/// lists ahead of presentation.
static RETIRED: Mutex<Vec<Retired>> = Mutex::new(Vec::new());

/// Holds `resource` alive until several full frames have completed on the GPU.
pub(crate) fn retire(resource: ID3D12Resource) {
    let frame = crate::queue::presented();
    RETIRED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Retired { frame, resource });
}

/// Ages are in frames presented to the host (not the title's 60 Hz clock, which
/// ticks many times per frame while the host runs at 1-2 fps).
/// Takes retired resources old enough to drop once the current submission
/// finishes on the GPU.
fn take_retired() -> Vec<Retired> {
    const KEEP_FRAMES: u64 = 4;
    let now = crate::queue::presented();
    let mut guard = RETIRED.lock().unwrap_or_else(|e| e.into_inner());
    let mut ready = Vec::new();
    let mut i = 0;
    while i < guard.len() {
        if now.saturating_sub(guard[i].frame) >= KEEP_FRAMES {
            ready.push(guard.swap_remove(i));
        } else {
            i += 1;
        }
    }
    ready
}

/// Time spent in submissions since the last frame-rate line, in
/// microseconds: preparing them on the CPU (title memory copies, texture
/// detiling), then waiting for the GPU.
static SUBMISSION_US: [std::sync::atomic::AtomicU64; 2] =
    [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];

/// After submissions: shader-written ranges compared and copied back into
/// title memory (microseconds, bytes that differed), since the last
/// frame-rate line.
static COPY_BACK: [std::sync::atomic::AtomicU64; 2] =
    [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];

/// Of the preparing: title memory copies (microseconds, bytes) and texture
/// refreshes (microseconds, bytes hashed), since the last frame-rate line.
pub(crate) static PREPARING: [std::sync::atomic::AtomicU64; 4] = [
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
];

/// What the runtime does when the host GPU fails for good: report it like a
/// crash and end the run. The console has no such state, and carrying on
/// draws nothing at seconds per frame.
static ON_FAILURE: OnceLock<fn(&str) -> !> = OnceLock::new();

/// Sets what a host GPU failure does (the runtime's crash report).
pub fn on_failure(report: fn(&str) -> !) {
    let _ = ON_FAILURE.set(report);
}

/// Ends the run on a host GPU failure: what failed, the removal reason and
/// the debug layer's first error, if any (`VICTORY_D3D12_DEBUG=1` in a
/// diagnostics build names the command).
pub(crate) fn fail(what: std::fmt::Arguments) -> ! {
    let mut text = what.to_string();
    if let Some(host) = host() {
        // SAFETY: a plain query.
        if let Err(e) = unsafe { host.device.GetDeviceRemovedReason() } {
            text.push_str(&format!("; device removed: {e}"));
        }
        match host.log_messages() {
            Some(first) => text.push_str(&format!("; debug layer: {first}")),
            None if host.messages.is_none() => text.push_str(
                "; a diagnostics build with VICTORY_D3D12_DEBUG=1 names the command",
            ),
            None => {}
        }
    }
    tracing::error!("{text}");
    match ON_FAILURE.get() {
        Some(report) => report(&text),
        None => std::process::abort(),
    }
}

/// The host GPU, created on first use; `None` without one (a run then
/// behaves as before: nothing executes).
pub(crate) fn host() -> Option<&'static Host> {
    static HOST: OnceLock<Option<Host>> = OnceLock::new();
    HOST.get_or_init(|| match create() {
        Ok(host) => Some(host),
        Err(e) => {
            tracing::warn!(error = %e, "no host D3D12 device; command lists will not run");
            None
        }
    })
    .as_ref()
}

fn create() -> windows::core::Result<Host> {
    let warp = cfg!(feature = "diagnostics") && std::env::var_os("VICTORY_D3D12_WARP").is_some();
    create_device(warp)
}

fn create_device(warp: bool) -> windows::core::Result<Host> {
    // SAFETY: plain D3D12 object creation with valid descriptions.
    unsafe {
        // The debug layer only in builds with diagnostics.
        let debug =
            cfg!(feature = "diagnostics") && std::env::var_os("VICTORY_D3D12_DEBUG").is_some();
        if debug {
            let mut layer: Option<ID3D12Debug> = None;
            match D3D12GetDebugInterface(&mut layer) {
                Ok(()) => layer.iter().for_each(|l| l.EnableDebugLayer()),
                Err(e) => tracing::warn!(error = %e, "no D3D12 debug layer"),
            }
            // GPU-based validation (`VICTORY_D3D12_GBV`): out-of-range
            // descriptor heap indices and the like become messages naming
            // the command instead of page faults. Without state tracking
            // (directly indexed textures are not transitioned).
            if std::env::var_os("VICTORY_D3D12_GBV").is_some()
                && let Some(l) = &layer
            {
                if let Ok(d1) = l.cast::<ID3D12Debug1>() {
                    d1.SetEnableGPUBasedValidation(true);
                }
                // Isolated WARP tests also validate resource-state transitions.
                if !cfg!(test)
                    && let Ok(d2) = l.cast::<ID3D12Debug2>()
                {
                    d2.SetGPUBasedValidationFlags(
                        D3D12_GPU_BASED_VALIDATION_FLAGS_DISABLE_STATE_TRACKING,
                    );
                }
                tracing::info!("GPU-based validation on");
            }
        }
        // Diagnostics: device-removed data (the command that faulted, the
        // faulting address), read when the device is removed.
        if dred_on() {
            let mut dred: Option<ID3D12DeviceRemovedExtendedDataSettings> = None;
            if D3D12GetDebugInterface(&mut dred).is_ok()
                && let Some(dred) = dred
            {
                dred.SetAutoBreadcrumbsEnablement(D3D12_DRED_ENABLEMENT_FORCED_ON);
                dred.SetPageFaultEnablement(D3D12_DRED_ENABLEMENT_FORCED_ON);
            }
        }
        let mut device: Option<ID3D12Device> = None;
        // Diagnostics: the software adapter, on which a shader's wild access
        // is an access violation in this process (a crash report with the
        // address and instruction) instead of a GPU fault and driver reset.
        if warp {
            let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let adapter: IDXGIAdapter = factory.EnumWarpAdapter()?;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_12_0, &mut device)?;
            tracing::info!("host device on the software adapter (WARP)");
        } else {
            D3D12CreateDevice(None, D3D_FEATURE_LEVEL_12_0, &mut device)?;
        }
        let device = device.ok_or_else(windows::core::Error::empty)?;
        let messages = debug
            .then(|| device.cast::<ID3D12InfoQueue>().ok())
            .flatten();
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let event = CreateEventW(None, false, false, None)?;
        let rtvs: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: RTV_RING,
                ..Default::default()
            })?;
        let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
        let dsvs: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                NumDescriptors: RTV_RING,
                ..Default::default()
            })?;
        let dsv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV);
        let mut opt1 = D3D12_FEATURE_DATA_D3D12_OPTIONS1::default();
        let wave_range = (device
            .CheckFeatureSupport(
                D3D12_FEATURE_D3D12_OPTIONS1,
                (&raw mut opt1).cast(),
                size_of_val(&opt1) as u32,
            )
            .is_ok()
            && opt1.WaveOps.as_bool()
            && opt1.WaveLaneCountMin > 0
            && opt1.WaveLaneCountMin <= opt1.WaveLaneCountMax)
            .then_some(victory_shader::emit::WaveRange {
                min: opt1.WaveLaneCountMin,
                max: opt1.WaveLaneCountMax,
            });
        tracing::info!(?wave_range, "host D3D12 device ready");
        Ok(Host {
            device,
            wave_range,
            queue,
            fence,
            submitted: Mutex::new(0),
            event,
            rtvs,
            rtv_size,
            dsvs,
            dsv_size,
            next_dsv: AtomicU32::new(0),
            messages,
            next_rtv: AtomicU32::new(0),
            presentation: Mutex::default(),
        })
    }
}

/// A host command list and its allocator, always used together.
pub(crate) struct HostList {
    allocator: ID3D12CommandAllocator,
    pub(crate) list: ID3D12GraphicsCommandList,
    /// Work for submission time, in recorded order.
    deferred: Mutex<Deferred>,
    /// Upload memory for data the list's draws make (expanded vertices),
    /// reused after a reset.
    scratch: Mutex<Scratch>,
    /// Host texture states the list assumes, by host copy: at its start and
    /// at its end. Lists are recorded on many threads and run in submission
    /// order, so the transitions from a texture's real state are made at
    /// submission, in `prelude`. Holding the copies keeps every texture the
    /// list names alive until it is reset, even if the title releases the
    /// resource first.
    states: Mutex<States>,
    /// The list that runs those transitions just before this one.
    prelude: (ID3D12CommandAllocator, ID3D12GraphicsCommandList),
    /// The pipelines the list binds, for reports of slow submissions.
    pipelines: Mutex<std::collections::BTreeSet<String>>,
    /// The work its indirect executions may still do (`indirect`): a
    /// shader-writable word, zeroed when the list is submitted.
    budget: OnceLock<Option<(ID3D12Resource, usize)>>,
    /// The texts of the markers `note_op` set, in order (for device-removed
    /// data without marker texts), with the host GPU range a copy reads.
    #[cfg(feature = "diagnostics")]
    notes: Mutex<Vec<Note>>,
    /// With `VICTORY_GPU_TIMING`: a GPU timestamp before each noted command
    /// and at the end, and where they are read back.
    #[cfg(feature = "diagnostics")]
    stamps: OnceLock<Option<(ID3D12QueryHeap, ID3D12Resource)>>,
}

/// Timestamps per list (diagnostics); commands past them are not timed.
#[cfg(feature = "diagnostics")]
const STAMPS: u32 = 4096;

/// A marker's text and the host range its command reads (diagnostics).
#[cfg(feature = "diagnostics")]
type Note = (String, Option<(u64, u64)>);

type States = std::collections::HashMap<usize, TrackedTexture>;

struct TrackedTexture {
    first: u32,
    last: u32,
    copy: std::sync::Arc<crate::resource::HostTexture>,
    /// Subresources temporarily in a different state from `last`.
    mips: std::collections::BTreeMap<u32, u32>,
}

impl TrackedTexture {
    fn unify(&mut self, mut record: impl FnMut(&ID3D12Resource, u32, u32, u32)) {
        for (mip, before) in std::mem::take(&mut self.mips) {
            record(&self.copy.texture, mip, before, self.last);
        }
    }
}

/// Whether device-removed data is on (`VICTORY_D3D12_DRED`, diagnostics
/// builds).
pub(crate) fn dred_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        cfg!(feature = "diagnostics") && std::env::var_os("VICTORY_D3D12_DRED").is_some()
    })
}

/// Whether each list of a submission runs and is timed on its own, slow
/// ones logged with their commands (`VICTORY_GPU_TIMING`, diagnostics
/// builds; the commands are noted with `VICTORY_D3D12_DRED` on).
fn timing() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        cfg!(feature = "diagnostics") && std::env::var_os("VICTORY_GPU_TIMING").is_some()
    })
}

/// Names a host object for the device-removed report, which names the
/// allocations a page fault hit (only with device-removed data on).
pub(crate) fn name(object: &impl Interface, text: impl FnOnce() -> String) {
    if dred_on()
        && let Ok(object) = object.cast::<ID3D12Object>()
    {
        // SAFETY: a plain call on a live object.
        let _ = unsafe { object.SetName(&windows::core::HSTRING::from(text())) };
    }
}

/// Upload chunks handed out front to back: the chunks, the one in use and
/// the bytes used in it.
#[derive(Default)]
struct Scratch {
    chunks: Vec<(ID3D12Resource, usize, u64)>,
    current: usize,
    used: u64,
}

// SAFETY: the chunk pointers are persistent mappings of upload buffers.
unsafe impl Send for Scratch {}

const SCRATCH_CHUNK: u64 = 4 << 20;

/// One piece of submission-time work, in recorded order.
enum Step {
    /// A copy into title memory (destination, source, bytes): the title's
    /// buffer copies, which later refreshes must see.
    Copy(usize, usize, usize),
    /// A copy of title memory into the list's own upload memory.
    Fill(usize, usize, usize),
    /// Brings a range (address, bytes) of a buffer's host copy up to date:
    /// once per submission, whichever lists ask, until a `Copy` or `Run`
    /// intervenes.
    Refresh(std::sync::Arc<crate::shadow::Shadow>, usize, usize),
    /// Work that may write title memory.
    Run(Box<dyn Fn() + Send>),
    /// Work that writes host memory only.
    Host(Box<dyn Fn() + Send>),
    /// Shaders write a range (address, bytes) of a buffer's host copy.
    GpuWrite(std::sync::Arc<crate::shadow::Shadow>, usize, usize),
    /// Brings a texture's staging buffer up to date: once per submission,
    /// whichever lists sample it, until a `Copy` or `Run` intervenes.
    Texture(std::sync::Arc<crate::texture::Staging>),
}

/// What a list does on the CPU when it is submitted: copies between host
/// memory regions (title buffers, upload buffers and buffer copies), and
/// the upload buffers its GPU commands read, kept until the list is reset.
#[derive(Default)]
struct Deferred {
    /// Destination, source, bytes.
    copies: Vec<Step>,
    uploads: Vec<ID3D12Resource>,
    /// Host objects the list's commands name (pipelines, root signatures).
    /// The title may release its objects before it submits the list.
    kept: Vec<windows::core::IUnknown>,
    /// Bound heaps own the resources in mirrored descriptors, including
    /// slots chosen by shaders and invisible to CPU recording.
    heaps: Vec<std::sync::Arc<crate::descriptor::host::HostHeap>>,
    /// Textures named by aliasing and UAV barriers without a state transition.
    barrier_textures: std::collections::HashMap<usize, std::sync::Arc<crate::resource::HostTexture>>,
    /// Buffer ranges refreshed since the last title copy (host copy,
    /// address, bytes): refreshing them again would find the same bytes.
    refreshed: std::collections::HashSet<(usize, usize, usize)>,
    /// Textures whose contents the list brings up to date.
    textures: std::collections::HashSet<usize>,
    /// Bottom-of-pipe writes, done once the list has run: address, what,
    /// bytes.
    writes: Vec<(usize, crate::queue::Written, usize)>,
    /// Whether a step zeroes the indirect-work budget.
    budget_zeroed: bool,
    #[cfg(feature = "diagnostics")]
    mesh_samples: Vec<crate::indirect::capture::Sample>,
}

impl Host {
    fn poll_submission(&self, value: u64) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            // SAFETY: a live fence; UINT64_MAX is D3D12's device removal sentinel.
            let completed = unsafe { self.fence.GetCompletedValue() };
            if completed != u64::MAX && completed >= value {
                return;
            }
            if completed == u64::MAX || std::time::Instant::now() >= deadline {
                fail(format_args!("host submission fence {value} did not complete"));
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Whether the host runs mesh shaders at shader model 6.6 (translated
    /// mesh shaders need both); logged once.
    pub(crate) fn mesh_shaders(&self) -> bool {
        static MESH: OnceLock<bool> = OnceLock::new();
        *MESH.get_or_init(|| {
            let mut options = D3D12_FEATURE_DATA_D3D12_OPTIONS7::default();
            let mut model = D3D12_FEATURE_DATA_SHADER_MODEL {
                HighestShaderModel: D3D_SHADER_MODEL_6_6,
            };
            // SAFETY: each query writes its own structure of the given size.
            let ok = unsafe {
                self.device
                    .CheckFeatureSupport(
                        D3D12_FEATURE_D3D12_OPTIONS7,
                        (&raw mut options).cast(),
                        size_of_val(&options) as u32,
                    )
                    .is_ok()
                    && self
                        .device
                        .CheckFeatureSupport(
                            D3D12_FEATURE_SHADER_MODEL,
                            (&raw mut model).cast(),
                            size_of_val(&model) as u32,
                        )
                        .is_ok()
            };
            let mesh = ok
                && options.MeshShaderTier != D3D12_MESH_SHADER_TIER_NOT_SUPPORTED
                && model.HighestShaderModel.0 >= D3D_SHADER_MODEL_6_6.0;
            tracing::info!(mesh, "host mesh shader support");
            mesh
        })
    }

    /// A new list, open for recording.
    pub(crate) fn new_list(&self) -> windows::core::Result<HostList> {
        // SAFETY: plain object creation.
        unsafe {
            let allocator: ID3D12CommandAllocator = self
                .device
                .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let list: ID3D12GraphicsCommandList = self.device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &allocator,
                None,
            )?;
            let prelude_allocator: ID3D12CommandAllocator = self
                .device
                .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let prelude: ID3D12GraphicsCommandList = self.device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &prelude_allocator,
                None,
            )?;
            prelude.Close()?;
            Ok(HostList {
                allocator,
                list,
                deferred: Mutex::default(),
                scratch: Mutex::default(),
                states: Mutex::default(),
                prelude: (prelude_allocator, prelude),
                pipelines: Mutex::default(),
                budget: OnceLock::new(),
                #[cfg(feature = "diagnostics")]
                notes: Mutex::default(),
                #[cfg(feature = "diagnostics")]
                stamps: OnceLock::new(),
            })
        }
    }

    /// Runs closed lists and waits for them.
    pub(crate) fn execute(&self, runs: &[&HostList]) {
        let _ = self.execute_checked(runs);
    }

    /// First debug-layer error from this submission, captured before the queue is cleared.
    pub(crate) fn execute_checked(&self, runs: &[&HostList]) -> Option<String> {
        // One submission at a time from preparation to copy-back: snapshots
        // and the ranges shaders write are per host copy, not per submission.
        let mut submitted = self.submitted.lock().unwrap_or_else(|e| e.into_inner());
        self.execute_locked(runs, &mut submitted)
    }

    /// Reads a retained diagnostic target while holding the same lock as
    /// submissions, so its actual state cannot change before our barriers.
    #[cfg(feature = "diagnostics")]
    pub(crate) fn read_target(
        &self,
        target: &crate::resource::HostTexture,
    ) -> Option<(ID3D12Resource, D3D12_PLACED_SUBRESOURCE_FOOTPRINT)> {
        let list = self.new_list().ok()?;
        let mut submitted = self.submitted.lock().unwrap_or_else(|e| e.into_inner());
        let texture = &target.texture;
        let before = D3D12_RESOURCE_STATES(legal_state(
            texture,
            target.state.load(Ordering::Relaxed),
        ) as i32);
        let source = D3D12_RESOURCE_STATE_COPY_SOURCE;
        // SAFETY: a live target and open list; the lock excludes submissions
        // that update the tracked state. The target returns to that state.
        let read = unsafe {
            if before != source {
                list.list
                    .ResourceBarrier(&[transition(texture, before, source)]);
            }
            let read = crate::capture::read_back(&self.device, &list.list, texture)?;
            if before != source {
                list.list
                    .ResourceBarrier(&[transition(texture, source, before)]);
            }
            read
        };
        list.close();
        self.execute_locked(&[&list], &mut submitted)
            .is_none()
            .then_some(read)
    }

    fn execute_locked(&self, runs: &[&HostList], submitted: &mut u64) -> Option<String> {
        let started = std::time::Instant::now();
        #[cfg(feature = "diagnostics")]
        crate::copystat::add(crate::copystat::C::Submissions, 1);
        let mut lists: Vec<Option<ID3D12CommandList>> = Vec::with_capacity(2 * runs.len());
        // Where each run's lists start in `lists`.
        let mut firsts = Vec::with_capacity(runs.len());
        let mut batch = Batch::default();
        for list in runs {
            list.run_deferred(&mut batch);
            firsts.push(lists.len());
            if let Some(prelude) = list.prelude() {
                lists.push(Some(prelude));
            }
            lists.push(list.list.cast().ok());
        }
        batch.flush();
        let retired = take_retired();
        let prepared = std::time::Instant::now();
        // Split command lists per-run so no single ExecuteCommandLists call
        // overwhelms the host driver scheduler. Only wait on the final submission
        // unless timing() is enabled.
        let groups: Vec<&[Option<ID3D12CommandList>]> = firsts
            .iter()
            .zip(firsts.iter().skip(1).chain([&lists.len()]))
            .map(|(&a, &b)| &lists[a..b])
            .collect();
        let each = timing();
        for (i, group) in groups.iter().enumerate() {
            let started = std::time::Instant::now();
            // SAFETY: closed lists.
            unsafe { self.queue.ExecuteCommandLists(group) };
            if !each && i + 1 < groups.len() {
                continue;
            }
            *submitted += 1;
            // SAFETY: closed lists; the fence and event are ours.
            unsafe {
                if let Err(e) = self.queue.Signal(&self.fence, *submitted) {
                    fail(format_args!("could not signal host submission fence: {e}"));
                }
                let waited = self.fence.SetEventOnCompletion(*submitted, self.event);
                let event_signalled =
                    waited.is_ok() && WaitForSingleObject(self.event, INFINITE) == WAIT_OBJECT_0;
                let completed = self.fence.GetCompletedValue();
                if !event_signalled || completed == u64::MAX || completed < *submitted {
                    // A failed wait or stale event leaves this submission in
                    // flight. Poll before releasing anything it may still use.
                    self.poll_submission(*submitted);
                }
            }
            #[cfg(feature = "diagnostics")]
            if each && started.elapsed() > std::time::Duration::from_millis(50) {
                // SAFETY: a plain query.
                let frequency = unsafe { self.queue.GetTimestampFrequency() }.unwrap_or(1);
                let commands = runs[i].notes.lock().unwrap_or_else(|e| e.into_inner()).len();
                tracing::warn!(ms = started.elapsed().as_millis(), commands, "slow host list");
                for (ms, op) in runs[i].slowest(frequency, 6) {
                    tracing::warn!("slow list command, {ms:.1} ms: {op}");
                }
            }
            let _ = (i, started);
        }
        let _ = &firsts;
        let gpu = prepared.elapsed();
        SUBMISSION_US[0].fetch_add((prepared - started).as_micros() as u64, Ordering::Relaxed);
        SUBMISSION_US[1].fetch_add(gpu.as_micros() as u64, Ordering::Relaxed);
        // The host resets a device whose submission runs for about 2 s.
        if gpu > std::time::Duration::from_millis(250) {
            let pipelines: std::collections::BTreeSet<String> = runs
                .iter()
                .flat_map(|l| l.pipelines.lock().unwrap_or_else(|e| e.into_inner()).clone())
                .collect();
            tracing::warn!(ms = gpu.as_millis(), lists = runs.len(), ?pipelines, "slow host submission");
        }
        // SAFETY: a plain query.
        if unsafe { self.device.GetDeviceRemovedReason() }.is_err() {
            fail(format_args!(
                "host GPU device removed after a submission of {} lists{}{}",
                runs.len(),
                self.removal_data(runs),
                last_notes(runs)
            ));
        }
        drop(retired);
        // What shaders wrote goes back before the bottom-of-pipe writes,
        // which tell the title the GPU is done; each range once per
        // submission, however many dispatches declared it (the world's
        // passes declare the same big ranges dozens of times).
        let copying = std::time::Instant::now();
        for (shadow, address, len) in batch.gpu_writes {
            let written = shadow.copy_back(address, len);
            COPY_BACK[1].fetch_add(written as u64, Ordering::Relaxed);
        }
        COPY_BACK[0].fetch_add(copying.elapsed().as_micros() as u64, Ordering::Relaxed);
        for list in runs {
            let deferred = list.deferred();
            #[cfg(feature = "diagnostics")]
            for sample in &deferred.mesh_samples {
                sample.report();
            }
            for &(at, what, bytes) in &deferred.writes {
                what.write(at as *mut u8, bytes);
            }
        }
        self.log_messages()
    }

    /// With `VICTORY_D3D12_DRED`: the faulting address and the allocations
    /// it lies in (by the names we give them) or the title memory copy just
    /// below it; for each list that did not finish, the command it stopped
    /// at and what the draws and dispatches in flight there run (or, in
    /// lists that never started, the pipelines they bind).
    fn removal_data(&self, runs: &[&HostList]) -> String {
        let Ok(dred) = self.device.cast::<ID3D12DeviceRemovedExtendedData1>() else {
            return String::new();
        };
        let mut text = String::new();
        let mut fault_va = None;
        // SAFETY: the outputs are the runtime's, valid while the device
        // lives; nodes are walked as linked, histories have their counts.
        unsafe {
            if let Ok(fault) = dred.GetPageFaultAllocationOutput1()
                && fault.PageFaultVA != 0
            {
                let va = fault.PageFaultVA;
                fault_va = Some(va);
                text.push_str(&format!("; page fault at {va:#x}"));
                for (what, head) in [
                    ("inside", fault.pHeadExistingAllocationNode),
                    ("inside freed", fault.pHeadRecentFreedAllocationNode),
                ] {
                    let mut names = Vec::new();
                    let mut node = head;
                    while let Some(n) = node.as_ref()
                        && names.len() < 8
                    {
                        let name = if n.ObjectNameW.is_null() {
                            "unnamed".to_owned()
                        } else {
                            n.ObjectNameW.to_string().unwrap_or_default()
                        };
                        names.push(format!("{name} (type {})", n.AllocationType.0));
                        node = n.pNext;
                    }
                    if !names.is_empty() {
                        text.push_str(&format!("; {what} {names:?}"));
                    }
                }
                text.push_str(&crate::shadow::near(va));
                text.push_str(&crate::descriptor::host::near_heaps(va));
            }
            // The plain output: the extended one (with marker texts) came
            // back without stopped lists on this host.
            let crumbs = match dred.GetAutoBreadcrumbsOutput() {
                Ok(crumbs) => crumbs,
                Err(e) => {
                    text.push_str(&format!("; no breadcrumbs ({e})"));
                    return text;
                }
            };
            // Every list's progress (done/count), in the runtime's order.
            let mut progress = Vec::new();
            let mut node = crumbs.pHeadAutoBreadcrumbNode;
            while let Some(p) = node.as_ref() {
                let done = p.pLastBreadcrumbValue.as_ref().copied().unwrap_or(0);
                let ours = p.pCommandList.as_ref().map(|l| l.as_raw());
                let index = runs.iter().position(|r| Some(r.list.as_raw()) == ours);
                progress.push(format!("{done}/{} {index:?}", p.BreadcrumbCount));
                node = p.pNext;
            }
            text.push_str(&format!("; breadcrumbs (done/count, list) {progress:?}"));
            let mut plain = crumbs.pHeadAutoBreadcrumbNode;
            while let Some(p) = plain.as_ref() {
                let done = p.pLastBreadcrumbValue.as_ref().copied().unwrap_or(0);
                if done < p.BreadcrumbCount && !p.pCommandHistory.is_null() {
                    let history =
                        std::slice::from_raw_parts(p.pCommandHistory, p.BreadcrumbCount as usize);
                    text.push_str(&format!(
                        "; a list stopped at command {done} of {} (op {})",
                        p.BreadcrumbCount, history[done as usize].0
                    ));
                    let list = p.pCommandList.as_ref().map(|l| l.as_raw());
                    let run = runs.iter().find(|r| Some(r.list.as_raw()) == list);
                    // The k-th marker command is the k-th note.
                    #[cfg(feature = "diagnostics")]
                    let markers: Vec<(u32, String)> = run.map_or_else(Vec::new, |r| {
                        let notes = r.notes.lock().unwrap_or_else(|e| e.into_inner());
                        history
                            .iter()
                            .enumerate()
                            .filter(|(_, op)| **op == D3D12_AUTO_BREADCRUMB_OP_SETMARKER)
                            .zip(notes.iter())
                            .map(|((at, _), (note, range))| {
                                let place = fault_va.zip(*range).map(|(va, r)| fault_in(va, r));
                                (at as u32, format!("{note}{}", place.unwrap_or_default()))
                            })
                            .collect()
                    });
                    #[cfg(not(feature = "diagnostics"))]
                    let markers: Vec<(u32, String)> = Vec::new();
                    #[cfg(not(feature = "diagnostics"))]
                    let _ = fault_va;
                    if done > 0 {
                        text.push_str(&in_flight(history, &markers, done as usize));
                    } else if let Some(r) = run {
                        let pipelines = r.pipelines.lock().unwrap_or_else(|e| e.into_inner());
                        text.push_str(&format!(", its pipelines {pipelines:?}"));
                    }
                }
                plain = p.pNext;
            }
        }
        text
    }

    /// Moves the debug layer's messages into our log; returns the first
    /// error among them.
    pub(crate) fn log_messages(&self) -> Option<String> {
        let Some(queue) = &self.messages else {
            return None;
        };
        let mut first_error = None;
        // SAFETY: each message is read into a buffer of the size the queue
        // reports, then the queue is emptied.
        unsafe {
            for i in 0..queue.GetNumStoredMessages() {
                let mut size = 0;
                if queue.GetMessage(i, None, &mut size).is_err() || size == 0 {
                    continue;
                }
                let mut buffer = vec![0u64; size.div_ceil(8)];
                let message = buffer.as_mut_ptr().cast::<D3D12_MESSAGE>();
                if queue.GetMessage(i, Some(message), &mut size).is_ok() {
                    let m = &*message;
                    let text = std::slice::from_raw_parts(m.pDescription, m.DescriptionByteLength);
                    let text = String::from_utf8_lossy(text);
                    let text = text.trim_end_matches('\0');
                    tracing::warn!(id = m.ID.0, severity = m.Severity.0, "D3D12: {text}");
                    if m.Severity.0 <= D3D12_MESSAGE_SEVERITY_ERROR.0 && first_error.is_none() {
                        first_error = Some(text.to_owned());
                    }
                }
            }
            queue.ClearStoredMessages();
        }
        first_error
    }

    /// A host render target view, valid until it is recorded.
    pub(crate) fn rtv(
        &self,
        resource: Option<&ID3D12Resource>,
        desc: Option<&D3D12_RENDER_TARGET_VIEW_DESC>,
    ) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let slot = self.next_rtv.fetch_add(1, Ordering::Relaxed) % RTV_RING;
        // SAFETY: a slot of our heap and a live resource or a typed null view.
        unsafe {
            let mut handle = self.rtvs.GetCPUDescriptorHandleForHeapStart();
            handle.ptr += (slot * self.rtv_size) as usize;
            self.device
                .CreateRenderTargetView(resource, desc.map(|d| d as *const _), handle);
            handle
        }
    }

    /// A host depth view of `resource`, valid until it is recorded.
    pub(crate) fn dsv(
        &self,
        resource: &ID3D12Resource,
        desc: Option<&D3D12_DEPTH_STENCIL_VIEW_DESC>,
    ) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let slot = self.next_dsv.fetch_add(1, Ordering::Relaxed) % RTV_RING;
        // SAFETY: a slot of our heap and a live resource.
        unsafe {
            let mut handle = self.dsvs.GetCPUDescriptorHandleForHeapStart();
            handle.ptr += (slot * self.dsv_size) as usize;
            self.device
                .CreateDepthStencilView(resource, desc.map(|d| d as *const _), handle);
            handle
        }
    }
}

impl HostList {
    /// The timestamp heap and its read-back buffer (diagnostics, timing on).
    #[cfg(feature = "diagnostics")]
    fn stamps(&self) -> Option<&(ID3D12QueryHeap, ID3D12Resource)> {
        if !timing() {
            return None;
        }
        self.stamps
            .get_or_init(|| {
                let device = &host()?.device;
                let mut heap: Option<ID3D12QueryHeap> = None;
                let mut back: Option<ID3D12Resource> = None;
                let readback = D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_READBACK, ..Default::default() };
                let desc = D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: u64::from(STAMPS) * 8,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                    ..Default::default()
                };
                // SAFETY: plain object creation.
                unsafe {
                    device
                        .CreateQueryHeap(
                            &D3D12_QUERY_HEAP_DESC { Type: D3D12_QUERY_HEAP_TYPE_TIMESTAMP, Count: STAMPS, NodeMask: 0 },
                            &mut heap,
                        )
                        .ok()?;
                    device
                        .CreateCommittedResource(
                            &readback,
                            D3D12_HEAP_FLAG_NONE,
                            &desc,
                            D3D12_RESOURCE_STATE_COPY_DEST,
                            None,
                            &mut back,
                        )
                        .ok()?;
                }
                Some((heap?, back?))
            })
            .as_ref()
    }

    /// The noted commands that took longest on the GPU, longest first, as
    /// (milliseconds, text) (diagnostics, timing on, after the list ran).
    #[cfg(feature = "diagnostics")]
    fn slowest(&self, frequency: u64, n: usize) -> Vec<(f64, String)> {
        let notes = self.notes.lock().unwrap_or_else(|e| e.into_inner());
        let (Some((_, back)), count) = (self.stamps(), notes.len().min(STAMPS as usize - 1)) else {
            return Vec::new();
        };
        let mut data = std::ptr::null_mut();
        let range = D3D12_RANGE { Begin: 0, End: (count + 1) * 8 };
        // SAFETY: a read-back buffer the finished list resolved into.
        let stamps: Vec<u64> = unsafe {
            if back.Map(0, Some(&range), Some(&mut data)).is_err() {
                return Vec::new();
            }
            let s = std::slice::from_raw_parts(data.cast::<u64>(), count + 1).to_vec();
            back.Unmap(0, None);
            s
        };
        let mut times: Vec<(f64, String)> = (0..count)
            .map(|i| {
                let ticks = stamps[i + 1].saturating_sub(stamps[i]);
                (ticks as f64 * 1e3 / frequency.max(1) as f64, notes[i].0.chars().take(300).collect())
            })
            .collect();
        times.sort_by(|a, b| b.0.total_cmp(&a.0));
        times.truncate(n);
        times
    }

    pub(crate) fn close(&self) {
        // The submission prelude and presentation use one state per texture.
        let mut barriers = Vec::new();
        let mut states = self.states.lock().unwrap_or_else(|e| e.into_inner());
        for state in states.values_mut() {
            state.unify(|texture, mip, before, after| {
                barriers.push(transition_subresource(
                    texture,
                    mip,
                    D3D12_RESOURCE_STATES(before as i32),
                    D3D12_RESOURCE_STATES(after as i32),
                ));
            });
        }
        drop(states);
        if !barriers.is_empty() {
            // SAFETY: an open list and resources held by the tracker.
            unsafe { self.list.ResourceBarrier(&barriers) };
        }
        #[cfg(feature = "diagnostics")]
        if let Some((heap, back)) = self.stamps() {
            let count = (self.notes.lock().unwrap_or_else(|e| e.into_inner()).len() as u32).min(STAMPS - 1);
            // SAFETY: an open list, our heap and read-back buffer.
            unsafe {
                self.list.EndQuery(heap, D3D12_QUERY_TYPE_TIMESTAMP, count);
                self.list.ResolveQueryData(heap, D3D12_QUERY_TYPE_TIMESTAMP, 0, count + 1, back, 0);
            }
        }
        // SAFETY: our list.
        if let Err(e) = unsafe { self.list.Close() } {
            fail(format_args!(
                "a host command list did not close ({e}): a command recorded into it was invalid"
            ));
        }
    }

    /// Only after the list's last submission finished (every one is waited
    /// for).
    pub(crate) fn reset(&self) {
        *self.deferred() = Deferred::default();
        self.states.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.pipelines.lock().unwrap_or_else(|e| e.into_inner()).clear();
        #[cfg(feature = "diagnostics")]
        self.notes.lock().unwrap_or_else(|e| e.into_inner()).clear();
        {
            let mut scratch = self.scratch.lock().unwrap_or_else(|e| e.into_inner());
            (scratch.current, scratch.used) = (0, 0);
        }
        // SAFETY: the list is not executing.
        let reset = unsafe {
            self.allocator
                .Reset()
                .and_then(|()| self.list.Reset(&self.allocator, None))
        };
        if let Err(e) = reset {
            fail(format_args!("a host command list did not reset ({e})"));
        }
    }

    /// Records the state changes required by a texture use. The first use
    /// sets the uniform state chosen by the submission prelude.
    pub(crate) fn transition(
        &self,
        resource: crate::object::This,
        wanted: u32,
        mips: Option<std::ops::Range<u32>>,
        mut record: impl FnMut(&ID3D12Resource, u32, u32, u32),
    ) {
        let Some(copy) = crate::resource::host_copy(resource) else {
            return;
        };
        let wanted = legal_state(&copy.texture, wanted);
        let mut states = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let state = states
            .entry(std::sync::Arc::as_ptr(&copy) as usize)
            .or_insert_with(|| TrackedTexture {
                first: wanted,
                last: wanted,
                copy,
                mips: Default::default(),
            });
        if let Some(mips) = mips {
            for mip in mips {
                let before = if wanted == state.last {
                    state.mips.remove(&mip)
                } else {
                    state.mips.insert(mip, wanted)
                }
                .unwrap_or(state.last);
                if before != wanted {
                    record(&state.copy.texture, mip, before, wanted);
                }
            }
        } else {
            state.unify(&mut record);
            let before = std::mem::replace(&mut state.last, wanted);
            if before != wanted {
                record(
                    &state.copy.texture,
                    D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    before,
                    wanted,
                );
            }
        }
    }

    /// At submission: the list that moves each texture this list uses from
    /// its real state to the one the list starts with; the textures' real
    /// states become those the list ends with.
    fn prelude(&self) -> Option<ID3D12CommandList> {
        let states = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let barriers: Vec<D3D12_RESOURCE_BARRIER> = states
            .values()
            .filter_map(|state| {
                let real = state.copy.state.swap(state.last, Ordering::Relaxed);
                (real != state.first).then(|| {
                    transition(
                        &state.copy.texture,
                        D3D12_RESOURCE_STATES(real as i32),
                        D3D12_RESOURCE_STATES(state.first as i32),
                    )
                })
            })
            .collect();
        if barriers.is_empty() {
            return None;
        }
        let (allocator, list) = &self.prelude;
        // SAFETY: the prelude is not executing (every submission is waited
        // for); the textures are live.
        let recorded = unsafe {
            allocator.Reset().and_then(|()| {
                list.Reset(allocator, None)?;
                list.ResourceBarrier(&barriers);
                list.Close()
            })
        };
        if let Err(e) = recorded {
            fail(format_args!("a host state prelude did not record ({e})"));
        }
        list.cast().ok()
    }

    /// Notes what the command about to be recorded does, as a marker
    /// command whose text the list keeps (diagnostics, with device-removed
    /// data on): the report pairs the breadcrumbs' marker commands with the
    /// texts in order and names the commands a fault stopped among.
    #[cfg(feature = "diagnostics")]
    pub(crate) fn note_op(&self, what: impl FnOnce() -> String) {
        self.note(what, None);
    }

    /// As [`HostList::note_op`], for a copy that reads the host GPU range
    /// `(address, bytes)`: the report places the faulting address in it.
    #[cfg(feature = "diagnostics")]
    pub(crate) fn note_read(&self, what: impl FnOnce() -> String, range: (u64, u64)) {
        self.note(what, Some(range));
    }

    #[cfg(feature = "diagnostics")]
    fn note(&self, what: impl FnOnce() -> String, range: Option<(u64, u64)>) {
        if dred_on() {
            let at = self.notes.lock().unwrap_or_else(|e| e.into_inner()).len() as u32;
            if let Some((heap, _)) = self.stamps() && at < STAMPS - 1 {
                // SAFETY: an open list and our query heap.
                unsafe { self.list.EndQuery(heap, D3D12_QUERY_TYPE_TIMESTAMP, at) };
            }
            let what = what();
            let text: Vec<u16> = what.encode_utf16().chain([0]).collect();
            const UNICODE: u32 = 0;
            // SAFETY: an open list; the marker is copied when recorded.
            unsafe {
                self.list.SetMarker(UNICODE, Some(text.as_ptr().cast()), 2 * text.len() as u32)
            };
            self.notes
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((what, range));
        }
    }

    /// Notes a pipeline the list binds.
    pub(crate) fn used_pipeline(&self, name: &str) {
        let mut pipelines = self.pipelines.lock().unwrap_or_else(|e| e.into_inner());
        if !pipelines.contains(name) {
            pipelines.insert(name.to_owned());
        }
    }

    fn deferred(&self) -> std::sync::MutexGuard<'_, Deferred> {
        self.deferred.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Copies `len` bytes from `src` to `dst` when the list is submitted.
    pub(crate) fn defer_copy(&self, dst: *mut u8, src: *const u8, len: usize) {
        let mut deferred = self.deferred();
        deferred
            .copies
            .push(Step::Copy(dst as usize, src as usize, len));
        deferred.refreshed.clear();
    }

    /// Copies `len` bytes of title memory at `src` into the list's upload
    /// memory at `dst` when the list is submitted.
    pub(crate) fn defer_fill(&self, dst: *mut u8, src: *const u8, len: usize) {
        self.deferred()
            .copies
            .push(Step::Fill(dst as usize, src as usize, len));
    }

    /// Brings `[address, address + len)` of a buffer's host copy up to date
    /// when the list is submitted (once per range until a title copy
    /// intervenes), keeping the copy alive until the list is reset.
    pub(crate) fn defer_refresh(
        &self,
        shadow: &std::sync::Arc<crate::shadow::Shadow>,
        address: usize,
        len: usize,
    ) {
        let mut deferred = self.deferred();
        let key = (std::sync::Arc::as_ptr(shadow) as usize, address, len);
        if deferred.refreshed.insert(key) {
            deferred
                .copies
                .push(Step::Refresh(shadow.clone(), address, len));
        }
    }

    /// Shaders of the list write `[address, address + len)` of a buffer's
    /// host copy: copied back into title memory once the list has run.
    pub(crate) fn defer_gpu_write(
        &self,
        shadow: &std::sync::Arc<crate::shadow::Shadow>,
        address: usize,
        len: usize,
    ) {
        self.deferred()
            .copies
            .push(Step::GpuWrite(shadow.clone(), address, len));
    }

    /// Stores `what` (`bytes` of it) at `dst` once the list has run.
    pub(crate) fn defer_write(&self, dst: *mut u8, what: crate::queue::Written, bytes: usize) {
        self.deferred().writes.push((dst as usize, what, bytes));
    }

    /// A mapped upload buffer of `size` bytes, alive until the list is
    /// reset.
    ///
    /// shortcut: one buffer per copy; pool them once uploads are frequent.
    pub(crate) fn upload(&self, host: &Host, size: u64) -> Option<(ID3D12Resource, *mut u8)> {
        let (buffer, data) = host.upload_buffer(size)?;
        name(&buffer, || format!("list upload {size:#x}"));
        self.deferred().uploads.push(buffer.clone());
        Some((buffer, data))
    }

    /// `len` bytes of upload memory (256-byte aligned) until the list is
    /// reset: the host GPU address and the CPU pointer.
    pub(crate) fn scratch(&self, len: u64) -> Option<(u64, *mut u8)> {
        let host = host()?;
        let mut s = self.scratch.lock().unwrap_or_else(|e| e.into_inner());
        let len = len.max(1).next_multiple_of(256);
        if len > SCRATCH_CHUNK {
            let (buffer, data) = host.upload_buffer(len)?;
            name(&buffer, || format!("list scratch {len:#x}"));
            // SAFETY: a live buffer.
            let gpu = unsafe { buffer.GetGPUVirtualAddress() };
            self.deferred().uploads.push(buffer);
            return Some((gpu, data));
        }
        if s.used + len > SCRATCH_CHUNK && s.current < s.chunks.len() {
            (s.current, s.used) = (s.current + 1, 0);
        }
        if s.current == s.chunks.len() {
            let (buffer, data) = host.upload_buffer(SCRATCH_CHUNK)?;
            name(&buffer, || "list scratch".into());
            // SAFETY: a live buffer.
            let gpu = unsafe { buffer.GetGPUVirtualAddress() };
            s.chunks.push((buffer, data as usize, gpu));
            s.used = 0;
        }
        let (_, data, gpu) = s.chunks[s.current];
        let at = s.used;
        s.used += len;
        Some((gpu + at, (data + at as usize) as *mut u8))
    }

    /// The host address of the list's indirect-work budget, zeroed at each
    /// submission of the list from the first use after a reset on.
    pub(crate) fn budget(&self) -> Option<u64> {
        let (buffer, data) = self
            .budget
            .get_or_init(|| {
                let (b, d) = host()?.shared_buffer(256)?;
                Some((b, d as usize))
            })
            .as_ref()?;
        let mut deferred = self.deferred();
        if !deferred.budget_zeroed {
            deferred.budget_zeroed = true;
            let at = *data;
            // SAFETY: the list's own mapped word, written before it runs.
            deferred
                .copies
                .push(Step::Host(Box::new(move || unsafe { (at as *mut u32).write_volatile(0) })));
        }
        // SAFETY: a live buffer.
        Some(unsafe { buffer.GetGPUVirtualAddress() })
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn keep_mesh_sample(&self, sample: crate::indirect::capture::Sample) {
        self.deferred().mesh_samples.push(sample);
    }

    /// Keeps `object` alive until the list is reset.
    pub(crate) fn keep(&self, object: &impl Interface) {
        if let Ok(object) = object.cast::<windows::core::IUnknown>() {
            self.deferred().kept.push(object);
        }
    }

    /// Binds native descriptor heaps and keeps their slot owners until reset.
    pub(crate) fn bind_heaps(
        &self,
        heaps: [Option<&std::sync::Arc<crate::descriptor::host::HostHeap>>; 2],
    ) {
        let native: Vec<_> = heaps.iter().flatten().map(|h| Some(h.heap.clone())).collect();
        self.deferred().heaps.extend(heaps.into_iter().flatten().cloned());
        // SAFETY: an open list and live shader-visible heaps.
        unsafe { self.list.SetDescriptorHeaps(&native) };
    }

    /// Retains the host texture named by an aliasing or UAV barrier.
    pub(crate) fn keep_barrier_texture(&self, texture: std::sync::Arc<crate::resource::HostTexture>) {
        self.deferred()
            .barrier_textures
            .entry(std::sync::Arc::as_ptr(&texture) as usize)
            .or_insert(texture);
    }

    /// Whether this is the first time since the reset that the list asks
    /// about `key` (a texture to bring up to date).
    pub(crate) fn first_use(&self, key: usize) -> bool {
        self.deferred().textures.insert(key)
    }

    /// Runs `work` when the list is submitted, in order with its copies.
    /// `work` may write title memory (refreshes after it copy again).
    pub(crate) fn defer(&self, work: impl Fn() + Send + 'static) {
        self.deferred().copies.push(Step::Run(Box::new(work)));
        self.deferred().refreshed.clear();
    }

    /// As [`HostList::defer`], for work that writes only host memory.
    pub(crate) fn defer_host(&self, work: impl Fn() + Send + 'static) {
        self.deferred().copies.push(Step::Host(Box::new(work)));
    }

    /// Brings a texture's staging buffer up to date when the list is
    /// submitted, keeping it alive until the list is reset.
    pub(crate) fn defer_texture(&self, staging: std::sync::Arc<crate::texture::Staging>) {
        self.deferred().copies.push(Step::Texture(staging));
    }

    /// Runs the list's submission-time steps; refreshes go into `batch`
    /// (the title's big buffers, a 174 MB one among them, are read by most
    /// lists of a submission, through many ranges).
    fn run_deferred(&self, batch: &mut Batch) {
        #[cfg(feature = "diagnostics")]
        use crate::copystat::{C, add};
        for step in &self.deferred().copies {
            let started = std::time::Instant::now();
            let copied = match step {
                &Step::Copy(dst, src, len) => {
                    #[cfg(feature = "diagnostics")]
                    {
                        add(C::Copies, 1);
                        add(C::CopyBytes, len as u64);
                    }
                    batch.title_written();
                    copy_mapped(dst as *mut u8, src as *const u8, len, true);
                    len
                }
                &Step::Fill(dst, src, len) => {
                    #[cfg(feature = "diagnostics")]
                    {
                        add(C::Copies, 1);
                        add(C::CopyBytes, len as u64);
                    }
                    copy_mapped(dst as *mut u8, src as *const u8, len, false);
                    len
                }
                Step::Refresh(shadow, address, len) => {
                    #[cfg(feature = "diagnostics")]
                    {
                        add(C::Refreshes, 1);
                        add(C::RefreshBytes, *len as u64);
                    }
                    batch.add(shadow, *address, *len);
                    continue;
                }
                Step::Run(work) | Step::Host(work) => {
                    if matches!(step, Step::Run(_)) {
                        batch.title_written();
                    }
                    work();
                    #[cfg(feature = "diagnostics")]
                    {
                        add(C::Runs, 1);
                        add(C::RunUs, started.elapsed().as_micros() as u64);
                    }
                    continue;
                }
                Step::GpuWrite(shadow, address, len) => {
                    batch.gpu_write(shadow, *address, *len);
                    continue;
                }
                Step::Texture(staging) => {
                    if batch.texture_due(staging) {
                        staging.refresh();
                    }
                    continue;
                }
            };
            PREPARING[0].fetch_add(started.elapsed().as_micros() as u64, Ordering::Relaxed);
            PREPARING[1].fetch_add(copied as u64, Ordering::Relaxed);
        }
    }
}

/// A submission's pending refreshes, grouped by buffer and nearby pages.
/// Flush them before a step writes title memory, before the buffer's
/// shader-write snapshot, and before the lists run.
///
/// Also the textures brought up to date in the submission so far: most
/// lists of a world submission sample the same textures, and a texture in
/// physical-page memory is hashed whole each time it is checked. Checked
/// once, it is checked again only after a step that writes title memory.
#[derive(Default)]
struct Batch {
    refreshes: Vec<(std::sync::Arc<crate::shadow::Shadow>, RefreshRanges)>,
    gpu_writes: Vec<(std::sync::Arc<crate::shadow::Shadow>, usize, usize)>,
    declared_writes: std::collections::HashSet<(usize, usize, usize)>,
    #[cfg(test)]
    updates: usize,
    /// Staging buffers (by address) refreshed since the last title write.
    textures: std::collections::HashSet<usize>,
}

/// Keep the common contiguous case inline. Sort disjoint spans once, when
/// a GPU write needs this shadow's snapshot or the submission flushes it.
struct RefreshRanges {
    first: (usize, usize),
    others: Vec<(usize, usize)>,
}

impl RefreshRanges {
    fn add(&mut self, from: usize, to: usize) {
        let last = self.others.last_mut().unwrap_or(&mut self.first);
        if from <= last.1 && last.0 <= to {
            *last = (last.0.min(from), last.1.max(to));
        } else {
            self.others.push((from, to));
        }
    }

    fn flush(mut self, shadow: &crate::shadow::Shadow) {
        if self.others.is_empty() {
            refresh_shadow(shadow, self.first.0, self.first.1);
            return;
        }
        self.others.push(self.first);
        self.others.sort_unstable();
        let (mut start, mut end) = self.others[0];
        for &(from, to) in &self.others[1..] {
            if from / crate::watch::PAGE <= end.div_ceil(crate::watch::PAGE) {
                end = end.max(to);
            } else {
                refresh_shadow(shadow, start, end);
                (start, end) = (from, to);
            }
        }
        refresh_shadow(shadow, start, end);
    }
}

impl Batch {
    fn new_gpu_write<T>(&mut self, shadow: &std::sync::Arc<T>, address: usize, len: usize) -> bool {
        self.declared_writes
            .insert((std::sync::Arc::as_ptr(shadow) as usize, address, len))
    }

    fn gpu_write(
        &mut self,
        shadow: &std::sync::Arc<crate::shadow::Shadow>,
        address: usize,
        len: usize,
    ) {
        if self.new_gpu_write(shadow, address, len) {
            // Only this host copy supplies the snapshot. Other copies can
            // keep merging refreshes until their own write or a title write.
            if let Some(index) = self
                .refreshes
                .iter()
                .position(|(pending, ..)| std::sync::Arc::ptr_eq(pending, shadow))
            {
                let (copy, ranges) = self.refreshes.remove(index);
                ranges.flush(&copy);
                #[cfg(test)]
                {
                    self.updates += 1;
                }
            }
            shadow.writing(address, len);
            self.gpu_writes.push((shadow.clone(), address, len));
        }
    }

    fn add(&mut self, shadow: &std::sync::Arc<crate::shadow::Shadow>, address: usize, len: usize) {
        if len == 0 {
            return;
        }
        let end = address.saturating_add(len);
        match self
            .refreshes
            .iter_mut()
            .find(|(s, ..)| std::sync::Arc::ptr_eq(s, shadow))
        {
            Some((_, ranges)) => ranges.add(address, end),
            None => self.refreshes.push((
                shadow.clone(),
                RefreshRanges {
                    first: (address, end),
                    others: Vec::new(),
                },
            )),
        }
    }

    /// Whether `staging` is still to be refreshed in this submission (and
    /// notes it refreshed).
    fn texture_due<T>(&mut self, staging: &std::sync::Arc<T>) -> bool {
        self.textures.insert(std::sync::Arc::as_ptr(staging) as *const () as usize)
    }

    /// Before a step that writes title memory: pending refreshes are done
    /// first, and textures are checked again after it.
    fn title_written(&mut self) {
        self.flush();
        self.textures.clear();
    }

    fn flush(&mut self) {
        #[cfg(test)]
        {
            self.updates += self.refreshes.len();
        }
        for (shadow, ranges) in self.refreshes.drain(..) {
            ranges.flush(&shadow);
        }
    }
}

fn refresh_shadow(shadow: &crate::shadow::Shadow, from: usize, to: usize) {
    let started = std::time::Instant::now();
    let copied = shadow.update(from, to - from);
    let us = started.elapsed().as_micros() as u64;
    #[cfg(feature = "diagnostics")]
    crate::copystat::range(from, to - from, us, 0);
    PREPARING[0].fetch_add(us, Ordering::Relaxed);
    PREPARING[1].fetch_add(copied as u64, Ordering::Relaxed);
}

/// Copies title memory at submission. The ranges were checked against their
/// resources when recorded, but the title may since have unmapped pages of
/// them (tiled memory, page mappings), which read as zeros here; a title
/// destination (`title_dst`) stops at its first unwritable page.
///
/// The kernel copy checks both source and destination pages. Querying a
/// region first can walk page tables across an entire title memory pool,
/// even for a small copy.
pub(crate) fn copy_mapped(dst: *mut u8, src: *const u8, len: usize, title_dst: bool) {
    #[cfg(feature = "diagnostics")]
    match crate::copystat::mode() {
        1 => {
            // SAFETY: timing only: the caller vouches for the source.
            unsafe { std::ptr::copy(src, dst, len) };
            return;
        }
        2 => return,
        _ => {}
    }
    if len == 0 {
        return;
    }
    if title_dst {
        // The kernel writes the destination: it must not be guarded.
        crate::watch::writing(dst as usize, len);
    }
    #[cfg(feature = "diagnostics")]
    let moved = std::time::Instant::now();
    let whole = read(src, dst, len);
    #[cfg(feature = "diagnostics")]
    crate::copystat::add(crate::copystat::C::MoveUs, moved.elapsed().as_micros() as u64);
    if whole {
        return;
    }
    #[cfg(feature = "diagnostics")]
    crate::copystat::add(crate::copystat::C::Fallbacks, 1);
    if title_dst {
        copy_title_pieces(src, dst, len);
    } else if len >= 64 << 10 {
        read_sparse(src, dst, len);
    } else {
        read_pieces(src, dst, len);
    }
}

/// Split at both page boundaries so zeroing an unreadable source page
/// cannot overwrite bytes successfully copied before a destination hole.
/// Use the kernel for the zero write too: the title can remap a page at
/// any time, and committed destination pages can still be read-only.
fn copy_title_pieces(src: *const u8, dst: *mut u8, len: usize) {
    const PAGE: usize = 4 << 10;
    static ZERO: [u8; PAGE] = [0; PAGE];
    let mut offset = 0;
    while offset < len {
        let from = src.wrapping_add(offset);
        let to = dst.wrapping_add(offset);
        let n = (len - offset)
            .min(PAGE - from as usize % PAGE)
            .min(PAGE - to as usize % PAGE);
        if !read(from, to, n) && !read(ZERO.as_ptr(), to, n) {
            break;
        }
        offset += n;
    }
}

/// After a failed bulk read into host memory, skip contiguous holes. Never
/// cache the query across copies: the title may remap the same address.
fn read_sparse(src: *const u8, dst: *mut u8, len: usize) -> usize {
    use windows_sys::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_NOACCESS, VirtualQuery,
    };
    let mut offset = 0;
    let mut copied = 0;
    while offset < len {
        let at = src.wrapping_add(offset);
        let to = dst.wrapping_add(offset);
        let remaining = len - offset;
        // SAFETY: the query inspects an address and fills a local structure.
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        let size = unsafe { VirtualQuery(at.cast(), &mut info, size_of_val(&info)) };
        let n = (info.BaseAddress as usize).saturating_add(info.RegionSize)
            .saturating_sub(at as usize).min(remaining);
        if size != size_of_val(&info) || n == 0 {
            copied += read_pieces(at, to, remaining);
            break;
        }
        if info.State != MEM_COMMIT || info.Protect == PAGE_NOACCESS {
            #[cfg(feature = "diagnostics")]
            crate::copystat::add(crate::copystat::C::Unmapped, n as u64);
            // SAFETY: this path is only used with a writable host destination.
            unsafe { to.write_bytes(0, n) };
        } else if !read(at, to, n) {
            // A mapping may change between query and read.
            copied += read_pieces(at, to, n);
        } else {
            copied += n;
        }
        offset += n;
    }
    copied
}

fn read_pieces(src: *const u8, dst: *mut u8, len: usize) -> usize {
    // Something is unmapped: page by page (64 KiB, then 4 KiB in a piece
    // that fails), zeros for what cannot be read.
    let mut copied = 0;
    for_pieces(src as usize, len, 64 << 10, |at, n| {
        let to = dst.wrapping_add(at - src as usize);
        if !read(at as *const u8, to, n) {
            for_pieces(at, n, 4 << 10, |at, n| {
                let to = dst.wrapping_add(at - src as usize);
                if !read(at as *const u8, to, n) {
                    #[cfg(feature = "diagnostics")]
                    crate::copystat::add(crate::copystat::C::Unmapped, n as u64);
                    // SAFETY: inside the destination, which is mapped.
                    unsafe { to.write_bytes(0, n) };
                } else {
                    copied += n;
                }
            });
        } else {
            copied += n;
        }
    });
    copied
}

/// Takes a kernel-checked snapshot with zeros for unreadable pages.
pub(crate) fn read_mapped(src: *const u8, dst: &mut [u8]) -> usize {
    let len = dst.len();
    if len == 0 || read_into(src, dst) {
        len
    } else if len >= 64 << 10 {
        read_sparse(src, dst.as_mut_ptr(), len)
    } else {
        read_pieces(src, dst.as_mut_ptr(), len)
    }
}

/// Reads title memory at `src` into `dst`; false if a page of it is not
/// readable.
pub(crate) fn read_into(src: *const u8, dst: &mut [u8]) -> bool {
    read(src, dst.as_mut_ptr(), dst.len())
}

/// Copies `len` bytes in the kernel. Returns false on an unreadable source
/// or unwritable destination page. Bytes before that page may have arrived.
fn read(src: *const u8, dst: *mut u8, len: usize) -> bool {
    use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut done = 0;
    // SAFETY: the caller permits writes to the destination range. The
    // kernel checks both ranges and handles faults during the copy.
    unsafe { ReadProcessMemory(GetCurrentProcess(), src.cast(), dst.cast(), len, &mut done) != 0 }
}

/// Copies host memory into title memory after a submission (what shaders
/// wrote), leaving pages that are not mapped alone.
pub(crate) fn write_mapped(dst: *mut u8, src: *const u8, len: usize) {
    let write = |at: usize, n: usize| read(src.wrapping_add(at - dst as usize), at as *mut u8, n);
    if len == 0 || write(dst as usize, len) {
        return;
    }
    for_pieces(dst as usize, len, 64 << 10, |at, n| {
        if !write(at, n) {
            for_pieces(at, n, 4 << 10, |at, n| {
                write(at, n);
            });
        }
    });
}

/// Calls `f` with the pieces of `[start, start + len)` cut at multiples of
/// `step`.
fn for_pieces(start: usize, len: usize, step: usize, mut f: impl FnMut(usize, usize)) {
    let end = start + len;
    let mut at = start;
    while at < end {
        let next = ((at / step + 1) * step).min(end);
        f(at, next - at);
        at = next;
    }
}

#[cfg(test)]
mod title_copies {
    use super::{copy_mapped, write_mapped};
    use windows_sys::Win32::System::Memory::{
        MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_NOACCESS, PAGE_READONLY, PAGE_READWRITE,
        VirtualAlloc, VirtualFree, VirtualProtect,
    };

    const BLOCK: usize = 64 << 10;

    /// Three reserved blocks, the first and the last committed and filled
    /// with `fill`: title memory with an unmapped block in it.
    fn holed(fill: u8) -> *mut u8 {
        // SAFETY: a fresh reservation; the committed blocks are ours to fill.
        unsafe {
            let base =
                VirtualAlloc(std::ptr::null(), 3 * BLOCK, MEM_RESERVE, PAGE_NOACCESS).cast::<u8>();
            assert!(!base.is_null());
            for block in [0, 2] {
                let at = base.add(block * BLOCK);
                assert!(!VirtualAlloc(at.cast(), BLOCK, MEM_COMMIT, PAGE_READWRITE).is_null());
                at.write_bytes(fill, BLOCK);
            }
            base
        }
    }

    fn block<'a>(base: *const u8, n: usize) -> &'a [u8] {
        // SAFETY: tests ask for committed blocks only.
        unsafe { std::slice::from_raw_parts(base.add(n * BLOCK), BLOCK) }
    }

    fn release(base: *mut u8) {
        // SAFETY: a reservation of `holed`.
        assert_ne!(unsafe { VirtualFree(base.cast(), 0, MEM_RELEASE) }, 0);
    }

    fn pattern() -> Vec<u8> {
        (0..3 * BLOCK).map(|i| (i % 251) as u8 + 1).collect()
    }

    #[test]
    fn title_copies_reach_mapped_destinations_and_stop_at_unmapped_pages() {
        let from = pattern();
        // All mapped: copied, through a destination that was never asked.
        let to = holed(0xee);
        copy_mapped(to, from.as_ptr(), BLOCK, true);
        assert_eq!(block(to, 0), &from[..BLOCK]);
        release(to);
        // An unmapped block in the destination: nothing faults, and nothing
        // arrives from that block on.
        let to = holed(0xee);
        copy_mapped(to, from.as_ptr(), 3 * BLOCK, true);
        assert!(block(to, 2).iter().all(|&b| b == 0xee));
        release(to);
        // An unmapped block in the source reads as zeros.
        let holes = holed(7);
        let mut to = vec![0xeeu8; 3 * BLOCK];
        copy_mapped(to.as_mut_ptr(), holes, 3 * BLOCK, true);
        assert!(to[..BLOCK].iter().all(|&b| b == 7));
        assert!(to[BLOCK..2 * BLOCK].iter().all(|&b| b == 0));
        assert!(to[2 * BLOCK..].iter().all(|&b| b == 7));
        release(holes);
    }

    #[test]
    fn title_copies_stop_at_a_read_only_destination_with_unaligned_source() {
        let from = pattern();
        let to = holed(0xee);
        let mut old = 0;
        // SAFETY: a reservation of `holed`. The first block remains
        // committed, with its last page read-only.
        unsafe {
            assert_ne!(
                VirtualProtect(to.add(BLOCK - 4096).cast(), 4096, PAGE_READONLY, &mut old),
                0
            );
            copy_mapped(to.add(3), from.as_ptr().add(29), BLOCK - 3, true);
        }
        let first = block(to, 0);
        assert_eq!(&first[..3], &[0xee; 3]);
        assert_eq!(&first[3..BLOCK - 4096], &from[29..BLOCK - 4096 + 26]);
        assert!(first[BLOCK - 4096..].iter().all(|&b| b == 0xee));
        release(to);
    }

    #[test]
    fn copy_back_writes_mapped_pages_only() {
        let from = pattern();
        let to = holed(0xee);
        write_mapped(to, from.as_ptr(), 3 * BLOCK);
        assert_eq!(block(to, 0), &from[..BLOCK]);
        assert_eq!(block(to, 2), &from[2 * BLOCK..]);
        release(to);
        // Neither aligned nor whole: the bytes around the range stay.
        let to = holed(0xee);

        // SAFETY: inside the first block.
        write_mapped(unsafe { to.add(100) }, from.as_ptr(), BLOCK - 300);
        let first = block(to, 0);
        assert_eq!(first[100..BLOCK - 200], from[..BLOCK - 300]);
        let around = first[..100].iter().chain(&first[BLOCK - 200..]);
        assert!(around.into_iter().all(|&b| b == 0xee));
        // A page that is mapped but not writable is not written either
        // (guarded title blocks are opened before they are copied back).
        let mut old = 0;
        // SAFETY: a committed block of ours.
        let protected =
            unsafe { VirtualProtect(to.add(2 * BLOCK).cast(), BLOCK, PAGE_READONLY, &mut old) };
        assert_ne!(protected, 0);
        // SAFETY: the third block.
        write_mapped(unsafe { to.add(2 * BLOCK) }, from.as_ptr(), BLOCK);
        assert!(block(to, 2).iter().all(|&b| b == 0xee));
        release(to);
    }
}

impl Host {
    /// A buffer in the upload heap, mapped for writing.
    pub(crate) fn upload_buffer(&self, size: u64) -> Option<(ID3D12Resource, *mut u8)> {
        let heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_UPLOAD,
            ..Default::default()
        };
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: size.max(1),
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            ..Default::default()
        };
        let mut buffer: Option<ID3D12Resource> = None;
        let mut data = std::ptr::null_mut();
        // SAFETY: plain creation and mapping of an upload buffer.
        let made = unsafe {
            self.device
                .CreateCommittedResource(
                    &heap,
                    D3D12_HEAP_FLAG_NONE,
                    &desc,
                    D3D12_RESOURCE_STATE_GENERIC_READ,
                    None,
                    &mut buffer,
                )
                .and_then(|()| {
                    let b = buffer.as_ref().ok_or_else(windows::core::Error::empty)?;
                    b.Map(0, None, Some(&mut data))
                })
        };
        match made {
            Ok(()) => Some((buffer?, data.cast())),
            Err(e) => {
                tracing::warn!(error = %e, size, "no upload buffer");
                None
            }
        }
    }
}

/// The swap chain formats a presented texture can be copied into: its own
/// family's.
fn swap_format(format: DXGI_FORMAT) -> Option<DXGI_FORMAT> {
    Some(match format.0 {
        27..=32 => DXGI_FORMAT_R8G8B8A8_UNORM,
        87 | 90 | 91 => DXGI_FORMAT_B8G8R8A8_UNORM,
        23..=25 => DXGI_FORMAT_R10G10B10A2_UNORM,
        9..=14 => DXGI_FORMAT_R16G16B16A16_FLOAT,
        _ => return None,
    })
}

const BUFFERS: u32 = 2;

static OUTPUT_HEIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Sets the host presentation height limit for this launch. This does not
/// change the title's output mode or the sizes of its render targets.
pub fn set_output_height(height: Option<std::num::NonZeroU32>) {
    OUTPUT_HEIGHT.store(
        height.map_or(0, std::num::NonZeroU32::get),
        Ordering::Relaxed,
    );
}

fn cap_extent((width, height): (u32, u32), limit: Option<std::num::NonZeroU32>) -> (u32, u32) {
    let Some(limit) = limit
        .map(std::num::NonZeroU32::get)
        .filter(|&limit| limit < height)
    else {
        return (width, height);
    };
    let scale = (u64::from(width) * u64::from(limit) + u64::from(height / 2)) / u64::from(height);
    (scale.max(1) as u32, limit)
}

type PresentationPlan<'a> = ((u32, u32), Option<Vec<compositor::Plane<'a>>>);

/// Select the drawing path before creating or resizing the swap chain.
fn plan_presentation<'a>(
    device: &ID3D12Device,
    primary: D3D12_RESOURCE_DESC,
    planes: &[compositor::PresentSource<'a>],
    limit: Option<std::num::NonZeroU32>,
    compositor_ready: impl FnOnce() -> bool,
) -> Option<PresentationPlan<'a>> {
    let primary_size = (u32::try_from(primary.Width).ok()?, primary.Height);
    let capped_primary = cap_extent(primary_size, limit);
    if capped_primary != primary_size
        || planes.len() > 1
        || planes
            .first()
            .is_some_and(|p| p.2 != PresentGeometry::default())
    {
        let sampled = compositor::prepare_planes(device, planes).or_else(|| {
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                tracing::warn!("cannot sample these present planes; trying the primary plane");
            }
            if planes.len() > 1 && capped_primary != primary_size {
                compositor::prepare_planes(device, &planes[..1])
            } else {
                None
            }
        });
        if let Some(sampled) = sampled
            && compositor_ready()
        {
            let extent = sampled.iter().skip(1).fold(primary_size, |(w, h), plane| {
                (w.max(plane.size.0), h.max(plane.size.1))
            });
            return Some((cap_extent(extent, limit), Some(sampled)));
        }
        if capped_primary != primary_size {
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    requested_height = capped_primary.1,
                    primary_height = primary_size.1,
                    "output height limit unavailable; trying the native primary plane"
                );
            }
        }
    }
    // CopyResource needs a single 2D subresource matching the swap buffer.
    let copyable = primary.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D
        && primary.DepthOrArraySize == 1
        && primary.MipLevels == 1
        && primary.SampleDesc.Count == 1
        && primary.SampleDesc.Quality == 0
        && primary_size.0 != 0
        && primary_size.1 != 0;
    copyable.then_some((primary_size, None))
}

#[derive(Default)]
struct Presentation {
    presenter: Option<Presenter>,
    compositor: Option<Compositor>,
    failures: Vec<(DXGI_FORMAT, Option<std::time::Instant>)>,
}

struct Presenter {
    swap_chain: IDXGISwapChain3,
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
    copy: HostList,
    hwnd: HWND,
    meter: Meter,
}

/// The frame rate, shown in the window's caption every second and logged
/// every few seconds.
struct Meter {
    since: std::time::Instant,
    frames: u32,
    drawn: u64,
    logged: std::time::Instant,
    /// Frames since the copy counters were last drained for a log report.
    #[cfg(feature = "diagnostics")]
    copy_frames: u64,
    /// The caption the window had, once read.
    caption: Option<Vec<u16>>,
}

impl Meter {
    fn new() -> Self {
        let now = std::time::Instant::now();
        Meter {
            since: now,
            frames: 0,
            drawn: crate::draw::drawn(),
            logged: now,
            #[cfg(feature = "diagnostics")]
            copy_frames: 0,
            caption: None,
        }
    }

    /// Counts a presented frame.
    fn frame(&mut self, hwnd: HWND) {
        #[cfg(feature = "diagnostics")]
        tracing::debug!(target: "victory_gpu::frame", token = crate::frame::current(), "present");
        self.frames += 1;
        let elapsed = self.since.elapsed();
        if elapsed < std::time::Duration::from_secs(1) {
            return;
        }
        let drawn = crate::draw::drawn();
        #[cfg(feature = "diagnostics")]
        {
            self.copy_frames = self.copy_frames.saturating_add(u64::from(self.frames));
        }
        let frames = f64::from(self.frames);
        let fps = frames / elapsed.as_secs_f64();
        let ms = elapsed.as_secs_f64() * 1000.0 / frames;
        let draws = (drawn - self.drawn) as f64 / frames;
        (self.since, self.frames, self.drawn) = (std::time::Instant::now(), 0, drawn);
        // Of a frame, ms preparing submissions and ms waiting for the GPU.
        let [cpu, gpu] = [0, 1].map(|i| {
            SUBMISSION_US[i].swap(0, Ordering::Relaxed) as f64 / 1000.0 / frames
        });
        let [copy_ms, copy_mb, texture_ms, texture_mb] = [0, 1, 2, 3].map(|i| {
            let scale = if i % 2 == 0 { 1e3 } else { 1e6 };
            PREPARING[i].swap(0, Ordering::Relaxed) as f64 / scale / frames
        });
        let [back_ms, back_mb] = [0, 1].map(|i| {
            COPY_BACK[i].swap(0, Ordering::Relaxed) as f64 / [1e3, 1e6][i] / frames
        });
        let dropped = crate::draw::take_dropped() as f64 / frames;
        let indirect = crate::list::take_indirect() as f64 / frames;
        // Over the same frames as the counts above.
        #[cfg(feature = "diagnostics")]
        let drops = crate::pipeline::take_drops();
        if self.logged.elapsed() >= std::time::Duration::from_secs(5) {
            self.logged = std::time::Instant::now();
            tracing::info!(
                "{fps:.1} fps, {ms:.1} ms/frame ({cpu:.1} preparing: copies {copy_ms:.1} ms \
                 {copy_mb:.1} MB, textures {texture_ms:.1} ms {texture_mb:.1} MB; {gpu:.1} on the \
                 GPU; {back_ms:.1} copying back {back_mb:.1} MB), {draws:.0} draws/frame, \
                 dropped {dropped:.0} without a host pipeline ({making} being made) and \
                 {indirect:.0} indirect",
                making = crate::pipeline::making()
            );
            #[cfg(feature = "diagnostics")]
            {
                crate::copystat::log(std::mem::take(&mut self.copy_frames) as f64);
                for (why, n) in drops.iter().take(40) {
                    tracing::info!(
                        "dropped {:.1}/frame without a host pipeline: {why}",
                        *n as f64 / frames
                    );
                }
            }
        }
        if self.caption.is_none() {
            // The window's own text, read without a message.
            let mut buffer = vec![0u16; 512];
            // SAFETY: a window handle and a buffer of that length.
            let n = unsafe { InternalGetWindowText(hwnd, &mut buffer) };
            buffer.truncate(n.max(0) as usize);
            self.caption = Some(buffer);
        }
        let Some(caption) = &self.caption else { return };
        let text = format!(" | {fps:.0} fps {ms:.1} ms/frame {draws:.0} draws/frame");
        let mut text: Vec<u16> = caption.iter().copied().chain(text.encode_utf16()).collect();
        text.push(0);
        // Setting the text waits for the window's thread (a title thread,
        // which may not pump for a while), so a thread of ours does it and
        // presenting never waits; it takes the newest text when free.
        static PENDING: Mutex<Option<(usize, Vec<u16>)>> = Mutex::new(None);
        static SETTER: std::sync::Once = std::sync::Once::new();
        *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((hwnd.0 as usize, text));
        SETTER.call_once(|| {
            let _ = std::thread::Builder::new()
                .name("victory-caption".into())
                .spawn(|| {
                    loop {
                        let next = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
                        match next {
                            // SAFETY: a window handle and a NUL-terminated text.
                            Some((hwnd, text)) => unsafe {
                                let _ = SetWindowTextW(HWND(hwnd as *mut _), PCWSTR(text.as_ptr()));
                            },
                            None => std::thread::sleep(std::time::Duration::from_millis(100)),
                        }
                    }
                });
        });
    }
}

impl Host {
    /// Shows one or more `planes`, keeping their host states intact and
    /// compositing planes 1.. on top of
    /// plane 0 when multiple planes are presented, as frame `frame` (saved if
    /// captures are on). `None` if nothing was shown; else, in diagnostics
    /// builds, whether the frame was black.
    pub(crate) fn present(
        &self,
        textures: &[(std::sync::Arc<crate::resource::HostTexture>, PresentGeometry)],
        frame: u64,
    ) -> Option<(bool, u32)> {
        // Trackers are read and GPU states restored entirely under the same
        // lock used by ordinary submission preludes.
        let mut submitted = self.submitted.lock().unwrap_or_else(|e| e.into_inner());
        let planes: Vec<_> = textures
            .iter()
            .map(|(host, geometry)| (&host.texture, host.state.load(Ordering::Relaxed), *geometry))
            .collect();
        let &(primary, primary_state, _) = planes.first()?;
        // SAFETY: live resources.
        let primary_desc = unsafe { primary.GetDesc() };
        let Some(format) = swap_format(primary_desc.Format) else {
            tracing::warn!(format = primary_desc.Format.0, "cannot present this format");
            return None;
        };
        let mut presentation = self.presentation.lock().unwrap_or_else(|e| e.into_inner());
        let Presentation {
            presenter,
            compositor,
            failures,
        } = &mut *presentation;
        let ((width, height), composition) = plan_presentation(
            &self.device,
            primary_desc,
            &planes,
            std::num::NonZeroU32::new(OUTPUT_HEIGHT.load(Ordering::Relaxed)),
            || {
                if compositor.as_ref().is_some_and(|c| c.format == format) {
                    return true;
                }
                let now = std::time::Instant::now();
                failures.retain(|&(_, retry)| retry.is_none_or(|retry| now < retry));
                if failures.iter().any(|&(failed, _)| failed == format) {
                    return false;
                }
                match Compositor::new(&self.device, format) {
                    Ok(comp) => {
                        *compositor = Some(comp);
                        true
                    }
                    Err(e) => {
                        use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL};
                        let retry = if matches!(e.code(), E_INVALIDARG | E_NOTIMPL) {
                            None
                        } else {
                            Some(now + std::time::Duration::from_secs(5))
                        };
                        failures.push((format, retry));
                        tracing::warn!(error = %e, "no present compositor; trying the primary plane");
                        false
                    }
                }
            },
        )?;
        if presenter.is_none() {
            *presenter = match self.presenter(width, height, format) {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::warn!(error = %e, "no swap chain");
                    return None;
                }
            };
        }
        let p = presenter.as_mut()?;
        if (p.width, p.height, p.format) != (width, height, format) {
            // SAFETY: no back buffer is held; every submission was waited for.
            let resized = unsafe {
                p.swap_chain
                    .ResizeBuffers(BUFFERS, width, height, format, DXGI_SWAP_CHAIN_FLAG(0))
            };
            match resized {
                Ok(()) => (p.width, p.height, p.format) = (width, height, format),
                Err(e) => {
                    tracing::warn!(error = %e, width, height, "swap chain not resized");
                    return None;
                }
            }
        }
        // SAFETY: our swap chain, list and live source textures.
        unsafe {
            let index = p.swap_chain.GetCurrentBackBufferIndex();
            let Ok(back) = p.swap_chain.GetBuffer::<ID3D12Resource>(index) else {
                return None;
            };
            p.copy.reset();
            let list = &p.copy.list;
            let (capture, probe) = match composition.as_deref() {
                None => {
                    let (texture, state, _) = planes[0];
                    let back_desc = back.GetDesc();
                    if back_desc.Width != primary_desc.Width
                        || back_desc.Height != primary_desc.Height
                        || back_desc.Dimension != primary_desc.Dimension
                        || back_desc.DepthOrArraySize != primary_desc.DepthOrArraySize
                        || back_desc.MipLevels != primary_desc.MipLevels
                        || back_desc.SampleDesc != primary_desc.SampleDesc
                        || swap_format(back_desc.Format) != Some(format)
                    {
                        tracing::warn!(
                            "primary plane and swap buffer do not match; skipping presentation copy"
                        );
                        return None;
                    }
                    let state = D3D12_RESOURCE_STATES(state as i32);
                    let mut before = Vec::with_capacity(2);
                    if state != D3D12_RESOURCE_STATE_COPY_SOURCE {
                        before.push(transition(texture, state, D3D12_RESOURCE_STATE_COPY_SOURCE));
                    }
                    before.push(transition(
                        &back,
                        D3D12_RESOURCE_STATE_PRESENT,
                        D3D12_RESOURCE_STATE_COPY_DEST,
                    ));
                    list.ResourceBarrier(&before);
                    list.CopyResource(&back, texture);
                    #[cfg(feature = "diagnostics")]
                    let capture = crate::capture::wanted(frame)
                        .then(|| crate::capture::read_back(&self.device, list, texture))
                        .flatten();
                    #[cfg(feature = "diagnostics")]
                    let probe = crate::capture::probe(&self.device, list, texture);
                    #[cfg(not(feature = "diagnostics"))]
                    let (capture, probe) = {
                        let _ = frame;
                        (None::<()>, None::<()>)
                    };
                    let mut after = Vec::with_capacity(2);
                    if state != D3D12_RESOURCE_STATE_COPY_SOURCE {
                        after.push(transition(texture, D3D12_RESOURCE_STATE_COPY_SOURCE, state));
                    }
                    after.push(transition(
                        &back,
                        D3D12_RESOURCE_STATE_COPY_DEST,
                        D3D12_RESOURCE_STATE_PRESENT,
                    ));
                    list.ResourceBarrier(&after);
                    (capture, probe)
                }
                Some(planes) => {
                    let comp = compositor.as_ref().expect("prepared compositor");
                    comp.record(
                        &self.device,
                        list,
                        &back,
                        D3D12_RESOURCE_STATE_PRESENT,
                        planes,
                    );
                    #[cfg(feature = "diagnostics")]
                    let (capture, probe) = {
                        list.ResourceBarrier(&[transition(
                            &back,
                            D3D12_RESOURCE_STATE_RENDER_TARGET,
                            D3D12_RESOURCE_STATE_COPY_SOURCE,
                        )]);
                        let capture = crate::capture::wanted(frame)
                            .then(|| crate::capture::read_back(&self.device, list, &back))
                            .flatten();
                        let probe = crate::capture::probe(&self.device, list, &back);
                        list.ResourceBarrier(&[transition(
                            &back,
                            D3D12_RESOURCE_STATE_COPY_SOURCE,
                            D3D12_RESOURCE_STATE_PRESENT,
                        )]);
                        (capture, probe)
                    };
                    #[cfg(not(feature = "diagnostics"))]
                    let (capture, probe) = {
                        let _ = frame;
                        list.ResourceBarrier(&[transition(
                            &back,
                            D3D12_RESOURCE_STATE_RENDER_TARGET,
                            D3D12_RESOURCE_STATE_PRESENT,
                        )]);
                        (None::<()>, None::<()>)
                    };
                    (capture, probe)
                }
            };
            p.copy.close();
            let _ = self.execute_locked(&[&p.copy], &mut submitted);
            #[cfg(feature = "diagnostics")]
            if let Some((buffer, footprint)) = capture {
                crate::capture::save_read_back(frame, &buffer, &footprint);
            }
            let _ = p.swap_chain.Present(0, DXGI_PRESENT(0));
            p.meter.frame(p.hwnd);
            #[cfg(feature = "diagnostics")]
            return Some((probe.is_some_and(|b| crate::capture::black(&b)), primary_state));
            #[cfg(not(feature = "diagnostics"))]
            {
                let _ = (capture, probe);
                Some((false, primary_state))
            }
        }
    }

    fn presenter(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> windows::core::Result<Presenter> {
        let hwnd = match title_window() {
            Some(hwnd) => hwnd,
            None => window(width, height)?,
        };
        // SAFETY: plain object creation for our window.
        unsafe {
            let factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: BUFFERS,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                ..Default::default()
            };
            let swap_chain = factory
                .CreateSwapChainForHwnd(&self.queue, hwnd, &desc, None, None)?
                .cast::<IDXGISwapChain3>()?;
            let copy = self.new_list()?;
            copy.close();
            tracing::info!(width, height, format = format.0, "presenting to a window");
            Ok(Presenter {
                swap_chain,
                width,
                height,
                format,
                copy,
                hwnd,
                meter: Meter::new(),
            })
        }
    }
}

/// A visible top-level window of this process: the one the title made, if
/// it made one.
fn title_window() -> Option<HWND> {
    extern "system" fn visit(hwnd: HWND, found: LPARAM) -> windows::core::BOOL {
        let mut process = 0;
        // SAFETY: a window handle from the enumeration; `found` is ours.
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process));
            if process == windows::Win32::System::Threading::GetCurrentProcessId()
                && IsWindowVisible(hwnd).as_bool()
            {
                *(found.0 as *mut Option<HWND>) = Some(hwnd);
                return false.into();
            }
        }
        true.into()
    }
    let mut found: Option<HWND> = None;
    // SAFETY: the callback only writes `found`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(visit), LPARAM(&raw mut found as isize)) };
    found
}

/// `state` without the bits the host texture's flags forbid: the console
/// lets the title name states its own resources allow (unordered access on
/// a depth target, whose host copy cannot have both).
pub(crate) fn legal_state(texture: &ID3D12Resource, state: u32) -> u32 {
    // SAFETY: a plain query.
    legal_for(unsafe { texture.GetDesc() }.Flags.0 as u32, state)
}

/// [`legal_state`] for a texture with resource flags `flags`.
pub(crate) fn legal_for(flags: u32, state: u32) -> u32 {
    // (flag, set or clear forbids, state bits): render target, unordered
    // access, depth, denied shader resource.
    const FORBIDDEN: [(u32, bool, u32); 4] = [
        (0x1, false, 0x4),
        (0x4, false, 0x8),
        (0x2, false, 0x10 | 0x20),
        (0x8, true, 0x40 | 0x80),
    ];
    FORBIDDEN
        .iter()
        .filter(|&&(flag, deny, _)| (flags & flag != 0) == deny)
        .fold(state, |s, &(_, _, bits)| s & !bits)
}

impl Host {
    /// A buffer in system memory that the CPU maps (write-back, so reading
    /// it is fast too) and shaders read and write: for host copies of title
    /// memory that shaders write, which upload memory cannot be. Starts in
    /// the common state (buffers move out of it implicitly).
    pub(crate) fn shared_buffer(&self, size: u64) -> Option<(ID3D12Resource, *mut u8)> {
        let heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_CUSTOM,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
            MemoryPoolPreference: D3D12_MEMORY_POOL_L0,
            ..Default::default()
        };
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: size.max(1),
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
            ..Default::default()
        };
        let mut buffer: Option<ID3D12Resource> = None;
        let mut data = std::ptr::null_mut();
        // SAFETY: plain creation and mapping of a CPU-visible buffer.
        let made = unsafe {
            self.device
                .CreateCommittedResource(
                    &heap,
                    D3D12_HEAP_FLAG_NONE,
                    &desc,
                    D3D12_RESOURCE_STATE_COMMON,
                    None,
                    &mut buffer,
                )
                .and_then(|()| {
                    let b = buffer.as_ref().ok_or_else(windows::core::Error::empty)?;
                    b.Map(0, None, Some(&mut data))
                })
        };
        match made {
            Ok(()) => Some((buffer?, data.cast())),
            Err(e) => {
                tracing::warn!(error = %e, size, "no shared buffer");
                None
            }
        }
    }
}

/// For adapters without device-removed data (the software one): the last
/// commands each list of the failed submission noted (diagnostics, with
/// `VICTORY_D3D12_DRED`).
fn last_notes(runs: &[&HostList]) -> String {
    #[cfg(feature = "diagnostics")]
    {
        let mut text = String::new();
        for (i, r) in runs.iter().enumerate() {
            let notes = r.notes.lock().unwrap_or_else(|e| e.into_inner());
            for (n, _) in notes.iter().rev().take(6).rev() {
                text.push_str(&format!("; list {i} of {}, of its {}: {n}", runs.len(), notes.len()));
            }
        }
        text
    }
    #[cfg(not(feature = "diagnostics"))]
    {
        let _ = runs;
        String::new()
    }
}

/// Where the faulting address `va` lies against a range `(start, bytes)` a
/// command reads (diagnostics).
#[cfg(feature = "diagnostics")]
fn fault_in(va: u64, (start, bytes): (u64, u64)) -> String {
    let end = start + bytes;
    if va < start {
        format!(" (fault {:#x} below its source)", start - va)
    } else if va < end {
        format!(" (fault at source +{:#x})", va - start)
    } else {
        format!(" (fault {:#x} past its source's end)", va - end)
    }
}

/// The commands that may have been running where a list stopped at
/// command `done`: those between the barriers around it (work between
/// barriers overlaps on the GPU), by kind, and what the markers just
/// before them say (`HostList::note_op`: diagnostics builds with
/// device-removed data on).
fn in_flight(history: &[D3D12_AUTO_BREADCRUMB_OP], markers: &[(u32, String)], done: usize) -> String {
    let barrier = |op: &D3D12_AUTO_BREADCRUMB_OP| *op == D3D12_AUTO_BREADCRUMB_OP_RESOURCEBARRIER;
    let from = history[..done].iter().rposition(barrier).map_or(0, |b| b + 1);
    let to = history[done..]
        .iter()
        .position(barrier)
        .map_or(history.len(), |b| (done + b).max(done + 1));
    let mut text = format!(", commands {from}..{to} between its barriers:");
    let mut shown = 0;
    for i in from..to {
        let op = history[i];
        if op == D3D12_AUTO_BREADCRUMB_OP_SETMARKER || shown == 12 {
            continue;
        }
        shown += 1;
        let marker = i
            .checked_sub(1)
            .filter(|&m| history[m] == D3D12_AUTO_BREADCRUMB_OP_SETMARKER)
            .and_then(|m| markers.iter().find(|(at, _)| *at as usize == m))
            .map_or("no marker", |(_, s)| s.as_str());
        text.push_str(&format!("; command {i} (op {}): {marker}", op.0));
    }
    text
}

pub(crate) fn transition(
    resource: &ID3D12Resource,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    transition_subresource(
        resource,
        D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
        before,
        after,
    )
}

/// Mip indices directly address the first plane of these color targets.
pub(crate) fn color_mip_count(texture: &ID3D12Resource) -> Option<u32> {
    // SAFETY: the host texture is live.
    let desc = unsafe { texture.GetDesc() };
    (desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D
        && desc.DepthOrArraySize == 1
        && desc.MipLevels > 1
        && desc.Flags.contains(D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET))
    .then_some(u32::from(desc.MipLevels))
}

pub(crate) fn transition_subresource(
    resource: &ID3D12Resource,
    subresource: u32,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                // SAFETY: a borrowed pointer; the barrier never releases it.
                pResource: unsafe { std::mem::transmute_copy(resource) },
                Subresource: subresource,
                StateBefore: before,
                StateAfter: after,
            }),
        },
    }
}

/// A window of the given client size, owned by its own message thread.
fn window(width: u32, height: u32) -> windows::core::Result<HWND> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("victory-window".into())
        .spawn(move || {
            let made = make_window(width, height);
            let ok = made.is_ok();
            let _ = sender.send(made.map(|h| h.0 as usize));
            if ok {
                pump();
            }
        })
        .map_err(|_| windows::core::Error::empty())?;
    let hwnd = receiver
        .recv()
        .map_err(|_| windows::core::Error::empty())??;
    Ok(HWND(hwnd as *mut _))
}

extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: the default handling for our window.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn make_window(width: u32, height: u32) -> windows::core::Result<HWND> {
    const CLASS: PCWSTR = w!("VIctory");
    // SAFETY: registers our class once and creates a window of it.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        RegisterClassExW(&class);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, false)?;
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            w!("VI Causeway"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect.right - rect.left,
            rect.bottom - rect.top,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        Ok(hwnd)
    }
}

fn pump() {
    let mut message = MSG::default();
    // SAFETY: the standard message loop of this thread's window.
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "diagnostics")]
    #[test]
    fn warp_target_readback_retains_backing_and_restores_render_target_state() {
        use super::*;
        use windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
        };
        let host = create_device(true).expect("software WARP host");
        if std::env::var_os("VICTORY_D3D12_DEBUG").is_some() {
            assert!(
                host.device.cast::<ID3D12InfoQueue>().is_ok(),
                "D3D12 debug layer"
            );
        }
        let mut texture = None;
        // SAFETY: the output pointer and both descriptions are valid.
        unsafe {
            host.device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                    Width: 4,
                    Height: 2,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                None,
                &mut texture,
            )
        }
        .unwrap();
        let target = std::sync::Arc::new(crate::resource::HostTexture {
            texture: texture.unwrap(),
            state: std::sync::atomic::AtomicU32::new(D3D12_RESOURCE_STATE_RENDER_TARGET.0 as u32),
        });
        let retained = target.clone();
        drop(target);
        let clear = |color| {
            let list = host.new_list().unwrap();
            let rtv = host.rtv(Some(&retained.texture), None);
            // SAFETY: a valid RTV to the retained target in RENDER_TARGET state.
            unsafe { list.list.ClearRenderTargetView(rtv, &color, None) };
            list.close();
            assert_eq!(host.execute_checked(&[&list]), None);
        };
        let pixels = || {
            let (read, footprint) = host
                .read_target(&retained)
                .expect("checked diagnostic readback");
            let mut data = std::ptr::null_mut();
            // SAFETY: the fence completed and both rows are inside the footprint.
            unsafe {
                read.Map(0, None, Some(&mut data)).unwrap();
                let start = data.cast::<u8>().add(footprint.Offset as usize);
                let a = start.cast::<[u8; 4]>().read();
                let b = start
                    .add(footprint.Footprint.RowPitch as usize)
                    .cast::<[u8; 4]>()
                    .read();
                read.Unmap(0, None);
                [a, b]
            }
        };
        clear([1.0, 0.0, 0.0, 1.0]);
        assert_eq!(pixels(), [[255, 0, 0, 255]; 2]);
        assert_eq!(
            retained.state.load(Ordering::Relaxed),
            D3D12_RESOURCE_STATE_RENDER_TARGET.0 as u32
        );
        clear([0.0, 1.0, 0.0, 1.0]);
        assert_eq!(pixels(), [[0, 255, 0, 255]; 2]);
    }

    #[test]
    fn sparse_copies_preserve_islands_and_observe_remaps() {
        use windows_sys::Win32::System::Memory::*;
        const N: usize = 5 << 16;
        // SAFETY: we reserve a private region, commit selected pages inside it,
        // and only directly access the pages while they are writable.
        unsafe {
            let base = VirtualAlloc(std::ptr::null(), N, MEM_RESERVE, PAGE_NOACCESS).cast::<u8>();
            assert!(!base.is_null());
            let mut expected = vec![0; N];
            for (offset, value) in [(0, 9), (2 << 16, 37), (N - 4096, 85)] {
                let at = base.add(offset);
                assert_eq!(VirtualAlloc(at.cast(), 4096, MEM_COMMIT, PAGE_READWRITE), at.cast());
                at.write_bytes(value, 4096);
                expected[offset..offset + 4096].fill(value);
            }
            for remap in [false, true] {
                if remap {
                    let mut old = 0;
                    assert_ne!(VirtualProtect(base.cast(), 4096, PAGE_NOACCESS, &mut old), 0);
                    expected[..4096].fill(0);
                    let at = base.add(1 << 16);
                    assert_eq!(VirtualAlloc(at.cast(), 4096, MEM_COMMIT, PAGE_READWRITE), at.cast());
                    at.write_bytes(0xc7, 4096);
                    expected[1 << 16..(1 << 16) + 4096].fill(0xc7);
                }
                let mut out = vec![0xee; N];
                super::copy_mapped(out.as_mut_ptr().add(27), base.add(27), N - 54, false);
                assert_eq!(out[27..N - 27], expected[27..N - 27]);
                assert!(out[..27].iter().chain(&out[N - 27..]).all(|&v| v == 0xee));
            }
            assert_ne!(VirtualFree(base.cast(), 0, MEM_RELEASE), 0);
        }
    }

    use super::Batch;
    use std::sync::Arc;

    #[test]
    fn exact_gpu_writes_are_declared_once_per_submission() {
        let mut batch = Batch::default();
        let (a, b) = (Arc::new(0), Arc::new(0));
        assert!(batch.new_gpu_write(&a, 4096, 16));
        assert!(!batch.new_gpu_write(&a.clone(), 4096, 16));
        assert!(batch.new_gpu_write(&a, 4096, 32));
        assert!(batch.new_gpu_write(&a, 4104, 16));
        assert!(batch.new_gpu_write(&b, 4096, 16));
        // CPU copies can flush refreshes, but these GPU writes stay pending.
        batch.title_written();
        assert!(!batch.new_gpu_write(&a, 4096, 16));
        assert!(Batch::default().new_gpu_write(&a, 4096, 16));
    }

    #[test]
    fn disjoint_refreshes_preserve_the_gap_until_it_is_requested() {
        let Some(_) = super::host() else {
            assert!(std::env::var_os("VICTORY_D3D12_WARP").is_none());
            return;
        };
        const LEN: usize = 4 * crate::watch::PAGE;
        const MIDDLE: usize = LEN / 2;
        let memory = crate::resource::Memory::zeroed(LEN as u64, 64 << 10).unwrap();
        let address = memory.address() as usize;
        crate::shadow::register(address, memory.address(), LEN as u64);
        let shadow = crate::shadow::find(memory.address(), LEN as u64).unwrap();
        let mut mapping = std::ptr::null_mut();
        // SAFETY: both allocations cover LEN bytes and no GPU work is in flight.
        let data = unsafe {
            shadow.buffer.Map(0, None, Some(&mut mapping)).unwrap();
            let data = mapping.cast::<u8>();
            data.add(MIDDLE).write(0xc7);
            (address as *mut u8).write(0x77);
            ((address + MIDDLE) as *mut u8).write(0x39);
            ((address + LEN - 1) as *mut u8).write(0xd5);
            data
        };
        let mut batch = Batch::default();
        batch.add(&shadow, address + LEN - 64, 64);
        batch.add(&shadow, address, 64);
        batch.flush();
        // SAFETY: these offsets are inside the two live allocations.
        let after = unsafe { [data.read(), data.add(MIDDLE).read(), data.add(LEN - 1).read()] };
        batch.add(&shadow, address + MIDDLE, 1);
        batch.gpu_write(&shadow, address + MIDDLE, 1);
        // SAFETY: the declaration flushes its shadow before snapshotting.
        let later = unsafe { data.add(MIDDLE).read() };
        assert_eq!(shadow.copy_back(address + MIDDLE, 1), 0);
        // SAFETY: balances the test's Map after its last read.
        unsafe { shadow.buffer.Unmap(0, None) };
        crate::shadow::unregister(address, memory.address());
        assert_eq!(after, [0x77, 0xc7, 0xd5]);
        assert_eq!(later, 0x39);
    }

    #[test]
    fn gpu_write_flushes_only_its_shadow_until_title_write() {
        let Some(_) = super::host() else {
            assert!(std::env::var_os("VICTORY_D3D12_WARP").is_none());
            return;
        };
        let memory = || crate::resource::Memory::zeroed(64 << 10, 64 << 10).unwrap();
        let (a_memory, b_memory) = (memory(), memory());
        let (a_va, b_va) = (a_memory.address(), b_memory.address());
        let (a, b) = (a_va as usize, b_va as usize);
        let register = |address: u64| {
            crate::shadow::register(address as usize, address, 64 << 10);
            crate::shadow::find(address, 64 << 10).unwrap()
        };
        let (a_shadow, b_shadow) = (register(a_va), register(b_va));
        assert_ne!(a_shadow.buffer, b_shadow.buffer);
        let title = |address: usize, bytes: u8| {
            // SAFETY: address is the start of one of the test allocations.
            unsafe { (address as *mut u8).write_bytes(bytes, 32) };
        };
        let mapped = |shadow: &crate::shadow::Shadow| {
            let mut mapping = std::ptr::null_mut();
            // SAFETY: the mapped D3D12 buffer covers the 32 test bytes.
            unsafe {
                shadow.buffer.Map(0, None, Some(&mut mapping)).unwrap();
                let bytes = mapping.cast::<[u8; 32]>().read();
                shadow.buffer.Unmap(0, None);
                bytes
            }
        };
        title(a, 0xa1);
        title(b, 0xb1);
        let b_initial = mapped(&b_shadow);
        let mut batch = Batch::default();
        batch.add(&b_shadow, b, 16);
        batch.add(&a_shadow, a, 16);
        batch.gpu_write(&a_shadow, a, 16);
        assert_eq!(batch.updates, 1);
        assert_eq!(mapped(&a_shadow)[..16], [0xa1; 16]);
        assert_eq!(mapped(&b_shadow), b_initial);

        batch.add(&b_shadow, b + 16, 16);
        batch.add(&a_shadow, a + 16, 16);
        batch.gpu_write(&a_shadow, a + 16, 16);
        batch.gpu_write(&a_shadow, a, 16);
        assert_eq!(batch.updates, 2);
        assert_eq!(mapped(&a_shadow), [0xa1; 32]);
        assert_eq!(mapped(&b_shadow), b_initial);
        assert_eq!(batch.refreshes.len(), 1);
        assert_eq!(batch.refreshes[0].1.first, (b, b + 32));

        batch.title_written();
        assert_eq!(batch.updates, 3);
        assert_eq!(mapped(&b_shadow), [0xb1; 32]);
        title(b, 0xb2);
        batch.add(&b_shadow, b, 32);
        assert_eq!(mapped(&b_shadow), [0xb1; 32]);
        batch.flush();
        assert_eq!(batch.updates, 4);
        assert_eq!(mapped(&b_shadow), [0xb2; 32]);
        assert_eq!(a_shadow.copy_back(a, 16), 0);
        assert_eq!(a_shadow.copy_back(a + 16, 16), 0);

        crate::shadow::unregister(a, a_va);
        crate::shadow::unregister(b, b_va);
    }

    #[test]
    fn a_texture_is_checked_once_per_submission() {
        let mut batch = Batch::default();
        let (a, b) = (Arc::new([0u8; 16]), Arc::new([0u8; 16]));
        // The first list sampling each texture checks it ...
        assert!(batch.texture_due(&a));
        assert!(batch.texture_due(&b));
        // ... later lists of the same submission do not.
        assert!(!batch.texture_due(&a));
        assert!(!batch.texture_due(&a.clone()));
        assert!(!batch.texture_due(&b));
    }

    #[test]
    fn title_writes_make_textures_due_again() {
        let mut batch = Batch::default();
        let a = Arc::new(1u32);
        assert!(batch.texture_due(&a));
        assert!(!batch.texture_due(&a));
        // A copy into title memory or work that may write it may have
        // changed the texture's memory.
        batch.title_written();
        assert!(batch.texture_due(&a));
        assert!(!batch.texture_due(&a));
        // A fresh submission starts over.
        let mut next = Batch::default();
        assert!(next.texture_due(&a));
    }

    /// The hashing a submission does for textures in physical-page memory,
    /// with every list checking the textures it samples (before) and with
    /// each checked once (after). Run with
    /// `cargo test --release -p victory-gpu -- --ignored --nocapture texture_checks`.
    #[test]
    #[ignore = "benchmark"]
    fn texture_checks_per_submission() {
        const TEXTURES: usize = 48;
        const SIZE: usize = 4 << 20;
        const LISTS: usize = 12;
        let textures: Vec<Arc<Vec<u8>>> = (0..TEXTURES)
            .map(|t| Arc::new((0..SIZE).map(|i| (i * 31 + t) as u8).collect()))
            .collect();
        let submit = |dedupe: bool| {
            let mut batch = Batch::default();
            let mut hashed = 0usize;
            let started = std::time::Instant::now();
            for _ in 0..LISTS {
                for t in &textures {
                    if !dedupe || batch.texture_due(t) {
                        std::hint::black_box(blake3::hash(t));
                        hashed += 1;
                    }
                }
            }
            (started.elapsed(), hashed)
        };
        submit(false); // warm up
        let (before, n_before) = submit(false);
        let (after, n_after) = submit(true);
        assert_eq!((n_before, n_after), (TEXTURES * LISTS, TEXTURES));
        println!(
            "{LISTS} lists sampling {TEXTURES} textures of {} MiB: \
             before {n_before} hashes in {before:?}, after {n_after} in {after:?} ({:.1}x)",
            SIZE >> 20,
            before.as_secs_f64() / after.as_secs_f64()
        );
    }
}
