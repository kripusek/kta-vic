//! Captures a bounded prefix of the mesh records the GPU will actually see.

use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use windows::Win32::Graphics::Direct3D12::*;

use crate::host::{HostList, host, transition};

const MAX_CALLS: u32 = 32;
const MAX_RECORDS: u32 = 16;
const GROUP_BYTES: u64 = 12;
const COUNT_BYTES: u64 = 4;
static FRAME: AtomicU64 = AtomicU64::new(0);
static BUDGET: Mutex<(u64, u32)> = Mutex::new((0, 0));

pub(crate) fn arm(frame: u64) {
    *BUDGET.lock().unwrap_or_else(|e| e.into_inner()) = (frame, 0);
    FRAME.store(frame, Relaxed);
}

pub(crate) fn disarm(frame: u64) {
    let _ = FRAME.compare_exchange(frame, 0, Relaxed, Relaxed);
}

pub(crate) fn claim() -> Option<(u64, u32)> {
    let frame = FRAME.load(Relaxed);
    if frame == 0 || frame != crate::queue::presented() + 1 {
        return None;
    }
    let mut budget = BUDGET.lock().unwrap_or_else(|e| e.into_inner());
    if FRAME.load(Relaxed) != frame || budget.0 != frame || budget.1 >= MAX_CALLS {
        return None;
    }
    budget.1 += 1;
    if budget.1 == MAX_CALLS {
        tracing::info!(
            frame,
            limit = MAX_CALLS,
            "indirect mesh capture call limit reached"
        );
    }
    Some((frame, budget.1))
}

pub(crate) struct Call {
    pub(crate) frame: u64,
    pub(crate) sequence: u32,
    pub(crate) pipeline: usize,
    pub(crate) shaders: String,
    pub(crate) arguments: u64,
    pub(crate) max: u32,
    pub(crate) stride: u32,
}

impl Call {
    fn unavailable(&self, reason: &str) {
        tracing::info!(
            frame = self.frame, sequence = self.sequence,
            pipeline = format_args!("{:#x}", self.pipeline), shaders = %self.shaders,
            arguments = format_args!("{:#x}", self.arguments), max = self.max,
            stride = self.stride, reason, "captured indirect mesh groups unavailable"
        );
    }
}

pub(crate) struct Sample {
    call: Call,
    buffer: ID3D12Resource,
    prefix: u32,
    has_count: bool,
    reported: Cell<bool>,
}

impl Sample {
    fn data(&self) -> windows::core::Result<(u32, Option<u32>, Vec<[u32; 3]>)> {
        let mut mapped = std::ptr::null_mut();
        let count_at = u64::from(self.prefix) * GROUP_BYTES;
        let end = count_at + if self.has_count { COUNT_BYTES } else { 0 };
        if end == 0 {
            return Ok((0, None, Vec::new()));
        }
        let range = D3D12_RANGE {
            Begin: 0,
            End: end as usize,
        };
        // SAFETY: the submission fence has completed and the copy fits the readback.
        let bytes = unsafe {
            self.buffer.Map(0, Some(&range), Some(&mut mapped))?;
            std::slice::from_raw_parts(mapped.cast::<u8>(), end as usize)
        };
        let word = |at: usize| u32::from_le_bytes(bytes[at..][..4].try_into().unwrap());
        let raw_count = self.has_count.then(|| word(count_at as usize));
        let effective = raw_count.unwrap_or(self.call.max).min(self.call.max);
        let groups = (0..effective.min(self.prefix) as usize)
            .map(|i| [word(i * 12), word(i * 12 + 4), word(i * 12 + 8)])
            .collect();
        // SAFETY: the mapping is no longer read and was never written.
        unsafe {
            self.buffer
                .Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }))
        };
        Ok((effective, raw_count, groups))
    }

    /// Report once. The recorded copy still references the buffer on replay.
    pub(crate) fn report(&self) {
        if self.reported.replace(true) {
            return;
        }
        let Ok((effective, raw_count, groups)) = self.data() else {
            self.call.unavailable("readback mapping failed");
            return;
        };
        let call = &self.call;
        tracing::info!(
            frame = call.frame, sequence = call.sequence,
            pipeline = format_args!("{:#x}", call.pipeline), shaders = %call.shaders,
            arguments = format_args!("{:#x}", call.arguments), max = call.max,
            stride = call.stride, raw_count, effective, sampled = groups.len(),
            truncated = effective > self.prefix, ?groups,
            "captured indirect mesh groups after clamp"
        );
    }
}

/// Records immediately before ExecuteIndirect. Only the argument resource's
/// state is known here: the clamp left it in INDIRECT_ARGUMENT.
pub(crate) fn record(
    list: &HostList,
    call: Call,
    (args, offset): (&ID3D12Resource, u64),
    count: Option<(&ID3D12Resource, u64)>,
) {
    match snapshot(list, &call, (args, offset), count) {
        Ok((buffer, prefix)) => list.keep_mesh_sample(Sample {
            call,
            buffer,
            prefix,
            has_count: count.is_some(),
            reported: Cell::default(),
        }),
        Err(reason) => call.unavailable(reason),
    }
}

fn snapshot(
    list: &HostList,
    call: &Call,
    (args, offset): (&ID3D12Resource, u64),
    count: Option<(&ID3D12Resource, u64)>,
) -> Result<(ID3D12Resource, u32), &'static str> {
    let count_offset = match count {
        Some((buffer, offset)) if buffer == args => Some(offset),
        Some(_) => return Err("separate count buffer state is unknown"),
        None => None,
    };
    // SAFETY: the recorded list holds the resource.
    let width = unsafe { args.GetDesc().Width };
    let fits = |at: u64, bytes| {
        at.is_multiple_of(4) && at.checked_add(bytes).is_some_and(|end| end <= width)
    };
    let prefix = call.max.min(MAX_RECORDS);
    let stride = u64::from(call.stride);
    if stride < GROUP_BYTES
        || !stride.is_multiple_of(4)
        || !offset
            .checked_add(u64::from(prefix.saturating_sub(1)) * stride)
            .is_some_and(|last| fits(last, GROUP_BYTES))
        || count_offset.is_some_and(|at| !fits(at, COUNT_BYTES))
    {
        return Err("invalid argument or count range");
    }
    let size = (u64::from(prefix) * GROUP_BYTES
        + if count_offset.is_some() {
            COUNT_BYTES
        } else {
            0
        })
    .max(COUNT_BYTES);
    let device = &host().ok_or("host device unavailable")?.device;
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_READBACK,
        ..Default::default()
    };
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: size,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        ..Default::default()
    };
    let mut buffer: Option<ID3D12Resource> = None;
    // SAFETY: a buffer with the required readback state and a valid out parameter.
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            &desc,
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
            &mut buffer,
        )
    }
    .map_err(|_| "readback allocation failed")?;
    let buffer = buffer.ok_or("readback allocation returned no buffer")?;
    if prefix != 0 || count_offset.is_some() {
        let indirect = D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT;
        let copy = D3D12_RESOURCE_STATE_COPY_SOURCE;
        // SAFETY: the clamp explicitly set INDIRECT_ARGUMENT; each range was
        // checked above. Restore the original state before ExecuteIndirect.
        unsafe {
            list.list
                .ResourceBarrier(&[transition(args, indirect, copy)]);
            for i in (0..prefix).map(u64::from) {
                list.list.CopyBufferRegion(
                    &buffer,
                    i * GROUP_BYTES,
                    args,
                    offset + i * stride,
                    GROUP_BYTES,
                );
            }
            if let Some(at) = count_offset {
                list.list.CopyBufferRegion(
                    &buffer,
                    u64::from(prefix) * GROUP_BYTES,
                    args,
                    at,
                    COUNT_BYTES,
                );
            }
            list.list
                .ResourceBarrier(&[transition(args, copy, indirect)]);
        }
    }
    Ok((buffer, prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::Interface;

    const WRITER: &str = r#"
#define RS "RootConstants(num32BitConstants=1, b0), UAV(u0)"
cbuffer Params : register(b0) { uint count; };
RWByteAddressBuffer args : register(u0);
[RootSignature(RS)]
[numthreads(1,1,1)]
void main() {
    args.Store3(16, uint3(99999, 5, 7));
    args.Store3(48, uint3(2, 1, 1));
    args.Store3(80, uint3(9, 1, 1));
    for (uint i = 3; i < 20; ++i) args.Store3(16 + i * 32, uint3(i, 1, 1));
    args.Store(1024, count);
}
"#;

    fn call(max: u32) -> Call {
        Call {
            frame: 17,
            sequence: 1,
            pipeline: 0,
            shaders: String::new(),
            arguments: 16,
            max,
            stride: 32,
        }
    }

    #[test]
    fn warp_indirect_mesh_snapshot_reads_gpu_counts_and_clamped_strided_args() {
        if std::env::var_os("VICTORY_D3D12_WARP").is_none() {
            return;
        }
        let host = host().expect("software WARP host");
        if std::env::var_os("VICTORY_D3D12_DEBUG").is_some() {
            host.device
                .cast::<ID3D12InfoQueue>()
                .expect("WARP debug layer");
        }
        let writer = super::super::make_for(&host.device, WRITER).unwrap();
        for (count, max) in [
            (None, 3),
            (Some(0u32), 3),
            (Some(2), 3),
            (Some(9), 3),
            (Some(19), 20),
        ] {
            let list = host.new_list().unwrap();
            let (args, _) = host.shared_buffer(64 << 10).unwrap();
            // SAFETY: open list, live resources and a producer with an explicit UAV state.
            let gpu = unsafe {
                let gpu = args.GetGPUVirtualAddress();
                list.list.ResourceBarrier(&[transition(
                    &args,
                    D3D12_RESOURCE_STATE_COMMON,
                    D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                )]);
                list.list.SetComputeRootSignature(&writer.root);
                list.list.SetPipelineState(&writer.pso);
                list.list
                    .SetComputeRoot32BitConstant(0, count.unwrap_or(0), 0);
                list.list.SetComputeRootUnorderedAccessView(1, gpu);
                list.list.Dispatch(1, 1, 1);
                gpu
            };
            assert!(super::super::bound(&list, &args, gpu + 16, (10, 32, max)));
            let call = call(max);
            let (readback, prefix) =
                snapshot(&list, &call, (&args, 16), count.map(|_| (&args, 1024))).unwrap();
            if let Ok(debug) = list.list.cast::<ID3D12DebugCommandList>() {
                // SAFETY: an open list and the restored argument resource.
                let state = unsafe {
                    debug.AssertResourceState(
                        &args,
                        0,
                        D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT.0 as u32,
                    )
                };
                assert!(state.as_bool());
            }
            super::super::done(&list, &args);
            // SAFETY: the original buffer is back in UAV state for a later
            // producer. Reuse it before submitting the list and mapping the copy.
            unsafe {
                list.list.SetComputeRootSignature(&writer.root);
                list.list.SetPipelineState(&writer.pso);
                list.list.SetComputeRoot32BitConstant(0, 99, 0);
                list.list.SetComputeRootUnorderedAccessView(1, gpu);
                list.list.Dispatch(1, 1, 1);
            }
            list.close();
            assert_eq!(host.execute_checked(&[&list]), None);
            let sample = Sample {
                call,
                buffer: readback,
                prefix,
                has_count: count.is_some(),
                reported: Cell::default(),
            };
            let (effective, raw_count, groups) = sample.data().unwrap();
            assert_eq!(raw_count, count);
            assert_eq!(effective, count.unwrap_or(max).min(max));
            let expected = [[65535, 1, 1], [2, 1, 1], [9, 1, 1]]
                .into_iter()
                .chain((3..MAX_RECORDS).map(|i| [i, 1, 1]))
                .take(effective.min(MAX_RECORDS) as usize)
                .collect::<Vec<_>>();
            assert_eq!(groups, expected);
        }
    }

    #[test]
    fn warp_indirect_mesh_snapshot_rejects_unknown_state_and_invalid_ranges() {
        if std::env::var_os("VICTORY_D3D12_WARP").is_none() {
            return;
        }
        let host = host().expect("software WARP host");
        let list = host.new_list().unwrap();
        let (args, _) = host.shared_buffer(64 << 10).unwrap();
        let (other, _) = host.shared_buffer(64 << 10).unwrap();
        let call = call(3);
        assert_eq!(
            snapshot(&list, &call, (&args, 16), Some((&other, 4))).unwrap_err(),
            "separate count buffer state is unknown",
        );
        assert_eq!(
            snapshot(&list, &call, (&args, 16), Some((&args, 6))).unwrap_err(),
            "invalid argument or count range",
        );
        assert_eq!(
            snapshot(&list, &call, (&args, 64 << 10), None).unwrap_err(),
            "invalid argument or count range",
        );
        list.close();
        assert_eq!(host.execute_checked(&[&list]), None);
    }
}
