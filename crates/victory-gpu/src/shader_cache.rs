//! Translated shaders kept on disk (decision 0003): under the configured
//! cache folder, keyed by the container's content and by the VIctory build
//! that translated it (the BLAKE3 of our executable), so a rebuilt
//! translator never serves an old translation. Entries go through
//! `victory-pipeline`'s content-addressed cache, which writes them
//! atomically; an entry that does not read back is translated again.
//!
//! The entry is our own encoding of [`HostShader`]: a field added to it
//! must be added to [`encode`] and [`decode`] too (entries of older builds
//! are ignored anyway).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use victory_pipeline::{Cache, Converter};
use victory_shader::emit::{HostShader, StaticSampler, UserData, WaveRange};
use victory_shader::{HwStage, RootType, ShaderError};

static CACHE: OnceLock<Cache> = OnceLock::new();
static HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);

/// Keeps translated shaders in `dir` from now on.
pub fn use_shader_cache(dir: PathBuf) {
    match Cache::open(dir) {
        Ok(cache) => {
            let _ = CACHE.set(cache);
        }
        Err(e) => tracing::warn!(error = %e, "no shader cache"),
    }
}

/// Shaders found in the cache and translated since the start.
pub(crate) fn counts() -> (u64, u64) {
    (HITS.load(Ordering::Relaxed), MISSES.load(Ordering::Relaxed))
}

/// What identifies this build: the BLAKE3 of our executable file.
fn build() -> &'static [u8; 32] {
    static BUILD: OnceLock<[u8; 32]> = OnceLock::new();
    BUILD.get_or_init(|| {
        let exe = std::env::current_exe().and_then(std::fs::read);
        match exe {
            Ok(bytes) => *blake3::hash(&bytes).as_bytes(),
            Err(e) => {
                // Without it no entry can be trusted: a fresh key per run.
                tracing::warn!(error = %e, "cannot read our executable; shader cache unused");
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                *blake3::hash(&now.as_nanos().to_le_bytes()).as_bytes()
            }
        }
    })
}

fn host_waves() -> Option<WaveRange> {
    #[cfg(windows)]
    {
        crate::host::host().and_then(|h| h.wave_range)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn cache_key(build: &[u8; 32], waves: Option<WaveRange>) -> [u8; 32] {
    match waves {
        None => *build,
        Some(r) => {
            let mut hasher = blake3::Hasher::new();
            hasher.update(build);
            hasher.update(&r.min.to_le_bytes());
            hasher.update(&r.max.to_le_bytes());
            *hasher.finalize().as_bytes()
        }
    }
}

/// The translation of a console shader container, from the cache when
/// there is one.
pub(crate) fn host_shader(container: &[u8]) -> Result<HostShader, String> {
    let waves = host_waves();
    let translate = |c: &[u8]| victory_shader::emit::translate_for(c, waves);
    match CACHE.get() {
        Some(cache) => cached(cache, &cache_key(build(), waves), container, translate),
        None => translate(container).map_err(|e| e.to_string()),
    }
}

/// The cache entry's converter: the build and the container in, the
/// encoded translation out. `ran` tells a miss from a hit.
struct Translate<F> {
    translate: F,
    ran: AtomicBool,
}

impl<F: Fn(&[u8]) -> Result<HostShader, ShaderError> + Send + Sync> Converter for Translate<F> {
    fn id(&self) -> &'static str {
        "shader.dxil"
    }
    fn version(&self) -> u32 {
        // The build fingerprint in the input does the versioning.
        1
    }
    fn convert(&self, input: &[u8]) -> anyhow::Result<Vec<u8>> {
        self.ran.store(true, Ordering::Relaxed);
        match (self.translate)(&input[32..]) {
            Ok(shader) => Ok(encode(&shader)),
            Err(e @ ShaderError::Unsupported { .. }) => {
                // This is a decision of the translator for this build, container
                // and wave range. Device, driver and file I/O failures occur
                // outside this decision and must remain retryable.
                let reason = e.to_string();
                let mut bytes = UNSUPPORTED.to_vec();
                bytes.extend_from_slice(blake3::hash(reason.as_bytes()).as_bytes());
                bytes.extend_from_slice(reason.as_bytes());
                Ok(bytes)
            }
            Err(e) => Err(e.into()),
        }
    }
}

fn cached(
    cache: &Cache,
    build: &[u8; 32],
    container: &[u8],
    translate: impl Fn(&[u8]) -> Result<HostShader, ShaderError> + Send + Sync,
) -> Result<HostShader, String> {
    let input = [&build[..], container].concat();
    let converter = Translate {
        translate,
        ran: AtomicBool::new(false),
    };
    for _ in 0..2 {
        let path = match cache.get_or_convert(&converter, &input) {
            Ok(path) => path,
            Err(e) if e.downcast_ref::<ShaderError>().is_some() => return Err(format!("{e:#}")),
            Err(e) => {
                tracing::debug!(error = %e, "shader cache unavailable; translating directly");
                break;
            }
        };
        let bytes = std::fs::read(&path).unwrap_or_default();
        if let Some(reason) = unsupported_reason(&bytes) {
            // Preserve the converter's diagnostic on the first and later runs.
            return Err(format!(
                "converter {} v{}: {reason}", converter.id(), converter.version()
            ));
        }
        if let Some(shader) = decode(&bytes) {
            let (counter, what) = if converter.ran.load(Ordering::Relaxed) {
                (&MISSES, "translated and cached")
            } else {
                (&HITS, "from the cache")
            };
            counter.fetch_add(1, Ordering::Relaxed);
            tracing::debug!(path = %path.display(), "shader {what}");
            return Ok(shader);
        }
        tracing::debug!(path = %path.display(), "unreadable shader cache entry; translating again");
        let _ = std::fs::remove_file(&path);
    }
    MISSES.fetch_add(1, Ordering::Relaxed);
    (converter.translate)(container).map_err(|e| e.to_string())
}

// Check the digest before accepting a negative entry. Damaged data must
// retry translation, just like a malformed positive entry.
const UNSUPPORTED: &[u8; 4] = b"VSU1";

fn unsupported_reason(bytes: &[u8]) -> Option<&str> {
    let (digest, reason) = bytes.strip_prefix(UNSUPPORTED)?.split_at_checked(32)?;
    if reason.is_empty() || digest != blake3::hash(reason).as_bytes() {
        return None;
    }
    std::str::from_utf8(reason).ok()
}

const MAGIC: &[u8; 4] = b"VSH1";
const STAGES: [HwStage; 8] = [
    HwStage::Ls,
    HwStage::Hs,
    HwStage::Es,
    HwStage::Gs,
    HwStage::Vs,
    HwStage::Ps,
    HwStage::Ms,
    HwStage::Cs,
];

fn root_code(r: RootType) -> u32 {
    match r {
        RootType::Constants => 0,
        RootType::Descriptor => 1,
        RootType::Table => 2,
        RootType::ImmediateSampler => 3,
        RootType::ResourceHeap => 4,
        RootType::SamplerHeap => 5,
        RootType::Other(x) => 6 + x,
    }
}

fn root_type(code: u32) -> RootType {
    match code {
        0 => RootType::Constants,
        1 => RootType::Descriptor,
        2 => RootType::Table,
        3 => RootType::ImmediateSampler,
        4 => RootType::ResourceHeap,
        5 => RootType::SamplerHeap,
        x => RootType::Other(x - 6),
    }
}

/// Little-endian words and length-prefixed lists.
fn encode(s: &HostShader) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    let word = |out: &mut Vec<u8>, v: u32| out.extend(v.to_le_bytes());
    let stage = STAGES.iter().position(|&x| x == s.stage).unwrap_or(0) as u32;
    word(&mut out, stage);
    word(&mut out, s.dxil.len() as u32);
    out.extend(&s.dxil);
    word(&mut out, s.user_data.len() as u32);
    for u in &s.user_data {
        for v in [u.index, u32::from(u.slot), root_code(u.root), u.dword] {
            word(&mut out, v);
        }
    }
    word(&mut out, s.samplers.len() as u32);
    for sampler in &s.samplers {
        for &v in sampler {
            word(&mut out, v);
        }
    }
    word(&mut out, s.static_samplers.len() as u32);
    for t in &s.static_samplers {
        let [u, v, w] = t.address;
        let words = [
            t.filter,
            u,
            v,
            w,
            t.mip_lod_bias.to_bits(),
            t.max_anisotropy,
            t.comparison,
            t.border,
            t.min_lod.to_bits(),
            t.max_lod.to_bits(),
        ];
        for v in words {
            word(&mut out, v);
        }
    }
    for slots in [&s.root_cbvs, &s.root_srvs] {
        word(&mut out, slots.len() as u32);
        out.extend(slots);
    }
    out
}

fn decode(bytes: &[u8]) -> Option<HostShader> {
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> Option<&'a [u8]> {
            let (head, rest) = self.0.split_at_checked(n)?;
            self.0 = rest;
            Some(head)
        }
        fn word(&mut self) -> Option<u32> {
            Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
        }
        fn list(&mut self) -> Option<Vec<u8>> {
            let n = self.word()? as usize;
            Some(self.take(n)?.to_vec())
        }
    }
    let mut r = Reader(bytes);
    if r.take(4)? != MAGIC {
        return None;
    }
    let stage = *STAGES.get(r.word()? as usize)?;
    let dxil = r.list()?;
    let n = r.word()? as usize;
    let mut user_data = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        user_data.push(UserData {
            index: r.word()?,
            slot: u8::try_from(r.word()?).ok()?,
            root: root_type(r.word()?),
            dword: r.word()?,
        });
    }
    let n = r.word()? as usize;
    let mut samplers = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        samplers.push([r.word()?, r.word()?, r.word()?, r.word()?]);
    }
    let n = r.word()? as usize;
    let mut static_samplers = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        static_samplers.push(StaticSampler {
            filter: r.word()?,
            address: [r.word()?, r.word()?, r.word()?],
            mip_lod_bias: f32::from_bits(r.word()?),
            max_anisotropy: r.word()?,
            comparison: r.word()?,
            border: r.word()?,
            min_lod: f32::from_bits(r.word()?),
            max_lod: f32::from_bits(r.word()?),
        });
    }
    let root_cbvs = r.list()?;
    let root_srvs = r.list()?;
    r.0.is_empty().then_some(HostShader {
        stage,
        dxil,
        user_data,
        samplers,
        static_samplers,
        root_cbvs,
        root_srvs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shader() -> HostShader {
        HostShader {
            stage: HwStage::Ps,
            dxil: b"DXBC and the rest".to_vec(),
            user_data: vec![UserData {
                index: 8,
                slot: 1,
                root: RootType::Table,
                dword: 0,
            }],
            samplers: vec![[0x400, 0xc0_0000, 0x18f0_0000, 0]],
            static_samplers: vec![StaticSampler::from_words([
                0x400,
                0xc0_0000,
                0x18f0_0000,
                0,
            ])],
            root_cbvs: vec![2],
            root_srvs: vec![0, 3],
        }
    }

    #[test]
    fn translations_come_back_from_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        let build = [7u8; 32];
        let container = b"a console container".as_slice();
        let runs = AtomicU64::new(0);
        let translate = |_: &[u8]| {
            runs.fetch_add(1, Ordering::Relaxed);
            Ok(shader())
        };
        assert_eq!(cached(&cache, &build, container, translate), Ok(shader()));
        assert_eq!(cached(&cache, &build, container, translate), Ok(shader()));
        assert_eq!(runs.load(Ordering::Relaxed), 1, "the second came from disk");

        // Another build translates again.
        assert_eq!(cached(&cache, &[8; 32], container, translate), Ok(shader()));
        assert_eq!(runs.load(Ordering::Relaxed), 2);

        // A damaged entry is a miss, translated and stored again.
        for e in walk(dir.path()) {
            std::fs::write(e, b"VSH1 broken").unwrap();
        }
        assert_eq!(cached(&cache, &build, container, translate), Ok(shader()));
        assert_eq!(runs.load(Ordering::Relaxed), 3);
        assert_eq!(cached(&cache, &build, container, translate), Ok(shader()));
        assert_eq!(runs.load(Ordering::Relaxed), 3);

        let fail = |_: &[u8]| {
            runs.fetch_add(1, Ordering::Relaxed);
            Err(ShaderError::Malformed("invalid header"))
        };
        for _ in 0..2 {
            assert!(cached(&cache, &build, b"other", fail).is_err());
        }
        assert_eq!(runs.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn unsupported_shader_is_retried_after_damage_or_target_change() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        let runs = AtomicU64::new(0);
        let fail = |_: &[u8]| {
            runs.fetch_add(1, Ordering::Relaxed);
            Err(ShaderError::Unsupported { offset: 16, what: "test operation" })
        };
        let key = cache_key(&[42; 32], Some(WaveRange { min: 32, max: 32 }));
        let reason = cached(&cache, &key, b"same", fail).unwrap_err();
        assert!(reason.contains("0x10: test operation is not supported"));
        assert_eq!(cached(&cache, &key, b"same", fail).unwrap_err(), reason);
        assert_eq!(runs.load(Ordering::Relaxed), 1);
        let path = walk(dir.path()).pop().unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.pop();
        std::fs::write(path, bytes).unwrap();
        assert_eq!(cached(&cache, &key, b"same", fail).unwrap_err(), reason);
        assert_eq!(runs.load(Ordering::Relaxed), 2);
        let wave64 = cache_key(&[42; 32], Some(WaveRange { min: 64, max: 64 }));
        assert_eq!(cached(&cache, &wave64, b"same", |_| Ok(shader())), Ok(shader()));
        assert_eq!(cached(&cache, &[43; 32], b"same", |_| Ok(shader())), Ok(shader()));
    }

    #[test]
    fn cache_write_failure_still_returns_the_translator_result() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("entries");
        let cache = Cache::open(&root).unwrap();
        std::fs::remove_dir(&root).unwrap();
        std::fs::write(root, b"no subdirectories here").unwrap();
        assert_eq!(cached(&cache, &[0; 32], b"valid", |_| Ok(shader())), Ok(shader()));
        let e = ShaderError::Unsupported { offset: 4, what: "test operation" };
        let reason = cached(&cache, &[0; 32], b"unsupported", |_| {
            Err(ShaderError::Unsupported { offset: 4, what: "test operation" })
        }).unwrap_err();
        assert_eq!(reason, e.to_string());
    }

    fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            if e.path().is_dir() {
                files.extend(walk(&e.path()));
            } else {
                files.push(e.path());
            }
        }
        files
    }

    #[test]
    fn entries_round_trip_and_reject_damage() {
        let bytes = encode(&shader());
        assert_eq!(decode(&bytes), Some(shader()));
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode(&[bytes.as_slice(), &[0]].concat()), None);
        assert_eq!(decode(b"VSH0"), None);
    }
}
