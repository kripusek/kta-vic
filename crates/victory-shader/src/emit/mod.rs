//! DXIL emission (decision 0003, step 3): a console vertex, pixel, compute
//! or mesh shader as a PC DXIL container (SM 6.6) that host D3D12 accepts.
//! Compute shaders keep the console's thread-group size; one that depends
//! on the lane layout (cross-lane operations) declares the console's wave
//! size (`WaveSize`), so the host device must support it.
//!
//! What the host binds for a translated shader ([`HostShader`]):
//!
//! * **User data** at `b0, space 0` (32-bit root constants or a constant
//!   buffer, per stage): dword `index` holds a root parameter's value, as
//!   [`UserData`] lists. A root-constant dword is the constant. A
//!   descriptor table is the byte offset of its first descriptor in its
//!   heap: the descriptor index times 32 for CBV/SRV/UAV tables, times 16
//!   for sampler tables (the console's descriptor sizes; the code adds
//!   offsets in those units). The heap bases of dynamic resources are 0
//!   and not listed.
//! * **Descriptors** through SM 6.6 `ResourceDescriptorHeap[]` and
//!   `SamplerDescriptorHeap[]`, at the index the console address gives, so
//!   the host heaps keep the title's descriptor order. The root signature
//!   needs `CBV_SRV_UAV_HEAP_DIRECTLY_INDEXED` and
//!   `SAMPLER_HEAP_DIRECTLY_INDEXED`.
//! * **Immediate samplers** as static samplers `s<n>, space 0`, `n` the
//!   position in [`HostShader::samplers`] (the console sampler words) and
//!   [`HostShader::static_samplers`] (the same decoded into a D3D12 static
//!   sampler description). Comparison sampling (added 2026-11-23): an
//!   immediate sampler it uses has a comparison filter there (bit 0x80);
//!   a heap sampler it uses must be a comparison sampler on the host.
//! * **Root CBVs and root SRVs** (raw buffers) as root descriptors
//!   `b<i>` / `t<i>` in [`ROOT_CBV_SPACE`], `i` the title's root parameter
//!   index ([`HostShader::root_cbvs`], [`HostShader::root_srvs`]); the
//!   address the title sets is the buffer's.
//! * **Vertex fetch** (added 2026-11-23). Grand Theft Auto VI's mesh and
//!   vertex programs fetch vertices themselves with typed-format loads
//!   (`tbuffer_load_format_*`): the format (8_8_8_8 unorm, 16_16 snorm,
//!   2_10_10_10 unorm, 8_8_8_8 and 16_16_16_16 uint...) is in the
//!   instruction, so the translation reads raw bytes and converts them
//!   itself. Their descriptors are built around a root SRV (stride 1, no
//!   record limit): the host binds the root SRV `t<slot>` above with the
//!   address the title sets, nothing more. The vertex index, the stride
//!   and the attribute offset are the shader's own arithmetic on data in
//!   that buffer (per-stream headers), not descriptor fields.
//! * **Buffer views in the heap** are declared by how the shader reads
//!   them: raw (ByteAddressBuffer) for dword loads and stores, atomics and
//!   typed-format loads without an index; structured with the used-resource
//!   entry's stride when indexed; typed with four 32-bit float channels for
//!   `buffer_load_format_*` and `buffer_store_format_*`. The host's view at
//!   that index must be of that kind and start at the console descriptor's
//!   base address (raw: `R32_TYPELESS` with the raw flag), whatever kind the
//!   title's view description says. Typed-format loads from heap
//!   descriptors (1,024 world compute programs, ray-tracing material
//!   resolves) read raw views this way.
//!
//! * **Texture atomics** use a scalar integer UAV. The host binds R32_UINT
//!   for 32-bit operations and R32G32_UINT for 64-bit operations. The latter
//!   require SM 6.6 atomic support on typed and descriptor-heap resources.
//!
//! Entry points: [`translate`] (container bytes as the title passes them
//! at pipeline creation → [`HostShader`]), [`emit`] (the same with the
//! earlier stages already run). The root constants of each stage must
//! cover [`HostShader::user_data_dwords`]. Stability: these types and the
//! binding scheme are what pipeline creation builds on; changes are
//! announced.
//! * **Signatures** as the console container has them (same semantics,
//!   registers and masks), so the input layout and stage linking are the
//!   title's; pixel outputs are plain 32-bit values.
//! * **Mesh shaders** ([`HwStage::Ms`], `ms_6_6`): user data and
//!   descriptors as above (the root constants visible to the mesh stage);
//!   the pipeline is a mesh pipeline (pipeline-state stream, no input
//!   layout) with the title's pixel shader. The thread group is the
//!   console's in lanes (`[numthreads]` in the DXIL), the vertex and
//!   primitive maxima its own, triangles only, no wave size required. The
//!   console numbers groups in x only: the draw path passes the title's
//!   count as `DispatchMesh(x, 1, 1)`. Per-primitive outputs (`PSG1`) are
//!   the console's, without `SV_PrimitiveID` and `SV_CullPrimitive` (the
//!   console never exports them).

mod bitcode;
mod container;
#[cfg(all(test, windows))]
mod diff_tests;
#[cfg(all(windows, any(test, feature = "host")))]
pub mod host;
#[cfg(all(test, windows))]
mod lds_tests;
mod lower;
mod sampler;
mod shader;
mod threading;
#[cfg(windows)]
pub mod validator;
#[cfg(all(test, windows))]
mod valu_tests;
mod wave;
#[cfg(all(test, windows))]
mod wave_tests;

#[cfg(test)]
mod wave64_tests;

#[cfg(all(test, windows))]
fn check_gpu_coverage(ran: usize) {
    if ran == 0 {
        #[cfg(feature = "agility")]
        panic!("no compatible SM 6.6 device available");
        #[cfg(not(feature = "agility"))]
        eprintln!("no compatible SM 6.6 device; GPU assertions skipped");
    }
}

use crate::dxbc::{Container, FourCC};
use crate::native::{HwStage, NativeProgram, RootType};
use crate::{ShaderError, io, ir, resource, structure};

pub use sampler::StaticSampler;
pub use shader::{ROOT_CBV_SPACE, SAMPLER_SPACE, USER_DATA_SPACE};

/// A translated shader and what the host binds for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostShader {
    pub stage: HwStage,
    /// The DXIL container, hashed as D3D12 expects.
    pub dxil: Vec<u8>,
    /// The user-data dwords the shader reads.
    pub user_data: Vec<UserData>,
    /// Immediate samplers: the four sampler words of `s<n>`.
    pub samplers: Vec<[u32; 4]>,
    /// The same samplers decoded, `s<n>` at position n: the static sampler
    /// descriptions the host's root signature declares.
    pub static_samplers: Vec<StaticSampler>,
    /// Root CBVs the shader reads: root parameter indices, each bound as
    /// `b<index>` in [`ROOT_CBV_SPACE`].
    pub root_cbvs: Vec<u8>,
    /// Root SRVs (raw buffers) the shader reads: `t<index>` in
    /// [`ROOT_CBV_SPACE`].
    pub root_srvs: Vec<u8>,
}

impl HostShader {
    /// Whether the DXIL requires direct indexing of the resource and sampler
    /// heaps, in that order. Static samplers need no indexed sampler heap.
    pub fn directly_indexed_heaps(&self) -> (bool, bool) {
        let features = Container::parse(&self.dxil)
            .ok()
            .and_then(|c| {
                let part = c.part(FourCC::SFI0)?;
                crate::dxbc::u64_at(part.data, 0).ok()
            })
            .unwrap_or(0);
        (
            features & shader::flags::FEATURE_RESOURCE_HEAP != 0,
            features & shader::flags::FEATURE_SAMPLER_HEAP != 0,
        )
    }

    /// Size of the user-data buffer the shader declares, in dwords (a
    /// multiple of 4; 0 when it reads none). Root constants must cover it.
    pub fn user_data_dwords(&self) -> u32 {
        let n = self
            .user_data
            .iter()
            .map(|u| u.index + 1)
            .max()
            .unwrap_or(0);
        n.next_multiple_of(4)
    }
}

/// One dword of user data and the root parameter that fills it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserData {
    /// Dword in the user-data buffer (the console's user-data index).
    pub index: u32,
    /// Root parameter index in the title's root signature.
    pub slot: u8,
    /// `Constants` or `Table` (see the module notes for the value).
    pub root: RootType,
    /// Dword within the parameter.
    pub dword: u32,
}

/// How a Wave64 compute shader is mapped onto the host device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wave64Mode {
    /// Emit native `WaveSize(64)` when compute lane semantics require it.
    #[default]
    Native,
    /// Map virtual 64-lane waves onto 32-lane host waves (`WaveSize(32)`).
    Wave32,
    /// Emulate 64-lane waves in group-shared memory without `WaveSize`.
    Emulate,
}

/// Options controlling DXIL emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TranslateOptions {
    pub wave64: Wave64Mode,
}

/// Wave sizes supported by a host GPU (`WaveLaneCountMin..=WaveLaneCountMax`).
///
/// Used to pick a host-supported wave size for small Wave64 compute thread
/// groups (`threads <= host_wave`) without hard-coding Wave32 (which would
/// break Wave64-only GPUs such as AMD GCN/Vega).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveRange {
    pub min: u32,
    pub max: u32,
}

impl WaveRange {
    /// Picks the wave size to declare for a compute group of `threads` lanes
    /// compiled for `console_wave` (32 or 64).
    ///
    /// Keeps `console_wave` when supported by the host; otherwise picks the
    /// smallest supported power-of-two wave size that still holds the whole
    /// group (`threads <= w < console_wave`), falling back to `console_wave`
    /// when no smaller supported wave fits.
    pub fn choose(self, console_wave: u32, threads: u32) -> u32 {
        if self.supports(console_wave) {
            return console_wave;
        }
        let mut w = self.min.max(4).next_power_of_two();
        while w <= self.max && w < console_wave {
            if threads <= w {
                return w;
            }
            w <<= 1;
        }
        console_wave
    }

    fn supports(self, wave: u32) -> bool {
        wave.is_power_of_two() && self.min <= wave && wave <= self.max
    }
}

/// Translates the vertex, pixel, compute or mesh program of a console
/// shader container (falling back to the console's wave size).
pub fn translate(container: &[u8]) -> Result<HostShader, ShaderError> {
    translate_for(container, None)
}

/// Translates the vertex, pixel, compute or mesh program of a console
/// shader container for a host with supported wave sizes `host_waves`
/// (`None` falls back to the console's wave size).
pub fn translate_for(
    container: &[u8],
    host_waves: Option<WaveRange>,
) -> Result<HostShader, ShaderError> {
    translate_target(container, host_waves, TranslateOptions::default())
}

pub fn translate_with(container: &[u8], options: TranslateOptions) -> Result<HostShader, ShaderError> {
    translate_target(container, None, options)
}

fn translate_target(
    container: &[u8],
    host_waves: Option<WaveRange>,
    options: TranslateOptions,
) -> Result<HostShader, ShaderError> {
    let c = Container::parse(container)?;
    let dxil = c
        .part(FourCC::DXIL)
        .ok_or(ShaderError::Malformed("no DXIL part"))?;
    let native = crate::native_programs(dxil.data)?
        .into_iter()
        .find(|p| {
            matches!(
                p.stage,
                HwStage::Vs | HwStage::Ps | HwStage::Cs | HwStage::Ms
            )
        })
        .ok_or(ShaderError::Unsupported {
            offset: 0,
            what: "a container without a vertex, pixel, compute or mesh program",
        })?;
    let p = ir::build_native(&native)?;
    match resource::track(&p, &native.layout, native.stage) {
        Ok(res) => emit_tracked(&c, &native, &p, &res, host_waves, options),
        Err(
            original @ ShaderError::Unsupported {
                offset,
                what: "a descriptor neither loaded nor built",
            },
        ) => {
            let Some(native) = structure::sampler_proof::retry_mesh(&native, &p, offset) else {
                return Err(original);
            };
            let retry = (|| {
                let p = ir::build(&native.code, native.wave64, native.vgprs)?;
                let res = resource::track(&p, &native.layout, native.stage)?;
                emit_tracked(&c, &native, &p, &res, host_waves, options)
            })();
            retry.map_err(|_| original)
        }
        Err(e) => Err(e),
    }
}

fn emit_tracked(
    c: &Container,
    native: &NativeProgram,
    p: &ir::Program,
    res: &resource::Resources,
    host_waves: Option<WaveRange>,
    options: TranslateOptions,
) -> Result<HostShader, ShaderError> {
    let io = io::raise(p, native)?;
    let st = structure::structure(p)?;
    emit_target(c, native, p, &st, &io, res, host_waves, options)
}

/// Emits a program whose earlier stages already ran (falling back to the
/// console's wave size).
pub fn emit(
    c: &Container,
    native: &NativeProgram,
    p: &ir::Program,
    st: &structure::Structured,
    io: &io::Io,
    res: &resource::Resources,
) -> Result<HostShader, ShaderError> {
    emit_for(c, native, p, st, io, res, None)
}

/// Emits a program whose earlier stages already ran for a host with
/// supported wave sizes `host_waves`.
pub fn emit_for(
    c: &Container,
    native: &NativeProgram,
    p: &ir::Program,
    st: &structure::Structured,
    io: &io::Io,
    res: &resource::Resources,
    host_waves: Option<WaveRange>,
) -> Result<HostShader, ShaderError> {
    emit_target(c, native, p, st, io, res, host_waves, TranslateOptions::default())
}

pub fn emit_with(
    c: &Container,
    native: &NativeProgram,
    p: &ir::Program,
    st: &structure::Structured,
    io: &io::Io,
    res: &resource::Resources,
    options: TranslateOptions,
) -> Result<HostShader, ShaderError> {
    emit_target(c, native, p, st, io, res, None, options)
}

fn emit_target(
    c: &Container,
    native: &NativeProgram,
    p: &ir::Program,
    st: &structure::Structured,
    io: &io::Io,
    res: &resource::Resources,
    host_waves: Option<WaveRange>,
    options: TranslateOptions,
) -> Result<HostShader, ShaderError> {
    let original = emit_with_threading(c, native, p, st, io, res, host_waves, options);
    let retry = match (&original, native.stage) {
        (
            Err(ShaderError::Unsupported {
                what: "a lane-mask join of an untracked mask"
                    | "a lane mask holding lanes elsewhere read as data",
                ..
            }),
            HwStage::Cs,
        ) => true,
        (
            Err(ShaderError::Unsupported {
                what: "a lane-mask join of an untracked mask",
                ..
            }),
            HwStage::Ps,
        ) => !p.wave64,
        _ => false,
    };
    if !retry {
        return original;
    }

    let mut p = p.clone();
    if !crate::sgpr_spill::forward(&mut p) {
        return original;
    }
    let attempt = (|| {
        let st = structure::structure(&p)?;
        let io = io::raise(&p, native)?;
        let res = resource::track(&p, &native.layout, native.stage)?;
        let dead_spills = crate::sgpr_spill::dead(&p);
        emit_program(
            c,
            native,
            &p,
            &st,
            &io,
            &res,
            host_waves,
            options,
            &dead_spills,
        )
    })();
    attempt.or(original)
}

fn emit_with_threading(
    c: &Container,
    native: &NativeProgram,
    p: &ir::Program,
    st: &structure::Structured,
    io: &io::Io,
    res: &resource::Resources,
    host_waves: Option<WaveRange>,
    options: TranslateOptions,
) -> Result<HostShader, ShaderError> {
    let original = emit_program(
        c,
        native,
        p,
        st,
        io,
        res,
        host_waves,
        options,
        &Default::default(),
    );
    // Retry unsupported programs only. Threading can separate an unknown
    // register value from its apparent use on a mutually exclusive path.
    if !matches!(original, Err(ShaderError::Unsupported { .. })) {
        return original;
    }
    let edges = threading::constant_edges(p, st);
    if edges.is_empty() {
        return original;
    }
    let p = ir::thread_edges(p, &edges)?;
    let st = structure::structure(&p)?;
    let io = io::raise(&p, native)?;
    let res = resource::track(&p, &native.layout, native.stage)?;
    emit_program(
        c,
        native,
        &p,
        &st,
        &io,
        &res,
        host_waves,
        options,
        &Default::default(),
    )
}

fn emit_program(
    c: &Container,
    native: &NativeProgram,
    p: &ir::Program,
    st: &structure::Structured,
    io: &io::Io,
    res: &resource::Resources,
    host_waves: Option<WaveRange>,
    options: TranslateOptions,
    dead_spills: &std::collections::HashSet<resource::Site>,
) -> Result<HostShader, ShaderError> {
    let sig = |fourcc| -> Result<Vec<container::SigElement>, ShaderError> {
        match c.part(fourcc) {
            Some(part) => signature(part.data),
            None => Ok(Vec::new()),
        }
    };
    let pixel = native.stage == HwStage::Ps;
    let mut inputs = sig(FourCC::ISG1)?;
    let mut outputs = sig(FourCC::OSG1)?;
    // Interpolation as DXC declares it by default; pixel inputs that are
    // read get the mode their barycentrics say.
    let interpolated = if pixel { &mut inputs } else { &mut outputs };
    for e in interpolated {
        e.interp = match (e.kind, e.comp) {
            (3, _) => 4,
            // Constant, as DXC declares it (the AMD compiler crashed on a
            // front face without a mode).
            (13, _) => 1,
            (0 | 6 | 7, 9) => 2,
            (0, _) => 1,
            _ => 0,
        };
    }
    // Per-primitive outputs are constant. The console never exports a
    // primitive id or cull flag a mesh shader writes (it culls with the
    // primitive export), so those rows are left out.
    let mut prims = sig(FourCC::PSG1)?;
    prims.retain(|e| !matches!(e.kind, 10 | 30));
    for e in &mut prims {
        e.interp = 1;
    }
    // A mesh group's id arrives multiplied by max(threads, vertices,
    // primitives) (the code divides by that); the thread count is in PSV0
    // from runtime info version 2 on.
    // shortcut: older containers fall back to the group's lane count, right
    // when the HLSL group fills its waves (Grand Theft Auto VI's do).
    let mesh_stride = match &native.io {
        crate::native::StageIo::Ms(ms) => {
            let psv = c.part(FourCC::PSV0).map(|p| p.data);
            let threads = psv
                .filter(|d| crate::dxbc::u32_at(d, 0).is_ok_and(|n| n >= 48))
                .and_then(|d| crate::dxbc::u32_at(d, 4 + 36).ok());
            threads.map_or(ms.threads, |t| {
                t.max(ms.max_vertices).max(ms.max_primitives)
            })
        }
        _ => 0,
    };
    let l = lower::lower_with_dead_spills(
        &lower::Parts {
            native,
            p,
            st,
            io,
            res,
            inputs,
            outputs,
            prims,
            mesh_stride,
            host_waves,
            options,
        },
        dead_spills,
    )?;
    Ok(HostShader {
        stage: native.stage,
        dxil: l.dxil,
        user_data: l.user_data,
        static_samplers: l
            .samplers
            .iter()
            .enumerate()
            .map(|(n, &w)| {
                let mut s = StaticSampler::from_words(w);
                if l.cmp_samplers >> n & 1 != 0 {
                    s.filter |= 0x80;
                }
                s
            })
            .collect(),
        samplers: l.samplers,
        root_cbvs: l.root_cbvs,
        root_srvs: l.root_srvs,
    })
}

/// The rows of an `ISG1`/`OSG1` part.
fn signature(part: &[u8]) -> Result<Vec<container::SigElement>, ShaderError> {
    use crate::dxbc::u32_at;
    let count = u32_at(part, 0)? as usize;
    let base = u32_at(part, 4)? as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let at = base + 32 * i;
        let name_at = u32_at(part, at + 4)? as usize;
        let name = part
            .get(name_at..)
            .and_then(|s| s.split(|&b| b == 0).next())
            .ok_or(ShaderError::Malformed("signature name out of bounds"))?;
        let mask = *part
            .get(at + 24)
            .ok_or(ShaderError::Malformed("truncated"))?;
        if u32_at(part, at + 28)? != 0 {
            return Err(ShaderError::Unsupported {
                offset: 0,
                what: "a minimum-precision signature element",
            });
        }
        if u32_at(part, at)? != 0 {
            return Err(ShaderError::Unsupported {
                offset: 0,
                what: "a signature element of another stream",
            });
        }
        let kind = match u32_at(part, at + 12)? {
            0 => 0,
            1 => 3,
            2 => 6,
            3 => 7,
            4 => 4,
            5 => 5,
            6 => 1,
            7 => 10,
            8 => 2,
            9 => 13,
            10 => 12,
            64 => 16,
            65 => 17,
            66 => 14,
            67 => 19,
            68 => 18,
            69 => 20,
            70 => 15,
            25 => 30, // cull primitive
            _ => {
                return Err(ShaderError::Unsupported {
                    offset: 0,
                    what: "a system value in the signature",
                });
            }
        };
        // 16-bit elements are declared 32-bit; reading or writing one
        // converts (16-bit values travel packed on the console).
        let raw = u32_at(part, at + 16)?;
        let comp = match (kind, raw) {
            // DXIL declares the front face a uint (the validator insists).
            (13, _) => 5,
            (_, 1 | 4) => 5,
            (_, 2 | 5) => 4,
            (_, 3 | 6) => 9,
            (_, c) => {
                return Err(ShaderError::Unsupported {
                    offset: 0,
                    what: match c {
                        0 => "a signature element of unknown type",
                        _ => "a 64-bit signature element",
                    },
                });
            }
        };
        let register = u32_at(part, at + 20)?;
        out.push(container::SigElement {
            name: String::from_utf8_lossy(name).into_owned(),
            index: u32_at(part, at + 8)?,
            kind,
            comp,
            interp: 0,
            row: (register != u32::MAX).then_some(register),
            col: mask.trailing_zeros().min(3) as u8,
            cols: mask.count_ones().max(1) as u8,
            usage: 0,
            half: (4..=6).contains(&raw),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn direct_heap_requirements_are_independent() {
        let mut shader = super::HostShader {
            stage: crate::HwStage::Cs,
            dxil: super::container::build(&[]),
            user_data: Vec::new(),
            samplers: Vec::new(),
            static_samplers: Vec::new(),
            root_cbvs: Vec::new(),
            root_srvs: Vec::new(),
        };
        assert_eq!(shader.directly_indexed_heaps(), (false, false));
        for (features, expected) in [
            (0u64, (false, false)),
            (0x0200_0000, (true, false)),
            (0x0400_0000, (false, true)),
            (0x0600_0000, (true, true)),
        ] {
            shader.dxil = super::container::build(&[(b"SFI0", &features.to_le_bytes())]);
            assert_eq!(shader.directly_indexed_heaps(), expected);
        }
    }

    use super::bitcode::Attr;
    use super::container::SigElement;
    use super::shader::Shader;
    use super::*;
    use crate::native::{ComputeIo, StageIo};

    /// A wave64 compute program that writes only EXEC's high dword
    /// (`s_mov_b32 exec_hi, 0`) before a vector instruction. The new mask is
    /// opaque and keyed by the unchanged low dword, so building its lane bit
    /// needed the mask's own value: lowering recursed until the stack
    /// overflowed, which takes the whole process down (found by
    /// random-input testing). Now the instruction fails like any other that
    /// cannot be expressed: its result is missing, and only a side effect
    /// that uses it would stop the translation.
    #[test]
    fn an_exec_high_write_does_not_overflow_the_stack() {
        // s_mov_b32 exec_hi, 0; v_mov_b32 v1, v0; s_endpgm
        let words = [0xBEFF_0380u32, 0x7E02_0300, 0xBF81_0000];
        let native = NativeProgram {
            stage: HwStage::Cs,
            wave64: true,
            vgprs: 2,
            layout: vec![],
            used: vec![],
            io: StageIo::Cs(ComputeIo {
                rsrc2: 0,
                threads: [64, 1, 1],
            }),
            code: words.iter().flat_map(|w| w.to_le_bytes()).collect(),
        };
        let p = ir::build(&native.patched_code(), native.wave64, native.vgprs).unwrap();
        let res = resource::track(&p, &native.layout, native.stage).unwrap();
        let io = io::raise(&p, &native).unwrap();
        let st = structure::structure(&p).unwrap();
        let container = crate::dxbc::build(&[]);
        let c = Container::parse(&container).unwrap();
        if let Err(e) = emit(&c, &native, &p, &st, &io, &res) {
            assert!(e.to_string().contains("its own value"), "{e}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn entry_spills_with_a_clobbered_sgpr_lower_and_validate() {
        use crate::native::RootParam;
        let words = [
            0xBE82_0300u32, // s2 = s0
            0x8703_FF01,
            0x0000_00FF, // s3 = s1 & 0xff
            0xF40C_0301,
            0xFA00_0010, // s[12:19] = argument dwords 4..11 (clobbers s16)
            0xF404_0101,
            0xFA00_0000, // s[4:5] = argument dwords 0..1
            0xBF07_8004, // branch on the value from entry s16
            0xBF85_0001,
            0xBE86_0380,
            0xBF81_0000,
        ];
        let native = NativeProgram {
            stage: HwStage::Cs,
            wave64: false,
            vgprs: 1,
            layout: vec![RootParam {
                root: RootType::Constants,
                table_offset_dwords: 0,
                sgprs: 12,
                base_register: 0,
                user_data: 16,
                slot: 0,
            }],
            used: vec![],
            io: StageIo::Cs(ComputeIo {
                rsrc2: 4 << 1,
                threads: [1, 1, 1],
            }),
            code: words.into_iter().flat_map(u32::to_le_bytes).collect(),
        };
        let p = ir::build_native(&native).unwrap();
        let st = structure::structure(&p).unwrap();
        let io = io::raise(&p, &native).unwrap();
        let res = resource::track(&p, &native.layout, native.stage).unwrap();
        let bytes = crate::dxbc::build(&[]);
        let container = Container::parse(&bytes).unwrap();
        let mut shader = emit(&container, &native, &p, &st, &io, &res).unwrap();
        assert!(shader.user_data.iter().any(|u| u.index == 16 && u.slot == 0));
        if let Some(path) = validator::Validator::default_path() {
            validator::Validator::load(Some(&path))
                .unwrap()
                .validate(&mut shader.dxil)
                .unwrap();
        }
    }

    #[test]
    fn direct_pixel_barycentrics_use_host_signature() {
        // v0/v1 contain perspective I/J and v2/v3 linear I/J. Export
        // both pairs directly, without v_interp to fold their uses away.
        let words = [0xF800_180Fu32, 0x0302_0100, 0xBF81_0000];
        let native = NativeProgram {
            stage: HwStage::Ps,
            wave64: false,
            vgprs: 4,
            layout: vec![],
            used: vec![],
            io: StageIo::Ps {
                inputs: vec![],
                input_addr: (1 << 1) | (1 << 5),
                db_shader_control: 0,
                patches: vec![],
            },
            code: words.iter().flat_map(|word| word.to_le_bytes()).collect(),
        };
        let outputs = container::signature(
            &[SigElement {
                name: "SV_Target".into(),
                index: 0,
                kind: 16,
                comp: 9,
                interp: 0,
                row: Some(0),
                col: 0,
                cols: 4,
                usage: 15,
                half: false,
            }],
            false,
        );
        let bytes = crate::dxbc::build(&[(FourCC::OSG1, &outputs)]);
        let p = ir::build(&native.code, false, native.vgprs).unwrap();
        let shader = emit(
            &Container::parse(&bytes).unwrap(),
            &native,
            &p,
            &structure::structure(&p).unwrap(),
            &io::raise(&p, &native).unwrap(),
            &resource::track(&p, &native.layout, native.stage).unwrap(),
        )
        .unwrap();
        let container = Container::parse(&shader.dxil).unwrap();
        let features = u64::from_le_bytes(
            container
                .part(FourCC::SFI0)
                .unwrap()
                .data
                .try_into()
                .unwrap(),
        );
        assert_eq!(features & 0x2_0000, 0x2_0000);
        let inputs = container.part(FourCC::ISG1).unwrap().data;
        assert_eq!(crate::dxbc::u32_at(inputs, 0).unwrap(), 2);
        for index in 0..2 {
            let at = 8 + index * 32;
            assert_eq!(crate::dxbc::u32_at(inputs, at + 8).unwrap(), index as u32);
            assert_eq!(crate::dxbc::u32_at(inputs, at + 12).unwrap(), 23);
            assert_eq!(crate::dxbc::u32_at(inputs, at + 20).unwrap(), u32::MAX);
            assert_eq!(inputs[at + 25], 0b110);
        }
        #[cfg(windows)]
        {
            let path = validator::Validator::default_path();
            if let Ok(mut validator) = validator::Validator::load(path.as_deref()) {
                validator.validate(&mut shader.dxil.clone()).unwrap();
            }
        }
    }

    #[test]
    fn nested_pixel_phis_materialize_barycentric_input() {
        let words = [
            0xbf06_8000u32, // if s0 == 0, retain incoming v0
            0xbf85_0001,
            0x7e00_0281, // otherwise v0 = 1
            0xbf06_8001, // first join; if s1 == 0, retain that join
            0xbf85_0001,
            0x7e00_0282, // otherwise v0 = 2
            0xf800_180f, // second join; export v0 in every channel
            0,
            0xbf81_0000,
        ];
        let native = NativeProgram {
            stage: HwStage::Ps,
            wave64: false,
            vgprs: 1,
            layout: vec![crate::native::RootParam {
                root: crate::native::RootType::Constants,
                table_offset_dwords: 0,
                sgprs: 2,
                base_register: 0,
                user_data: 0,
                slot: 1,
            }],
            used: vec![],
            io: StageIo::Ps {
                inputs: vec![],
                input_addr: 1 << 1,
                db_shader_control: 0,
                patches: vec![],
            },
            code: words.iter().flat_map(|word| word.to_le_bytes()).collect(),
        };
        let outputs = container::signature(
            &[SigElement {
                name: "SV_Target".into(),
                index: 0,
                kind: 16,
                comp: 9,
                interp: 0,
                row: Some(0),
                col: 0,
                cols: 4,
                usage: 15,
                half: false,
            }],
            false,
        );
        let bytes = crate::dxbc::build(&[(FourCC::OSG1, &outputs)]);
        let p = ir::build(&native.code, false, native.vgprs).unwrap();
        let initial = p
            .inputs
            .iter()
            .find(|(r, _)| *r == ir::Reg::V(0))
            .unwrap()
            .1;
        let phis: Vec<_> = p.blocks.iter().flat_map(|block| &block.phis).collect();
        let first = phis.iter().find(|phi| phi.args.contains(&initial)).unwrap();
        assert!(phis.iter().any(|phi| phi.args.contains(&first.value)));
        let shader = emit(
            &Container::parse(&bytes).unwrap(),
            &native,
            &p,
            &structure::structure(&p).unwrap(),
            &io::raise(&p, &native).unwrap(),
            &resource::track(&p, &native.layout, native.stage).unwrap(),
        )
        .unwrap();
        let dxil = Container::parse(&shader.dxil).unwrap();
        let inputs = dxil.part(FourCC::ISG1).unwrap().data;
        assert_eq!(crate::dxbc::u32_at(inputs, 0).unwrap(), 1);
        assert_eq!(crate::dxbc::u32_at(inputs, 20).unwrap(), 23);
        assert_eq!(inputs[33], 0b010);
        #[cfg(windows)]
        if let Ok(mut validator) =
            validator::Validator::load(validator::Validator::default_path().as_deref())
        {
            validator.validate(&mut shader.dxil.clone()).unwrap();
        }
    }

    #[test]
    fn pixel_mask_join_retries_a_proven_lane_spill() {
        let mut words = [
            0x7d84_0080u32, // vcc = (v0 == 0)
            0xd761_0002,
            0x0001_026a, // v_writelane_b32 v2, vcc_lo, 1
            0xbf06_8000, // s_cmp_eq_u32 s0, 0
            0xbf85_0003,
            0xd760_0004,
            0x0001_0302, // v_readlane_b32 s4, v2, 1
            0xbf82_0002,
            0x7d84_0280, // alternate vcc = (v1 == 0)
            0xbe84_036a, // s_mov_b32 s4, vcc_lo
            0xd501_0003,
            0x0012_0300, // join: v_cndmask_b32 v3, v0, v1, s4
            0xf800_180f,
            0x0303_0303, // export v3 in every channel
            0xbf81_0000,
        ];
        let mut native = NativeProgram {
            stage: HwStage::Ps,
            wave64: false,
            vgprs: 4,
            layout: vec![crate::native::RootParam {
                root: crate::native::RootType::Constants,
                table_offset_dwords: 0,
                sgprs: 1,
                base_register: 0,
                user_data: 0,
                slot: 1,
            }],
            used: vec![],
            io: StageIo::Ps {
                inputs: vec![],
                input_addr: 1 << 1,
                db_shader_control: 0,
                patches: vec![],
            },
            code: vec![],
        };
        let bytes = constant_color();
        let c = Container::parse(&bytes).unwrap();
        for (reload_word, reload_matches) in [(0x0001_0302, true), (0x0001_0502, false)] {
            words[6] = reload_word; // the second case reads an unspilled lane
            native.code = words.iter().flat_map(|word| word.to_le_bytes()).collect();
            let p = ir::build(&native.code, native.wave64, native.vgprs).unwrap();
            let st = structure::structure(&p).unwrap();
            let io = io::raise(&p, &native).unwrap();
            let res = resource::track(&p, &native.layout, native.stage).unwrap();
            let original =
                emit_with_threading(&c, &native, &p, &st, &io, &res, None, Default::default());
            assert!(matches!(
                original,
                Err(ShaderError::Unsupported {
                    what: "a lane-mask join of an untracked mask",
                    ..
                })
            ));
            let translated = emit(&c, &native, &p, &st, &io, &res);
            if reload_matches {
                let shader = translated.unwrap();
                #[cfg(windows)]
                if let Ok(mut validator) =
                    validator::Validator::load(validator::Validator::default_path().as_deref())
                {
                    validator.validate(&mut shader.dxil.clone()).unwrap();
                }
                assert!(!shader.dxil.is_empty());
            } else {
                assert!(translated.is_err());
            }
        }
    }

    /// A pixel shader writing a constant colour.
    fn constant_color() -> Vec<u8> {
        let mut s = Shader::new(true);
        s.outputs.push(SigElement {
            name: "SV_Target".into(),
            index: 0,
            kind: 16,
            comp: 9,
            interp: 0,
            row: Some(0),
            col: 0,
            cols: 4,
            usage: 15,
            half: false,
        });
        let b = s.entry;
        for (c, v) in [1.0f32, 0.5, 0.25, 1.0].into_iter().enumerate() {
            let args = [
                s.m.i32(0),
                s.m.i32(0),
                s.m.i8(c as u8),
                s.m.f32(v.to_bits()),
            ];
            s.op(b, "storeOutput.f32", Attr::NoUnwind, s.t.void, 5, &args);
        }
        s.m.terminate(b, super::bitcode::Term::Ret);
        s.finish()
    }

    #[test]
    #[cfg(windows)]
    fn stub_pipeline_is_created() {
        let Ok(host) = super::host::Host::new(false) else {
            eprintln!("no SM 6.6 device; skipped");
            return;
        };
        let ps = super::HostShader {
            stage: crate::HwStage::Ps,
            dxil: constant_color(),
            user_data: Vec::new(),
            samplers: Vec::new(),
            static_samplers: Vec::new(),
            root_cbvs: Vec::new(),
            root_srvs: Vec::new(),
        };
        let file = |var: &str, stage| {
            std::env::var(var).ok().map(|p| super::HostShader {
                stage,
                dxil: std::fs::read(p).unwrap(),
                user_data: Vec::new(),
                samplers: Vec::new(),
                static_samplers: Vec::new(),
                root_cbvs: Vec::new(),
                root_srvs: Vec::new(),
            })
        };
        let ps = file("VICTORY_REF_PS", crate::HwStage::Ps).unwrap_or(ps);
        let vs = file("VICTORY_REF_VS", crate::HwStage::Vs)
            .unwrap_or_else(|| super::host::stub_vs(&ps).unwrap());
        if let Ok(mut v) = super::validator::Validator::load(
            super::validator::Validator::default_path().as_deref(),
        ) {
            v.validate(&mut vs.dxil.clone()).unwrap();
        }
        host.pipeline(&vs, &ps).unwrap();
        // The runtime checks the container hash: no hash or a wrong one is
        // refused. (Recent runtimes also take sixteen 0x01 bytes, a bypass
        // value; ours works with older ones too.)
        let mut wrong = ps.dxil[4..20].to_vec();
        wrong[0] ^= 1;
        for digest in [&[0u8; 16][..], &wrong] {
            let mut other = ps.clone();
            other.dxil[4..20].copy_from_slice(digest);
            assert!(
                host.pipeline(&vs, &other).is_err(),
                "digest {digest:02x?} accepted"
            );
        }
    }

    #[test]
    fn wave_range_matches_device_capabilities() {
        use super::WaveRange;
        // Wave64-only (e.g. AMD GCN / Vega): never hard-code 32.
        let gcn = WaveRange { min: 64, max: 64 };
        assert_eq!(gcn.choose(64, 32), 64);
        assert_eq!(gcn.choose(64, 64), 64);
        // Dual-wave (e.g. AMD RDNA): keep console wave size.
        let rdna = WaveRange { min: 32, max: 64 };
        assert_eq!(rdna.choose(64, 32), 64);
        assert_eq!(rdna.choose(32, 32), 32);
        // Wave32-only (e.g. NVIDIA): narrow small Wave64 groups to 32; keep 64 if > 32 threads.
        let nv = WaveRange { min: 32, max: 32 };
        assert_eq!(nv.choose(64, 32), 32);
        assert_eq!(nv.choose(64, 16), 32);
        assert_eq!(nv.choose(64, 64), 64);
        // Intel Arc (16..=32): pick smallest supported wave holding the group.
        let arc = WaveRange { min: 16, max: 32 };
        assert_eq!(arc.choose(64, 16), 16);
        assert_eq!(arc.choose(64, 32), 32);
        assert_eq!(arc.choose(64, 64), 64);
        // WARP (4..=8): holds <= 8 threads in 8; falls back to 64 for 32 threads.
        let warp = WaveRange { min: 4, max: 8 };
        assert_eq!(warp.choose(64, 8), 8);
        assert_eq!(warp.choose(64, 32), 64);
    }

    #[test]
    #[cfg(windows)]
    fn constant_color_validates() {
        let mut c = constant_color();
        let path = super::validator::Validator::default_path();
        let Ok(mut v) = super::validator::Validator::load(path.as_deref()) else {
            eprintln!("no dxil.dll; skipped");
            return;
        };
        let ours = c[4..20].to_vec();
        v.validate(&mut c).unwrap();
        assert_eq!(c[4..20], ours[..], "our hash differs from the validator's");
    }

    /// A pixel shader reading channel z of an input whose element starts at
    /// w. Looking the channel up subtracted the element's first channel
    /// before checking the channel lies in it, so in a build with overflow
    /// checks (tests, debug) any read below an element that starts at z or
    /// w panicked, whichever row it was in (found by random-input testing).
    #[test]
    fn reading_a_channel_below_an_input_element_does_not_underflow() {
        // v_interp_mov_f32 v3, p0, attr0.z; s_endpgm
        let words = [0xC80E_0202u32, 0xBF81_0000];
        let native = super::NativeProgram {
            stage: super::HwStage::Ps,
            wave64: false,
            vgprs: 4,
            layout: vec![],
            used: vec![],
            io: crate::native::StageIo::Ps {
                inputs: vec![crate::native::PsInput {
                    usage: crate::native::Usage::Generic,
                    row: 0,
                    mask: 0b1000,
                    attr: 0,
                    flat: false,
                    fp16: false,
                    pack2: false,
                    row2: 0,
                }],
                input_addr: 0,
                db_shader_control: 0,
                patches: vec![],
            },
            code: words.iter().flat_map(|w| w.to_le_bytes()).collect(),
        };
        let w_only = SigElement {
            name: "TEXCOORD".into(),
            index: 0,
            kind: 0,
            comp: 9,
            interp: 0,
            row: Some(0),
            col: 3,
            cols: 1,
            usage: 0,
            half: false,
        };
        let isg1 = super::container::signature(&[w_only], true);
        let bytes = crate::dxbc::build(&[(super::FourCC::ISG1, &isg1)]);
        let c = super::Container::parse(&bytes).unwrap();
        let p = crate::ir::build(&native.patched_code(), native.wave64, native.vgprs).unwrap();
        let res = crate::resource::track(&p, &native.layout, native.stage).unwrap();
        let io = crate::io::raise(&p, &native).unwrap();
        let st = crate::structure::structure(&p).unwrap();
        // Translated or refused with a reason; it must not panic.
        let _ = super::emit(&c, &native, &p, &st, &io, &res);
    }

    /// A low-half f16 interpolation preserves the destination's old high
    /// half. The second interpolation writes zero into the low half,
    /// leaving the original 1.0 bit pattern for the color export.
    #[test]
    fn an_f16_interpolation_preserves_its_old_high_half() {
        check_f16_interpolation_high_half(false);
    }

    #[test]
    fn a_guarded_f16_fma_requires_the_entire_old_interpolation_result() {
        check_f16_interpolation_high_half(true);
    }

    fn check_f16_interpolation_high_half(guarded_fma: bool) {
        let mut words = vec![
            0x7E16_02F2u32, // v_mov_b32 v11, 1.0 (initial high bits)
            0x7E20_0280,    // v_mov_b32 v16, 0
            0xD742_000B,
            0x0002_0102, // v_interp_p1ll_f16 v11, v0, attr2.x high
        ];
        if guarded_fma {
            words.extend([
                0x7E10_0280, // v_mov_b32 v8, 0
                0xD75A_000F,
                0x042E_0302, // v_interp_p2_f16 v15, v1, attr2.x, v11 high
                0x7D84_0080, // v_cmp_eq_u32 vcc, 0, v0
                0xBE88_246A, // s_and_saveexec_b64 s[8:9], vcc
                0xCC21_000B,
                0x0A02_2108, // v_fma_mixlo_f16 v11, sel_lo(v8), v16, 0
                0xBEFE_0408, // s_mov_b64 exec, s[8:9]
            ]);
        } else {
            words.extend([0xD75A_000B, 0x042E_0302]); // v_interp_p2_f16 v11, v1, attr2.x, v11 high
        }
        words.extend([
            0xF800_1801,
            0x0000_000B, // exp mrt0, v11, off, off, off done vm
            0xBF81_0000, // s_endpgm
        ]);
        let native = NativeProgram {
            stage: HwStage::Ps,
            wave64: guarded_fma,
            vgprs: 17,
            layout: vec![],
            used: vec![],
            io: StageIo::Ps {
                inputs: vec![crate::native::PsInput {
                    usage: crate::native::Usage::Generic,
                    row: 2,
                    mask: 3,
                    attr: 2,
                    flat: false,
                    fp16: true,
                    pack2: true,
                    row2: 2,
                }],
                input_addr: 2,
                db_shader_control: 0,
                patches: vec![],
            },
            code: words.iter().flat_map(|w| w.to_le_bytes()).collect(),
        };
        let input = SigElement {
            name: "TEXCOORD".into(),
            index: 0,
            kind: 0,
            comp: 9,
            interp: 0,
            row: Some(2),
            col: 0,
            cols: 2,
            usage: 0,
            half: false,
        };
        let output = SigElement {
            name: "SV_Target".into(),
            index: 0,
            kind: 16,
            comp: 9,
            interp: 0,
            row: Some(0),
            col: 0,
            cols: 1,
            usage: 1,
            half: false,
        };
        let isg1 = super::container::signature(&[input], true);
        let osg1 = super::container::signature(&[output], false);
        let bytes =
            crate::dxbc::build(&[(super::FourCC::ISG1, &isg1), (super::FourCC::OSG1, &osg1)]);
        let c = Container::parse(&bytes).unwrap();
        let p = ir::build(&native.patched_code(), native.wave64, native.vgprs).unwrap();
        let res = resource::track(&p, &native.layout, native.stage).unwrap();
        let io = io::raise(&p, &native).unwrap();
        let st = structure::structure(&p).unwrap();
        let emitted = emit(&c, &native, &p, &st, &io, &res);
        if guarded_fma {
            assert!(
                matches!(
                    emitted,
                    Err(crate::ShaderError::Unsupported {
                        offset: 48,
                        what: "the first half of an interpolation",
                    })
                ),
                "guarded result: {:?}",
                emitted.as_ref().err(),
            );
            return;
        }
        let mut host = emitted.unwrap();
        #[cfg(windows)]
        if let Ok(mut v) = super::validator::Validator::load(
            super::validator::Validator::default_path().as_deref(),
        ) {
            v.validate(&mut host.dxil).unwrap();
        }
        #[cfg(all(windows, feature = "agility"))]
        {
            use super::bitcode::{Attr, Term};
            use super::host::{Draw, Host};
            use super::shader::Shader;

            let mut vs = Shader::new(false);
            vs.outputs.push(SigElement {
                name: "SV_Position".into(),
                index: 0,
                kind: 3,
                comp: 9,
                interp: 4,
                row: Some(0),
                col: 0,
                cols: 4,
                usage: 15,
                half: false,
            });
            vs.inputs.push(SigElement {
                name: "POSITION".into(),
                index: 0,
                kind: 0,
                comp: 9,
                interp: 0,
                row: Some(0),
                col: 0,
                cols: 4,
                usage: 15,
                half: false,
            });
            vs.outputs.push(SigElement {
                name: "TEXCOORD".into(),
                index: 0,
                kind: 0,
                comp: 9,
                interp: 2,
                row: Some(2),
                col: 0,
                cols: 2,
                usage: 3,
                half: false,
            });
            let block = vs.entry;
            for col in 0..4u8 {
                let undef = vs.m.undef(vs.t.i32);
                let args = [vs.m.i32(0), vs.m.i32(0), vs.m.i8(col), undef];
                let v = vs.op(block, "loadInput.f32", Attr::ReadNone, vs.t.f32, 4, &args);
                let out = [vs.m.i32(0), vs.m.i32(0), vs.m.i8(col), v];
                vs.op(block, "storeOutput.f32", Attr::NoUnwind, vs.t.void, 5, &out);
            }
            for col in 0..2u8 {
                let out = [vs.m.i32(1), vs.m.i32(0), vs.m.i8(col), vs.m.f32(0)];
                vs.op(block, "storeOutput.f32", Attr::NoUnwind, vs.t.void, 5, &out);
            }
            vs.m.terminate(block, Term::Ret);
            let vs = HostShader {
                stage: HwStage::Vs,
                dxil: vs.finish(),
                user_data: Vec::new(),
                samplers: Vec::new(),
                static_samplers: Vec::new(),
                root_cbvs: Vec::new(),
                root_srvs: Vec::new(),
            };
            let warp = match Host::new(true) {
                Ok(warp) => warp,
                Err(error) if std::env::var("VICTORY_WARP").as_deref() != Ok("1") => {
                    eprintln!("WARP host unavailable: {error}");
                    return;
                }
                Err(error) => panic!("WARP host required: {error}"),
            };
            let (root, pso) = warp.pipeline(&vs, &host).unwrap();
            let vertices = [
                -1.0, -1.0, 0.5, 1.0, -1.0, 3.0, 0.5, 1.0, 3.0, -1.0, 0.5, 1.0,
            ];
            let colors = warp
                .draw(
                    &root,
                    &pso,
                    &Draw {
                        roots: vec![],
                        textures: &[],
                        samplers: &[],
                        vertices: &vertices,
                        stride: 16,
                        size: (1, 1),
                        mesh_groups: 0,
                    },
                )
                .unwrap();
            assert_eq!(colors[0][0], 1.0);
        }
    }
}

#[cfg(test)]
mod atomic_tests {
    use super::*;
    use crate::native::{ComputeIo, RootParam, StageIo};

    #[test]
    fn heap_texture_atomic_widths() {
        // A synthetic compute shader loads a 2D UAV from a root table,
        // compares or adds at (0, 0), then uses the returned register(s).
        for (opcode, mask, wide) in [
            (16u32, 3, false),
            (16, 15, true),
            (17, 1, false),
            (17, 3, true),
        ] {
            let words = [
                0xBE81_0380, // s_mov_b32 s1, 0 (table address high)
                0xF40C_0200,
                0xFA00_0000, // s_load_dwordx8 s[8:15], s[0:1], 0
                0x7E00_0280,
                0x7E02_0280, // v0, v1 = coordinates
                0x7E04_0281,
                0x7E06_0282,
                0x7E08_0283,
                0x7E0A_0284, // v2..v5 = 1..4
                0xF000_2008 | opcode << 18 | mask << 8,
                0x0002_0200,
                // Use the old value in a second atomic at the same location.
                0xF044_0008 | (if wide { 3 } else { 1 }) << 8,
                0x0002_0200,
                0xBF81_0000,
            ];
            let native = NativeProgram {
                stage: HwStage::Cs,
                wave64: false,
                vgprs: 6,
                layout: vec![RootParam {
                    root: RootType::Table,
                    table_offset_dwords: 0,
                    sgprs: 1,
                    base_register: 0,
                    user_data: 0,
                    slot: 0,
                }],
                used: vec![],
                io: StageIo::Cs(ComputeIo {
                    rsrc2: 2,
                    threads: [32, 1, 1],
                }),
                code: words.iter().flat_map(|w| w.to_le_bytes()).collect(),
            };
            let p = ir::build(&native.code, false, native.vgprs).unwrap();
            let bytes = crate::dxbc::build(&[]);
            let shader = emit(
                &Container::parse(&bytes).unwrap(),
                &native,
                &p,
                &structure::structure(&p).unwrap(),
                &io::raise(&p, &native).unwrap(),
                &resource::track(&p, &native.layout, native.stage).unwrap(),
            )
            .unwrap();
            let container = Container::parse(&shader.dxil).unwrap();
            let features = u64::from_le_bytes(
                container
                    .part(FourCC::SFI0)
                    .unwrap()
                    .data
                    .try_into()
                    .unwrap(),
            );
            let required = 0x1040_8000; // heap atomics, typed atomics, Int64Ops
            assert_eq!(features & required, if wide { required } else { 0 });
            #[cfg(windows)]
            if let Ok(mut v) =
                validator::Validator::load(validator::Validator::default_path().as_deref())
            {
                v.validate(&mut shader.dxil.clone()).unwrap();
            }
        }
    }
}
