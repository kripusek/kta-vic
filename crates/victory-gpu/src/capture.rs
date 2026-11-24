//! Presented frames written to PNG files, to see what reaches the screen:
//! with `VICTORY_CAPTURE=<folder>`, every frame whose number is a power of
//! two (the frames the log reports) is saved as `frame-<n>.png`, at half
//! size. With `VICTORY_CAPTURE_ON_DEMAND=1`, an empty `capture-next` file
//! in that folder requests a frame instead. Explicitly listed frames are
//! saved in both modes. Only in builds with diagnostics.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// The folder frames go to, if capturing.
pub(crate) fn folder() -> Option<&'static PathBuf> {
    static FOLDER: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    FOLDER
        .get_or_init(|| std::env::var_os("VICTORY_CAPTURE").map(PathBuf::from))
        .as_ref()
}

/// Frames listed in `VICTORY_CAPTURE_FRAMES` (comma-separated): saved
/// too, and their draws traced at debug level.
pub(crate) fn listed(n: u64) -> bool {
    static LIST: std::sync::OnceLock<Vec<u64>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| {
        let list = std::env::var("VICTORY_CAPTURE_FRAMES").unwrap_or_default();
        list.split(',')
            .filter_map(|f| f.trim().parse().ok())
            .collect()
    })
    .contains(&n)
}

/// Whether frame `n` is saved.
pub(crate) fn wanted(n: u64) -> bool {
    static MARKER: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
        let folder = folder()?;
        (std::env::var("VICTORY_CAPTURE_ON_DEMAND").as_deref() == Ok("1"))
            .then(|| folder.join("capture-next"))
    });
    static NEXT_POLL: Mutex<Option<Instant>> = Mutex::new(None);
    let Some(marker) = MARKER.as_ref() else {
        return folder().is_some() && (n.is_power_of_two() || listed(n));
    };
    let mut next_poll = NEXT_POLL.lock().unwrap_or_else(|e| e.into_inner());
    poll_marker(marker, &mut next_poll, Instant::now()) || listed(n) || TARGET_REQUEST.armed(n)
}

fn poll_marker(path: &Path, next_poll: &mut Option<Instant>, now: Instant) -> bool {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DISPOSITION_INFO, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FileDispositionInfo, SetFileInformationByHandle,
    };

    if next_poll.is_some_and(|at| now < at) {
        return false;
    }
    *next_poll = Some(now + Duration::from_millis(100));
    let Ok(file) = std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES | DELETE)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
    else {
        return false;
    };
    let Ok(meta) = file.metadata() else {
        return false;
    };
    if !meta.is_file()
        || meta.len() != 0
        || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return false;
    }
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: the handle has DELETE access and remains exclusive from the size
    // check through deletion. A replacement file cannot be consumed by mistake.
    unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&raw const disposition).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        ) != 0
    }
}

/// Saves 8-bit RGBA or BGRA rows (`pitch` bytes apart) as frame `n`.
pub(crate) fn save(n: u64, width: u32, height: u32, pitch: usize, bgra: bool, rows: &[u8]) {
    save_as(&format!("frame-{n}"), width, height, pitch, bgra, rows);
}

/// Saves 8-bit RGBA or BGRA rows as `<name>.png`, at half size.
fn save_as(name: &str, width: u32, height: u32, pitch: usize, bgra: bool, rows: &[u8]) {
    let Some(folder) = folder() else { return };
    let (w, h) = ((width / 2).max(1), (height / 2).max(1));
    // Each row starts with a zero filter byte, followed by downsampled RGB.
    let row_bytes = w as usize * 3 + 1;
    let mut raw = vec![0; row_bytes * h as usize];
    for (y, scanline) in raw.chunks_exact_mut(row_bytes).enumerate() {
        let row = &rows[(2 * y).min(height as usize - 1) * pitch..];
        for (x, pixel) in scanline[1..].chunks_exact_mut(3).enumerate() {
            let p = &row[(2 * x).min(width as usize - 1) * 4..][..3];
            if bgra {
                pixel.copy_from_slice(&[p[2], p[1], p[0]]);
            } else {
                pixel.copy_from_slice(p);
            }
        }
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend(w.to_be_bytes());
    header.extend(h.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]); // 8-bit RGB
    chunk(&mut png, b"IHDR", &header);
    chunk(
        &mut png,
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 3),
    );
    chunk(&mut png, b"IEND", &[]);
    let path = folder.join(format!("{name}.png"));
    match std::fs::write(&path, png) {
        Ok(()) => tracing::info!(path = %path.display(), "frame saved"),
        Err(e) => tracing::warn!(error = %e, path = %path.display(), "frame not saved"),
    }
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend((data.len() as u32).to_be_bytes());
    let start = png.len();
    png.extend(kind);
    png.extend(data);
    let crc = crc32(&png[start..]);
    png.extend(crc.to_be_bytes());
}

const CRC32_TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut byte = 0;
    while byte < table.len() {
        let mut crc = byte as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
            bit += 1;
        }
        table[byte] = crc;
        byte += 1;
    }
    table
};

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc = (crc >> 8) ^ CRC32_TABLE[usize::from((crc as u8) ^ byte)];
    }
    !crc
}

/// Records a copy of `texture` (in the copy-source state) into a
/// readback buffer.
pub(crate) fn read_back(
    device: &ID3D12Device,
    list: &ID3D12GraphicsCommandList,
    texture: &ID3D12Resource,
) -> Option<(ID3D12Resource, D3D12_PLACED_SUBRESOURCE_FOOTPRINT)> {
    let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
    let mut total = 0;
    // SAFETY: a live texture; outputs for one subresource.
    unsafe {
        let desc = texture.GetDesc();
        device.GetCopyableFootprints(
            &desc,
            0,
            1,
            0,
            Some(&mut footprint),
            None,
            None,
            Some(&mut total),
        );
    }
    if total == u64::MAX {
        return None;
    }
    // Our CPU readers take a slice of complete pitched rows, including
    // padding in the last row that the GPU copy does not require.
    let f = footprint.Footprint;
    let pitched_bytes = u64::from(f.RowPitch) * u64::from(f.Height);
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_READBACK,
        ..Default::default()
    };
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: total.max(pitched_bytes),
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
    // SAFETY: plain creation; an open list; locations naming live resources.
    unsafe {
        device
            .CreateCommittedResource(
                &heap,
                D3D12_HEAP_FLAG_NONE,
                &desc,
                D3D12_RESOURCE_STATE_COPY_DEST,
                None,
                &mut buffer,
            )
            .ok()?;
        let buffer = buffer?;
        let to = D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::transmute_copy(&buffer),
            Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                PlacedFootprint: footprint,
            },
        };
        let from = D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::transmute_copy(texture),
            Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                SubresourceIndex: 0,
            },
        };
        list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
        Some((buffer, footprint))
    }
}

/// Saves a frame read back by [`read_back`] (8-bit color only).
pub(crate) fn save_read_back(
    frame: u64,
    buffer: &ID3D12Resource,
    footprint: &D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
) {
    let f = footprint.Footprint;
    let bgra = match f.Format {
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => false,
        DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => true,
        other => {
            tracing::warn!(format = other.0, "frame not saved: not 8-bit color");
            return;
        }
    };
    let mut data = std::ptr::null_mut();
    // SAFETY: the copy finished (every submission is waited for); the
    // mapping spans the footprint's rows.
    unsafe {
        if buffer.Map(0, None, Some(&mut data)).is_err() {
            return;
        }
        let len = f.RowPitch as usize * f.Height as usize;
        let rows = std::slice::from_raw_parts(data.cast::<u8>(), len);
        save(frame, f.Width, f.Height, f.RowPitch as usize, bgra, rows);
        buffer.Unmap(0, None);
    }
}

/// With `VICTORY_DUMP_TARGETS` set: title textures the frames listed in
/// `VICTORY_CAPTURE_FRAMES` render to or write, in first-use order, saved
/// at that frame's present as `frame-<n>-<i>-<resource>-<w>x<h>-f<format>.png`
/// (what each holds at the end of the frame; formats normalized to show
/// structure: floats scaled by their largest value, depth by its range).
/// The host copies are held: the title may release a target before the
/// frame is presented.
type FrameTargets = Vec<(usize, Arc<crate::resource::HostTexture>)>;
static TARGETS: Mutex<BTreeMap<u64, FrameTargets>> = Mutex::new(BTreeMap::new());

#[derive(Debug, PartialEq, Eq)]
enum TargetFilter {
    All,
    Match {
        width: u64,
        height: u32,
        format: i32,
    },
}

impl TargetFilter {
    fn parse(value: &str) -> Option<Self> {
        if value == "1" || value.is_empty() {
            return Some(Self::All);
        }
        let (size, format) = value.split_once("-f")?;
        let (width, height) = size.split_once('x')?;
        let (width, height, format) = (
            width.parse().ok()?,
            height.parse().ok()?,
            format.parse().ok()?,
        );
        (width > 0 && height > 0 && format > 0).then_some(Self::Match {
            width,
            height,
            format,
        })
    }

    fn matches(&self, desc: &D3D12_RESOURCE_DESC) -> bool {
        desc.SampleDesc.Count == 1
            && desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D
            && match *self {
                Self::All => true,
                Self::Match {
                    width,
                    height,
                    format,
                } => (desc.Width, desc.Height, desc.Format.0) == (width, height, format),
            }
    }
}

fn target_filter() -> Option<&'static TargetFilter> {
    static FILTER: LazyLock<Option<TargetFilter>> = LazyLock::new(|| {
        folder()?;
        let value = std::env::var("VICTORY_DUMP_TARGETS").ok()?;
        let filter = TargetFilter::parse(&value);
        if filter.is_none() {
            tracing::warn!(%value, "invalid VICTORY_DUMP_TARGETS; use 1 or <width>x<height>-f<format>");
        }
        filter
    });
    FILTER.as_ref()
}

struct TargetRequest {
    frame: AtomicU64,
    next_poll: Mutex<Option<Instant>>,
}

static TARGET_REQUEST: TargetRequest = TargetRequest {
    frame: AtomicU64::new(0),
    next_poll: Mutex::new(None),
};

impl TargetRequest {
    fn armed(&self, frame: u64) -> bool {
        frame != 0 && self.frame.load(Relaxed) == frame
    }

    fn poll(&self, marker: &Path, present: u64, now: Instant) -> Option<u64> {
        let mut next = self.next_poll.lock().unwrap_or_else(|e| e.into_inner());
        if self.frame.load(Relaxed) >= present || !poll_marker(marker, &mut next, now) {
            return None;
        }
        // One intervening frame leaves time for this arm to become visible
        // before the title starts recording the next frame's targets.
        let selected = present.checked_add(2)?;
        self.frame.store(selected, Relaxed);
        Some(selected)
    }
}

/// Arm a single future frame when the dedicated on-demand marker appears.
pub(crate) fn poll_target_request(present: u64) {
    static MARKER: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
        let TargetFilter::Match { .. } = target_filter()? else { return None };
        (std::env::var("VICTORY_CAPTURE_ON_DEMAND").as_deref() == Ok("1"))
            .then(|| folder().unwrap().join("capture-target-next"))
    });
    if let Some(marker) = MARKER.as_ref()
        && let Some(frame) = TARGET_REQUEST.poll(marker, present, Instant::now())
    {
        tracing::info!(frame, "target and presentation capture armed");
    }
}

pub(crate) fn wants_targets(frame: u64) -> bool {
    listed(frame) || TARGET_REQUEST.armed(frame)
}

/// Notes a title texture the frame being built renders to or writes.
pub(crate) fn note_target(resource: crate::object::This) {
    if resource.is_null() {
        return;
    }
    let Some(filter) = target_filter() else {
        return;
    };
    let frame = crate::queue::presented() + 1;
    if !wants_targets(frame) {
        return;
    }
    let Some(copy) = crate::resource::host_copy(resource) else {
        return;
    };
    // SAFETY: the retained copy owns the texture.
    if !filter.matches(&unsafe { copy.texture.GetDesc() }) {
        return;
    }
    let mut frames = TARGETS.lock().unwrap_or_else(|e| e.into_inner());
    if crate::queue::presented() + 1 != frame {
        return;
    }
    let targets = frames.entry(frame).or_default();
    if !targets.iter().any(|(_, prior)| Arc::ptr_eq(prior, &copy)) {
        // The title address is just a label, even after the resource is released.
        targets.push((resource as usize, copy));
    }
}

/// At the present of frame `frame`: saves the noted targets.
pub(crate) fn dump_targets(frame: u64) {
    let targets = TARGETS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&frame);
    let Some(targets) = targets else { return };
    let Some(host) = crate::host::host() else {
        return;
    };
    for (i, (label, copy)) in targets.iter().enumerate() {
        dump_one(host, copy, frame, i, *label);
    }
}

fn dump_one(
    host: &crate::host::Host,
    copy: &crate::resource::HostTexture,
    frame: u64,
    i: usize,
    label: usize,
) {
    let Some((buffer, footprint)) = host.read_target(copy) else {
        return;
    };
    let f = footprint.Footprint;
    let mut data = std::ptr::null_mut();
    // SAFETY: the copy finished; the mapping spans the footprint's rows.
    unsafe {
        if buffer.Map(0, None, Some(&mut data)).is_err() {
            return;
        }
        let pitch = f.RowPitch as usize;
        let rows = std::slice::from_raw_parts(data.cast::<u8>(), pitch * f.Height as usize);
        if let Some(rgba) = to_rgba(
            f.Format.0 as u32,
            f.Width as usize,
            f.Height as usize,
            pitch,
            rows,
        ) {
            let name = format!(
                "frame-{frame}-{i:02}-0x{label:x}-{}x{}-f{}",
                f.Width,
                f.Height,
                copy.texture.GetDesc().Format.0
            );
            save_as(&name, f.Width, f.Height, f.Width as usize * 4, false, &rgba);
        } else {
            tracing::info!(format = f.Format.0, label, "target not dumped: format");
        }
        buffer.Unmap(0, None);
    }
}

fn half(h: u16) -> f32 {
    let (sign, exp, man) = ((h >> 15) as u32, ((h >> 10) & 0x1f) as i32, (h & 0x3ff) as f32);
    let v = match exp {
        0 => man * 2f32.powi(-24),
        31 => f32::INFINITY,
        e => (1.0 + man / 1024.0) * 2f32.powi(e - 15),
    };
    if sign == 1 { -v } else { v }
}

/// A float packed in `bits` with a 5-bit exponent and `man` mantissa bits
/// (the 11- and 10-bit floats of R11G11B10).
fn small_float(bits: u32, man: u32) -> f32 {
    let (exp, m) = ((bits >> man) & 0x1f, (bits & ((1 << man) - 1)) as f32);
    match exp {
        0 => m / (1 << man) as f32 * 2f32.powi(-14),
        31 => f32::INFINITY,
        e => (1.0 + m / (1 << man) as f32) * 2f32.powi(e as i32 - 15),
    }
}

/// Rows of a host format as 8-bit RGBA: 8-bit color as is, everything else
/// as channel values scaled so the largest finite one is white (single
/// channels: lowest black, highest white, as grey). `None` for formats not
/// handled.
fn to_rgba(format: u32, w: usize, h: usize, pitch: usize, rows: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(w * h * 4);
    let px = |x: usize, y: usize, n: usize| &rows[y * pitch + x * n..][..n];
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap_or([0; 4]));
    // Channel values as floats, then scaled.
    type Decode = Box<dyn Fn(&[u8]) -> [f32; 3]>;
    let (n, decode): (usize, Decode) = match format {
        27..=32 => {
            for y in 0..h {
                for x in 0..w {
                    let p = px(x, y, 4);
                    out.extend([p[0], p[1], p[2], 255]);
                }
            }
            return Some(out);
        }
        87..=93 => {
            for y in 0..h {
                for x in 0..w {
                    let p = px(x, y, 4);
                    out.extend([p[2], p[1], p[0], 255]);
                }
            }
            return Some(out);
        }
        23..=25 => (4, Box::new(move |b| {
            let v = u32_at(b, 0);
            [(v & 0x3ff) as f32, (v >> 10 & 0x3ff) as f32, (v >> 20 & 0x3ff) as f32]
        })),
        26 => (4, Box::new(move |b| {
            let v = u32_at(b, 0);
            [small_float(v & 0x7ff, 6), small_float(v >> 11 & 0x7ff, 6), small_float(v >> 22, 5)]
        })),
        9..=11 => (8, Box::new(move |b| [half(u16_at(b, 0)), half(u16_at(b, 1)), half(u16_at(b, 2))])),
        33..=34 => (4, Box::new(move |b| [half(u16_at(b, 0)), half(u16_at(b, 1)), 0.0])),
        1..=2 => (16, Box::new(move |b| {
            [0, 1, 2].map(|i| f32::from_bits(u32_at(b, i)))
        })),
        15..=16 => (8, Box::new(move |b| [f32::from_bits(u32_at(b, 0)), f32::from_bits(u32_at(b, 1)), 0.0])),
        35..=38 => (4, Box::new(move |b| [f32::from(u16_at(b, 0)), f32::from(u16_at(b, 1)), 0.0])),
        48..=52 => (2, Box::new(move |b| [f32::from(b[0]), f32::from(b[1]), 0.0])),
        // Single channels (and depth planes): shown as grey.
        39..=43 => (4, Box::new(move |b| [f32::from_bits(u32_at(b, 0)); 3])),
        19..=22 => (8, Box::new(move |b| [f32::from_bits(u32_at(b, 0)); 3])),
        44..=47 => (4, Box::new(move |b| [(u32_at(b, 0) & 0xff_ffff) as f32; 3])),
        53..=54 => (2, Box::new(move |b| [half(u16_at(b, 0)); 3])),
        55..=59 => (2, Box::new(move |b| [f32::from(u16_at(b, 0)); 3])),
        60..=65 => (1, Box::new(move |b| [f32::from(b[0]); 3])),
        _ => return None,
    };
    let values: Vec<[f32; 3]> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| decode(px(x, y, n)))
        .collect();
    let grey = matches!(format, 19..=22 | 39..=47 | 53..=65);
    let finite = values.iter().flatten().copied().filter(|v| v.is_finite());
    let (low, high) = if grey {
        finite.fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)))
    } else {
        (0.0, finite.fold(0.0f32, f32::max))
    };
    let range = (high - low).max(1e-20);
    tracing::info!(format, low, high, "target values");
    for v in values {
        let c = v.map(|c| (((c - low) / range).clamp(0.0, 1.0) * 255.0) as u8);
        out.extend([c[0], c[1], c[2], 255]);
    }
    Some(out)
}

/// Pixels per side of the grid [`probe`] reads.
const GRID: u32 = 4;
/// Bytes between the grid's pixels in the readback buffer (placement
/// alignment).
const SLOT: u64 = 512;

/// Records copies of a grid of pixels of `texture` (8-bit color, in the
/// copy-source state) into a readback buffer, to tell black frames.
pub(crate) fn probe(
    device: &ID3D12Device,
    list: &ID3D12GraphicsCommandList,
    texture: &ID3D12Resource,
) -> Option<ID3D12Resource> {
    static FAST: LazyLock<bool> =
        LazyLock::new(|| std::env::var("VICTORY_CAPTURE_FAST").as_deref() == Ok("1"));
    if *FAST {
        return None;
    }
    // SAFETY: a live resource.
    let d = unsafe { texture.GetDesc() };
    if !matches!(d.Format.0, 27..=29 | 87 | 90 | 91) {
        return None;
    }
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: SLOT * u64::from(GRID * GRID),
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
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_READBACK,
        ..Default::default()
    };
    let mut buffer: Option<ID3D12Resource> = None;
    // SAFETY: plain creation; an open list; locations naming live resources.
    unsafe {
        device
            .CreateCommittedResource(
                &heap,
                D3D12_HEAP_FLAG_NONE,
                &desc,
                D3D12_RESOURCE_STATE_COPY_DEST,
                None,
                &mut buffer,
            )
            .ok()?;
        let buffer = buffer?;
        for i in 0..GRID * GRID {
            let (x, y) = (
                (2 * (i % GRID) + 1) * d.Width as u32 / (2 * GRID),
                (2 * (i / GRID) + 1) * d.Height / (2 * GRID),
            );
            let to = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&buffer),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: SLOT * u64::from(i),
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                            Format: d.Format,
                            Width: 1,
                            Height: 1,
                            Depth: 1,
                            RowPitch: 256,
                        },
                    },
                },
            };
            let from = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(texture),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            };
            let pixel = D3D12_BOX {
                left: x,
                top: y,
                front: 0,
                right: x + 1,
                bottom: y + 1,
                back: 1,
            };
            list.CopyTextureRegion(&to, 0, 0, 0, &from, Some(&pixel));
        }
        Some(buffer)
    }
}

/// Whether every pixel [`probe`] read is (near) black.
pub(crate) fn black(buffer: &ID3D12Resource) -> bool {
    let mut data = std::ptr::null_mut();
    // SAFETY: the copies finished (every submission is waited for).
    unsafe {
        if buffer.Map(0, None, Some(&mut data)).is_err() {
            return false;
        }
        let bytes =
            std::slice::from_raw_parts(data.cast::<u8>(), (SLOT * u64::from(GRID * GRID)) as usize);
        let level = bytes
            .chunks(SLOT as usize)
            .flat_map(|pixel| pixel[..3].iter().copied())
            .max()
            .unwrap_or(0);
        buffer.Unmap(0, None);
        let mut levels = LEVELS.lock().unwrap_or_else(|e| e.into_inner());
        if levels.len() == 12 {
            levels.pop_front();
        }
        levels.push_back(level);
        level < 8
    }
}

/// The brightest probed channel of the last frames, oldest first: a fade
/// darkens over several, a glitch drops at once.
static LEVELS: std::sync::Mutex<std::collections::VecDeque<u8>> =
    std::sync::Mutex::new(std::collections::VecDeque::new());

pub(crate) fn recent_levels() -> Vec<u8> {
    let levels = LEVELS.lock().unwrap_or_else(|e| e.into_inner());
    levels.iter().copied().collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn target_marker_arms_one_future_frame() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("capture-target-next");
        let request = TargetRequest {
            frame: AtomicU64::new(0),
            next_poll: Mutex::new(None),
        };
        let now = Instant::now();
        assert!(!request.armed(0));
        std::fs::write(&marker, []).unwrap();
        assert_eq!(request.poll(&marker, 40, now), Some(42));
        assert!(!marker.exists());
        assert!(!request.armed(40));
        assert!(!request.armed(41));
        assert!(request.armed(42));
        assert!(!request.armed(43));
        assert_eq!(request.poll(&marker, 43, now + Duration::from_secs(1)), None);
        assert!(!request.armed(45));
    }

    #[test]
    fn target_filter_keeps_only_matching_single_sample_textures() {
        use super::*;
        let filter = TargetFilter::parse("2560x1440-f28").unwrap();
        let mut d = D3D12_RESOURCE_DESC {
            Width: 2560,
            Height: 1440,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        assert!(filter.matches(&d));
        d.Width -= 1;
        assert!(!filter.matches(&d));
        assert!(TargetFilter::parse("1").unwrap().matches(&d));
        d.Width += 1;
        d.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
        assert!(!filter.matches(&d));
        d.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
        d.SampleDesc.Count = 4;
        assert!(!filter.matches(&d));
        for bad in ["0x1440-f28", "2560x1440-f0", "2560x1440", "2560x1440-f28,"] {
            assert!(TargetFilter::parse(bad).is_none());
        }
    }

    #[test]
    fn capture_marker_consumes_only_empty_files_and_throttles_polls() {
        use super::{Duration, Instant, poll_marker};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture-next");
        let mut next = None;
        let mut now = Instant::now();
        let interval = Duration::from_millis(100);
        assert!(!poll_marker(&path, &mut next, now));
        std::fs::write(&path, []).unwrap();
        assert!(!poll_marker(
            &path,
            &mut next,
            now + Duration::from_millis(99)
        ));
        now += interval;
        assert!(poll_marker(&path, &mut next, now));
        assert!(!path.exists());
        now += interval;
        assert!(!poll_marker(&path, &mut next, now));
        std::fs::write(&path, b"keep").unwrap();
        now += interval;
        assert!(!poll_marker(&path, &mut next, now));
        assert_eq!(std::fs::read(&path).unwrap(), b"keep");
    }

    #[test]
    fn crc_matches_the_png_check_value() {
        assert_eq!(super::crc32(b"IEND"), 0xae42_6082);
    }
}
