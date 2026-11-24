use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{PCSTR, s};

use super::transition;

const MAX_PLANES: u32 = 8;
const FULL_DESTINATION: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PresentGeometry {
    /// Left, top, right, bottom in source texels. None means the whole plane.
    pub(crate) source: Option<[u32; 4]>,
    /// Left, top, right, bottom in display-relative units.
    pub(crate) destination: [f32; 4],
}

impl Default for PresentGeometry {
    fn default() -> Self {
        Self {
            source: None,
            destination: FULL_DESTINATION,
        }
    }
}

/// Built-in fullscreen-triangle compositor for multi-plane `PresentX` calls
/// (e.g. Series S presenting a 1080p scene plane with a 4K UI overlay plane).
pub(super) struct Compositor {
    pub(super) format: DXGI_FORMAT,
    root: ID3D12RootSignature,
    opaque: ID3D12PipelineState,
    overlay: ID3D12PipelineState,
    rtv_heap: ID3D12DescriptorHeap,
    srv_heap: ID3D12DescriptorHeap,
    srv_stride: u32,
}

impl Compositor {
    pub(super) fn new(device: &ID3D12Device, format: DXGI_FORMAT) -> windows::core::Result<Self> {
        const SOURCE: &str = r#"
            #define RS "DescriptorTable(SRV(t0), visibility=SHADER_VISIBILITY_PIXEL), \
                        RootConstants(num32BitConstants=9, b0, visibility=SHADER_VISIBILITY_PIXEL), \
                        StaticSampler(s0, filter=FILTER_MIN_MAG_MIP_LINEAR, \
                            addressU=TEXTURE_ADDRESS_CLAMP, addressV=TEXTURE_ADDRESS_CLAMP, \
                            visibility=SHADER_VISIBILITY_PIXEL)"
            Texture2D tex : register(t0);
            SamplerState smp : register(s0);
            cbuffer Plane : register(b0) {
                float4 source_edges;
                float4 sample_centers;
                uint encode_srgb;
            };
            [RootSignature(RS)]
            void vs(uint id : SV_VertexID, out float4 pos : SV_Position, out float2 uv : TEXCOORD) {
                uv = float2((id << 1) & 2, id & 2);
                pos = float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
            }
            [RootSignature(RS)]
            float4 ps(float4 pos : SV_Position, float2 uv : TEXCOORD) : SV_Target {
                uv = lerp(source_edges.xy, source_edges.zw, uv);
                // Linear filtering cannot sample texels outside the source rectangle.
                float4 color = tex.Sample(smp, clamp(uv, sample_centers.xy, sample_centers.zw));
                if (encode_srgb != 0) {
                    // The swap-chain RTV is UNORM. Restore the encoded RGB
                    // components after sampling through a legal typed SRV.
                    float3 lo = color.rgb * 12.92;
                    float3 hi = 1.055 * pow(color.rgb, 1.0 / 2.4) - 0.055;
                    color.rgb = lerp(lo, hi, step(0.0031308, color.rgb));
                }
                return color;
            }
        "#;
        let compile = |entry: PCSTR, target: PCSTR| -> windows::core::Result<ID3DBlob> {
            let mut code: Option<ID3DBlob> = None;
            let mut errors: Option<ID3DBlob> = None;
            // SAFETY: static source and entry/target strings; out pointers ours.
            let res = unsafe {
                D3DCompile(
                    SOURCE.as_ptr().cast(),
                    SOURCE.len(),
                    s!("present_composite"),
                    None,
                    None,
                    entry,
                    target,
                    0,
                    0,
                    &mut code,
                    Some(&mut errors),
                )
            };
            if let Err(e) = res {
                if let Some(errors) = errors {
                    // SAFETY: the blob's own bytes.
                    let text = unsafe {
                        std::slice::from_raw_parts(
                            errors.GetBufferPointer().cast::<u8>(),
                            errors.GetBufferSize(),
                        )
                    };
                    tracing::warn!("present compositor: {}", String::from_utf8_lossy(text));
                }
                return Err(e);
            }
            code.ok_or_else(windows::core::Error::empty)
        };
        let vs = compile(s!("vs"), s!("vs_5_1"))?;
        let ps = compile(s!("ps"), s!("ps_5_1"))?;
        // SAFETY: compiled shader blobs live through PSO creation.
        unsafe {
            let vs_bytes =
                std::slice::from_raw_parts(vs.GetBufferPointer().cast::<u8>(), vs.GetBufferSize());
            let ps_bytes =
                std::slice::from_raw_parts(ps.GetBufferPointer().cast::<u8>(), ps.GetBufferSize());
            let root: ID3D12RootSignature = device.CreateRootSignature(0, vs_bytes)?;
            let make_pso = |blend: bool| -> windows::core::Result<ID3D12PipelineState> {
                let mut rt_blend = D3D12_RENDER_TARGET_BLEND_DESC {
                    RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    ..Default::default()
                };
                if blend {
                    rt_blend.BlendEnable = true.into();
                    rt_blend.SrcBlend = D3D12_BLEND_ONE;
                    rt_blend.DestBlend = D3D12_BLEND_INV_SRC_ALPHA;
                    rt_blend.BlendOp = D3D12_BLEND_OP_ADD;
                    rt_blend.SrcBlendAlpha = D3D12_BLEND_ONE;
                    rt_blend.DestBlendAlpha = D3D12_BLEND_INV_SRC_ALPHA;
                    rt_blend.BlendOpAlpha = D3D12_BLEND_OP_ADD;
                }
                let mut blend_desc = D3D12_BLEND_DESC::default();
                blend_desc.RenderTarget[0] = rt_blend;
                let mut rtv_formats = [DXGI_FORMAT_UNKNOWN; 8];
                rtv_formats[0] = format;
                let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                    pRootSignature: std::mem::ManuallyDrop::new(Some(root.clone())),
                    VS: D3D12_SHADER_BYTECODE {
                        pShaderBytecode: vs_bytes.as_ptr().cast(),
                        BytecodeLength: vs_bytes.len(),
                    },
                    PS: D3D12_SHADER_BYTECODE {
                        pShaderBytecode: ps_bytes.as_ptr().cast(),
                        BytecodeLength: ps_bytes.len(),
                    },
                    BlendState: blend_desc,
                    SampleMask: u32::MAX,
                    RasterizerState: D3D12_RASTERIZER_DESC {
                        FillMode: D3D12_FILL_MODE_SOLID,
                        CullMode: D3D12_CULL_MODE_NONE,
                        DepthClipEnable: false.into(),
                        ..Default::default()
                    },
                    PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                    NumRenderTargets: 1,
                    RTVFormats: rtv_formats,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    ..Default::default()
                };
                let pso = device.CreateGraphicsPipelineState(&desc);
                let _ = std::mem::ManuallyDrop::into_inner(desc.pRootSignature);
                pso
            };
            let opaque = make_pso(false)?;
            let overlay = make_pso(true)?;
            let rtv_heap: ID3D12DescriptorHeap =
                device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                    NumDescriptors: 1,
                    Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
                    NodeMask: 0,
                })?;
            let srv_heap: ID3D12DescriptorHeap =
                device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                    NumDescriptors: MAX_PLANES,
                    Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                    NodeMask: 0,
                })?;
            let srv_stride =
                device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
            Ok(Compositor {
                format,
                root,
                opaque,
                overlay,
                rtv_heap,
                srv_heap,
                srv_stride,
            })
        }
    }
}

pub(super) type PresentSource<'a> = (&'a ID3D12Resource, u32, PresentGeometry);

pub(super) struct Plane<'a> {
    pub(super) texture: &'a ID3D12Resource,
    pub(super) size: (u32, u32),
    state: D3D12_RESOURCE_STATES,
    format: DXGI_FORMAT,
    geometry: PresentGeometry,
}

impl Plane<'_> {
    fn constants(&self) -> [u32; 9] {
        let (w, h) = self.size;
        let [left, top, right, bottom] = self.geometry.source.unwrap_or([0, 0, w, h]);
        let (w, h) = (w as f32, h as f32);
        let (l, t, r, b) = (left as f32, top as f32, right as f32, bottom as f32);
        let srgb = matches!(
            self.format,
            DXGI_FORMAT_R8G8B8A8_UNORM_SRGB | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
        );
        [
            (l / w).to_bits(),
            (t / h).to_bits(),
            (r / w).to_bits(),
            (b / h).to_bits(),
            ((l + 0.5) / w).to_bits(),
            ((t + 0.5) / h).to_bits(),
            ((r - 0.5) / w).to_bits(),
            ((b - 0.5) / h).to_bits(),
            u32::from(srgb),
        ]
    }

    fn placement(&self, (width, height): (u32, u32)) -> Option<(D3D12_VIEWPORT, RECT)> {
        let [l, t, r, b] = self.geometry.destination;
        let (w, h) = (width as f32, height as f32);
        let (l, t, r, b) = (l * w, t * h, r * w, b * h);
        // Scissors include pixels whose centers fall within the placed view.
        let edge = |v: f32| (v - 0.5).ceil() as i32;
        let scissor = RECT {
            left: edge(l),
            top: edge(t),
            right: edge(r),
            bottom: edge(b),
        };
        if scissor.left >= scissor.right || scissor.top >= scissor.bottom {
            return None;
        }
        Some((
            D3D12_VIEWPORT {
                TopLeftX: l,
                TopLeftY: t,
                Width: r - l,
                Height: b - t,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            },
            scissor,
        ))
    }
}

pub(super) fn prepare_planes<'a>(
    device: &ID3D12Device,
    planes: &[PresentSource<'a>],
) -> Option<Vec<Plane<'a>>> {
    let mut prepared: Vec<Plane<'_>> = Vec::with_capacity(planes.len());
    if planes.len() > MAX_PLANES as usize {
        return None;
    }
    for &(texture, state, geometry) in planes {
        // SAFETY: a live D3D12 resource and a local feature query.
        let desc = unsafe { texture.GetDesc() };
        let format = sampled_format(desc.Format)?;
        let mut support = D3D12_FEATURE_DATA_FORMAT_SUPPORT {
            Format: format,
            ..Default::default()
        };
        if desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D
            || desc.DepthOrArraySize != 1
            || desc.SampleDesc.Count != 1
            || (desc.Flags & D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE).0 != 0
            || desc.Width == 0
            || desc.Height == 0
            || prepared.iter().any(|previous| {
                previous.texture == texture
                    && (previous.geometry == geometry || previous.state.0 as u32 != state)
            })
        {
            return None;
        }
        unsafe {
            device.CheckFeatureSupport(
                D3D12_FEATURE_FORMAT_SUPPORT,
                (&raw mut support).cast(),
                size_of_val(&support) as u32,
            )
        }
        .ok()?;
        if (support.Support1 & D3D12_FORMAT_SUPPORT1_SHADER_SAMPLE).0 == 0 {
            return None;
        }
        prepared.push(Plane {
            texture,
            size: (u32::try_from(desc.Width).ok()?, desc.Height),
            state: D3D12_RESOURCE_STATES(state as i32),
            format,
            geometry,
        });
    }
    Some(prepared)
}

fn sampled_format(format: DXGI_FORMAT) -> Option<DXGI_FORMAT> {
    Some(match format {
        DXGI_FORMAT_R8G8B8A8_TYPELESS => DXGI_FORMAT_R8G8B8A8_UNORM,
        DXGI_FORMAT_B8G8R8A8_TYPELESS => DXGI_FORMAT_B8G8R8A8_UNORM,
        DXGI_FORMAT_R10G10B10A2_TYPELESS => DXGI_FORMAT_R10G10B10A2_UNORM,
        DXGI_FORMAT_R16G16B16A16_TYPELESS => DXGI_FORMAT_R16G16B16A16_FLOAT,
        DXGI_FORMAT_R8G8B8A8_UNORM
        | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
        | DXGI_FORMAT_B8G8R8A8_UNORM
        | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
        | DXGI_FORMAT_R10G10B10A2_UNORM
        | DXGI_FORMAT_R16G16B16A16_FLOAT
        | DXGI_FORMAT_R16G16B16A16_UNORM => format,
        _ => return None,
    })
}

impl Compositor {
    /// Draws the planes and restores their incoming states. The target
    /// starts in back_state and is left in RENDER_TARGET.
    pub(super) fn record(
        &self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        back: &ID3D12Resource,
        back_state: D3D12_RESOURCE_STATES,
        planes: &[Plane<'_>],
    ) {
        // SAFETY: open list, a matching render target, and prepared SRVs.
        // Presentation serializes and waits for all use of these heaps.
        unsafe {
            let desc = back.GetDesc();
            let (width, height) = (desc.Width as u32, desc.Height);
            let rtv = self.rtv_heap.GetCPUDescriptorHandleForHeapStart();
            device.CreateRenderTargetView(
                back,
                Some(&D3D12_RENDER_TARGET_VIEW_DESC {
                    Format: self.format,
                    ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_RTV {
                            MipSlice: 0,
                            PlaneSlice: 0,
                        },
                    },
                }),
                rtv,
            );
            let srv_cpu_base = self.srv_heap.GetCPUDescriptorHandleForHeapStart();
            let srv_gpu_base = self.srv_heap.GetGPUDescriptorHandleForHeapStart();
            let count = planes.len();
            let mut before = Vec::with_capacity(count + 1);
            for (i, plane) in planes.iter().enumerate() {
                let handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                    ptr: srv_cpu_base.ptr + i * self.srv_stride as usize,
                };
                device.CreateShaderResourceView(
                    plane.texture,
                    Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                        Format: plane.format,
                        ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                        Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                        Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                            Texture2D: D3D12_TEX2D_SRV {
                                MostDetailedMip: 0,
                                MipLevels: 1,
                                PlaneSlice: 0,
                                ResourceMinLODClamp: 0.0,
                            },
                        },
                    }),
                    handle,
                );
                if plane.state != D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE
                    && !planes[..i]
                        .iter()
                        .any(|earlier| earlier.texture == plane.texture)
                {
                    before.push(transition(
                        plane.texture,
                        plane.state,
                        D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                    ));
                }
            }
            before.push(transition(
                back,
                back_state,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            ));
            list.ResourceBarrier(&before);
            list.SetDescriptorHeaps(&[Some(self.srv_heap.clone())]);
            list.SetGraphicsRootSignature(&self.root);
            list.OMSetRenderTargets(1, Some(&rtv), false, None);
            if planes[0].geometry.destination != FULL_DESTINATION {
                list.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0], None);
            }
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            for (i, plane) in planes.iter().enumerate() {
                let Some((viewport, scissor)) = plane.placement((width, height)) else {
                    continue;
                };
                list.RSSetViewports(&[viewport]);
                list.RSSetScissorRects(&[scissor]);
                let constants = plane.constants();
                list.SetGraphicsRoot32BitConstants(
                    1,
                    constants.len() as u32,
                    constants.as_ptr().cast(),
                    0,
                );
                list.SetPipelineState(if i == 0 { &self.opaque } else { &self.overlay });
                list.SetGraphicsRootDescriptorTable(
                    0,
                    D3D12_GPU_DESCRIPTOR_HANDLE {
                        ptr: srv_gpu_base.ptr + (i as u64) * u64::from(self.srv_stride),
                    },
                );
                list.DrawInstanced(3, 1, 0, 0);
            }
            let mut after = Vec::with_capacity(count);
            for (i, plane) in planes.iter().enumerate() {
                if plane.state != D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE
                    && !planes[..i]
                        .iter()
                        .any(|earlier| earlier.texture == plane.texture)
                {
                    after.push(transition(
                        plane.texture,
                        D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                        plane.state,
                    ));
                }
            }
            if !after.is_empty() {
                list.ResourceBarrier(&after);
            }
        }
    }
}

#[cfg(all(test, feature = "diagnostics"))]
mod tests {
    use super::*;
    use windows::core::Interface;

    const FORMAT: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

    fn texture(
        device: &ID3D12Device,
        width: u32,
        height: u32,
        state: D3D12_RESOURCE_STATES,
        flags: D3D12_RESOURCE_FLAGS,
        format: DXGI_FORMAT,
    ) -> ID3D12Resource {
        let mut result = None;
        // SAFETY: output and descriptors live through this call.
        unsafe {
            device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_DEFAULT,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                        Width: u64::from(width),
                        Height: height,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        Format: format,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Flags: flags,
                        ..Default::default()
                    },
                    state,
                    None,
                    &mut result,
                )
                .expect("offscreen texture");
        }
        result.unwrap()
    }

    fn upload(
        host: &super::super::Host,
        list: &ID3D12GraphicsCommandList,
        texture: &ID3D12Resource,
        pixels: &[[u8; 4]],
    ) -> ID3D12Resource {
        let mut layout = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut size = 0;
        // SAFETY: live destination in COPY_DEST and mapped source retained by the caller.
        unsafe {
            host.device.GetCopyableFootprints(
                &texture.GetDesc(),
                0,
                1,
                0,
                Some(&mut layout),
                None,
                None,
                Some(&mut size),
            );
            let width = layout.Footprint.Width as usize;
            assert_eq!(pixels.len(), width * layout.Footprint.Height as usize);
            let pitch = layout.Footprint.RowPitch as usize;
            let (buffer, bytes) = host.upload_buffer(size).expect("upload buffer");
            for (row, pixels) in pixels.chunks_exact(width).enumerate() {
                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr().cast::<u8>(),
                    bytes.add(layout.Offset as usize + row * pitch),
                    width * 4,
                );
            }
            let source = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&buffer),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: layout,
                },
            };
            let dest = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(texture),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            };
            list.CopyTextureRegion(&dest, 0, 0, 0, &source, None);
            buffer
        }
    }

    fn pixels(
        (buffer, layout): (ID3D12Resource, D3D12_PLACED_SUBRESOURCE_FOOTPRINT),
    ) -> Vec<[u8; 4]> {
        // SAFETY: the list has completed and this buffer was allocated for the footprint.
        unsafe {
            let mut data = std::ptr::null_mut();
            buffer.Map(0, None, Some(&mut data)).expect("readback map");
            let mut result = Vec::new();
            for row in 0..layout.Footprint.Height as usize {
                let row = data
                    .cast::<u8>()
                    .add(layout.Offset as usize + row * layout.Footprint.RowPitch as usize);
                for x in 0..layout.Footprint.Width as usize {
                    result.push(row.add(x * 4).cast::<[u8; 4]>().read());
                }
            }
            buffer.Unmap(0, None);
            result
        }
    }

    #[test]
    fn warp_present_fallback_uses_the_primary_extent_when_copyable() {
        let host = super::super::create_device(true).expect("software WARP host");
        let texture_in = |width, height, format| {
            texture(
                &host.device,
                width,
                height,
                D3D12_RESOURCE_STATE_COMMON,
                D3D12_RESOURCE_FLAG_NONE,
                format,
            )
        };
        let base = texture_in(2, 3, FORMAT);
        let overlay = texture_in(4, 5, FORMAT);
        let integer_overlay = texture_in(6, 7, DXGI_FORMAT_R8G8B8A8_UINT);
        // SAFETY: live primary texture.
        let primary = unsafe { base.GetDesc() };
        let planes = [(&base, 0), (&overlay, 0)]
            .map(|(texture, state)| (texture, state, PresentGeometry::default()));
        let plan =
            |ready| super::super::plan_presentation(&host.device, primary, &planes, None, || ready);
        let (extent, composition) = plan(true).unwrap();
        assert_eq!(extent, (4, 5));
        assert!(composition.is_some());
        let (extent, composition) = plan(false).unwrap();
        assert_eq!(extent, (2, 3));
        assert!(composition.is_none());

        let invalid_planes = [(&base, 0), (&integer_overlay, 0)]
            .map(|(texture, state)| (texture, state, PresentGeometry::default()));
        let (extent, composition) =
            super::super::plan_presentation(&host.device, primary, &invalid_planes, None, || {
                panic!("unsupported overlay should bypass compositor initialization")
            })
            .unwrap();
        assert_eq!(extent, (2, 3));
        assert!(composition.is_none());

        let cap = std::num::NonZeroU32::new(2);
        let (extent, composition) =
            super::super::plan_presentation(&host.device, primary, &invalid_planes, cap, || true)
                .unwrap();
        assert_eq!(extent, (1, 2));
        assert_eq!(composition.unwrap().len(), 1);

        let unsampleable = [(&integer_overlay, 0, PresentGeometry::default())];
        // SAFETY: live integer source texture.
        let integer_desc = unsafe { integer_overlay.GetDesc() };
        let (extent, composition) =
            super::super::plan_presentation(&host.device, integer_desc, &unsampleable, cap, || {
                panic!("unsampleable plane should bypass compositor initialization")
            })
            .unwrap();
        assert_eq!(extent, (6, 7));
        assert!(composition.is_none());

        let unsafe_copy = D3D12_RESOURCE_DESC {
            MipLevels: 2,
            ..primary
        };
        assert!(
            super::super::plan_presentation(&host.device, unsafe_copy, &planes, None, || false,)
                .is_none()
        );
    }

    #[test]
    fn warp_srgb_primary_matches_a_raw_copy() {
        let host = super::super::create_device(true).expect("software WARP host");
        let compositor = Compositor::new(&host.device, FORMAT).expect("compositor");
        let make_texture = |format, flags, state| texture(&host.device, 2, 2, state, flags, format);
        let primary = make_texture(
            DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
            D3D12_RESOURCE_FLAG_NONE,
            D3D12_RESOURCE_STATE_COPY_DEST,
        );
        let overlay = make_texture(
            FORMAT,
            D3D12_RESOURCE_FLAG_NONE,
            D3D12_RESOURCE_STATE_COPY_DEST,
        );
        let raw = make_texture(
            FORMAT,
            D3D12_RESOURCE_FLAG_NONE,
            D3D12_RESOURCE_STATE_COPY_DEST,
        );
        let back = make_texture(
            FORMAT,
            D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COMMON,
        );
        let primary_pixels = [
            [128, 128, 128, 255],
            [1, 10, 11, 255],
            [32, 77, 111, 255],
            [250, 191, 199, 255],
        ];
        let run = host.new_list().expect("offscreen list");
        let list = &run.list;
        let _base_upload = upload(&host, list, &primary, &primary_pixels);
        let _overlay_upload = upload(&host, list, &overlay, &[[0; 4]; 4]);
        // SAFETY: live 2x2 textures in COPY_DEST; RGBA8 formats share a type group.
        unsafe {
            list.ResourceBarrier(&[transition(
                &primary,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            )]);
            list.CopyResource(&raw, &primary);
            list.ResourceBarrier(&[transition(
                &raw,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            )]);
        }
        let planes = [
            (&primary, D3D12_RESOURCE_STATE_COPY_SOURCE.0 as u32),
            (&overlay, D3D12_RESOURCE_STATE_COPY_DEST.0 as u32),
        ]
        .map(|(texture, state)| (texture, state, PresentGeometry::default()));
        let prepared = prepare_planes(&host.device, &planes).unwrap();
        compositor.record(
            &host.device,
            list,
            &back,
            D3D12_RESOURCE_STATE_COMMON,
            &prepared,
        );
        // SAFETY: composition left the target in RENDER_TARGET on this list.
        unsafe {
            list.ResourceBarrier(&[transition(
                &back,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            )]);
        }
        let copy = crate::capture::read_back(&host.device, list, &raw).unwrap();
        let result = crate::capture::read_back(&host.device, list, &back).unwrap();
        // SAFETY: submit the live offscreen list on the WARP queue. Use the
        // normal fence without registering an event to exercise the fallback.
        unsafe {
            list.Close().expect("offscreen list closes");
            let command_list = list.cast().expect("command list interface");
            host.queue.ExecuteCommandLists(&[Some(command_list)]);
            host.queue.Signal(&host.fence, 1).expect("WARP signal");
        }
        host.poll_submission(1);
        assert_eq!(host.log_messages(), None, "D3D12 debug errors");
        assert_eq!(pixels(copy), primary_pixels);
        assert_eq!(pixels(result), primary_pixels);
    }

    #[test]
    fn warp_composites_scaled_planes_and_restores_sources() {
        let host = super::super::create_device(true).expect("software WARP host");
        let compositor = Compositor::new(&host.device, FORMAT).expect("compositor");
        let base = texture(
            &host.device,
            2,
            2,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_FLAG_NONE,
            FORMAT,
        );
        let overlay = texture(
            &host.device,
            4,
            4,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_FLAG_NONE,
            FORMAT,
        );
        let back = texture(
            &host.device,
            4,
            4,
            D3D12_RESOURCE_STATE_COMMON,
            D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
            FORMAT,
        );
        let base_pixels = [
            [200, 0, 0, 255],
            [0, 200, 0, 255],
            [0, 0, 200, 255],
            [200, 200, 200, 255],
        ];
        let mut overlay_pixels = [[0u8; 4]; 16];
        overlay_pixels[3] = [64, 0, 0, 128];
        overlay_pixels[12] = [240, 190, 0, 255];
        let planes = [
            (&base, D3D12_RESOURCE_STATE_COPY_SOURCE.0 as u32),
            (&overlay, D3D12_RESOURCE_STATE_COPY_DEST.0 as u32),
        ]
        .map(|(texture, state)| (texture, state, PresentGeometry::default()));
        let prepared = prepare_planes(&host.device, &planes).expect("sampleable planes");
        assert!(prepare_planes(&host.device, &[planes[0], planes[0]]).is_none());
        assert_eq!(
            sampled_format(DXGI_FORMAT_R8G8B8A8_UNORM_SRGB),
            Some(DXGI_FORMAT_R8G8B8A8_UNORM_SRGB)
        );
        assert_eq!(sampled_format(DXGI_FORMAT_R8G8B8A8_UINT), None);

        let run = host.new_list().expect("offscreen list");
        let list = &run.list;
        let _base_upload = upload(&host, list, &base, &base_pixels);
        let _overlay_upload = upload(&host, list, &overlay, &overlay_pixels);
        // SAFETY: resources are live, with the indicated states on the open list.
        unsafe {
            list.ResourceBarrier(&[transition(
                &base,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            )]);
        }
        compositor.record(
            &host.device,
            list,
            &back,
            D3D12_RESOURCE_STATE_COMMON,
            &prepared,
        );
        // Read the untouched base directly in its restored COPY_SOURCE state.
        let base_read = crate::capture::read_back(&host.device, list, &base).unwrap();
        // SAFETY: the compositor restored the source states on this list.
        unsafe {
            list.ResourceBarrier(&[
                transition(
                    &back,
                    D3D12_RESOURCE_STATE_RENDER_TARGET,
                    D3D12_RESOURCE_STATE_COPY_SOURCE,
                ),
                transition(
                    &overlay,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    D3D12_RESOURCE_STATE_COPY_SOURCE,
                ),
            ]);
        }
        let overlay_read = crate::capture::read_back(&host.device, list, &overlay).unwrap();
        let result = crate::capture::read_back(&host.device, list, &back).unwrap();
        // SAFETY: the WARP queue owns the closed list until its fence completes.
        unsafe {
            use windows::Win32::Foundation::WAIT_OBJECT_0;
            use windows::Win32::System::Threading::WaitForSingleObject;

            list.Close().expect("compositor list closes");
            let command_list = list.cast().expect("command list interface");
            host.queue.ExecuteCommandLists(&[Some(command_list)]);
            host.queue.Signal(&host.fence, 1).expect("WARP signal");
            host.fence
                .SetEventOnCompletion(1, host.event)
                .expect("WARP fence event");
            assert_eq!(WaitForSingleObject(host.event, 5000), WAIT_OBJECT_0);
            host.device
                .GetDeviceRemovedReason()
                .expect("WARP device remains available");
        }
        assert_eq!(host.log_messages(), None, "D3D12 debug errors");
        if host.messages.is_some() {
            eprintln!("D3D12 debug layer reported no errors");
        }
        assert_eq!(pixels(base_read), base_pixels);
        assert_eq!(pixels(overlay_read), overlay_pixels);
        let image = pixels(result);
        for (x, y, expected) in [
            (0, 0, [200, 0, 0, 255]),
            (1, 0, [150, 50, 0, 255]),
            (3, 0, [64, 100, 0, 255]),
            (2, 2, [125, 150, 150, 255]),
            (0, 3, [240, 190, 0, 255]),
            (3, 3, [200, 200, 200, 255]),
        ] {
            let actual = image[y * 4 + x];
            assert!(
                actual
                    .iter()
                    .zip(expected)
                    .all(|(&a, e)| a.abs_diff(e) <= 1),
                "pixel ({x}, {y}): {actual:?}, expected {expected:?}"
            );
        }
        assert!(image.iter().all(|pixel| pixel[3] == 255));
        eprintln!("software WARP 4x4 composite: {image:?}");
    }

    #[test]
    fn warp_composites_1080p_and_4k_cropped_placed_planes() {
        let host = super::super::create_device(true).expect("software WARP host");
        let comp = Compositor::new(&host.device, FORMAT).expect("compositor");
        let target = |w, h, state| {
            texture(
                &host.device,
                w,
                h,
                state,
                D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
                FORMAT,
            )
        };
        let rt = D3D12_RESOURCE_STATE_RENDER_TARGET;
        let base = target(1920, 1080, rt);
        let overlay = target(3840, 2160, rt);
        let run = host.new_list().unwrap();
        let list = &run.list;
        let crop = RECT {
            left: 1024,
            top: 768,
            right: 1040,
            bottom: 784,
        };
        // SAFETY: the source textures and the open list are live.
        unsafe {
            list.ClearRenderTargetView(host.rtv(Some(&base), None), &[0.0, 0.5, 0.0, 1.0], None);
            let overlay_rtv = host.rtv(Some(&overlay), None);
            list.ClearRenderTargetView(overlay_rtv, &[1.0, 0.0, 1.0, 1.0], None);
            list.ClearRenderTargetView(overlay_rtv, &[0.0, 0.0, 0.5, 0.5], Some(&[crop]));
            list.ClearRenderTargetView(
                overlay_rtv,
                &[0.5, 0.0, 0.0, 0.5],
                Some(&[RECT {
                    left: 2048,
                    top: 1024,
                    right: 2050,
                    bottom: 1026,
                }]),
            );
        }
        let input = [
            (
                &base,
                rt.0 as u32,
                PresentGeometry {
                    source: None,
                    destination: [0.0, 0.125, 1.0, 1.0],
                },
            ),
            (
                &overlay,
                rt.0 as u32,
                PresentGeometry {
                    source: Some([1024, 768, 1040, 784]),
                    destination: [0.25, 0.25, 0.75, 0.75],
                },
            ),
            (
                &overlay,
                rt.0 as u32,
                PresentGeometry {
                    source: Some([2048, 1024, 2050, 1026]),
                    destination: [0.5, 0.5, 0.625, 0.625],
                },
            ),
        ];
        let cases = [
            (None, (3840, 2160)),
            (Some(1080), (1920, 1080)),
            (Some(720), (1280, 720)),
        ];
        let mut captures = Vec::new();
        for (cap, expected) in cases {
            // SAFETY: the primary source is live.
            let ((width, height), planes) = super::super::plan_presentation(
                &host.device,
                unsafe { base.GetDesc() },
                &input,
                cap.and_then(std::num::NonZeroU32::new),
                || true,
            )
            .expect("presentation plan");
            assert_eq!((width, height), expected);
            let back = target(width, height, D3D12_RESOURCE_STATE_COMMON);
            comp.record(
                &host.device,
                list,
                &back,
                D3D12_RESOURCE_STATE_COMMON,
                &planes.unwrap(),
            );
            // SAFETY: the compositor leaves the destination in render-target state.
            unsafe {
                list.ResourceBarrier(&[transition(&back, rt, D3D12_RESOURCE_STATE_COPY_SOURCE)])
            };
            let (readback, layout) = crate::capture::read_back(&host.device, list, &back).unwrap();
            assert_eq!((layout.Footprint.Width, layout.Footprint.Height), expected);
            captures.push((back, readback, layout, width, height));
            // The compositor reuses shader-visible slots, so submit before
            // recording another composite that could change these descriptors.
            run.close();
            assert_eq!(host.execute_checked(&[&run]), None);
            run.reset();
        }
        let single = [(&base, rt.0 as u32, PresentGeometry::default())];
        // SAFETY: the primary source is live.
        let ((width, height), single) = super::super::plan_presentation(
            &host.device,
            unsafe { base.GetDesc() },
            &single,
            std::num::NonZeroU32::new(720),
            || true,
        )
        .expect("single-plane downscale plan");
        assert_eq!((width, height), (1280, 720));
        let single_back = target(width, height, D3D12_RESOURCE_STATE_COMMON);
        comp.record(
            &host.device,
            list,
            &single_back,
            D3D12_RESOURCE_STATE_COMMON,
            &single.unwrap(),
        );
        // SAFETY: render-target transition for the final readback.
        unsafe {
            list.ResourceBarrier(&[transition(
                &single_back,
                rt,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            )])
        };
        let (single_readback, single_layout) =
            crate::capture::read_back(&host.device, list, &single_back).unwrap();
        assert_eq!(
            (
                single_layout.Footprint.Width,
                single_layout.Footprint.Height
            ),
            (1280, 720)
        );
        run.close();
        assert_eq!(host.execute_checked(&[&run]), None);
        let black = [0u8, 0, 0, 255];
        let green = [0, 128, 0, 255];
        let blue_over_green = [0, 64, 128, 255];
        let red_over_blue = [128, 32, 64, 255];
        // SAFETY: read back the first and last pixels of the full single plane.
        unsafe {
            let mut data = std::ptr::null_mut();
            single_readback.Map(0, None, Some(&mut data)).unwrap();
            for (x, y) in [(0, 0), (width - 1, height - 1)] {
                let at = single_layout.Offset as usize
                    + (y * single_layout.Footprint.RowPitch + x * 4) as usize;
                assert_eq!(data.cast::<u8>().add(at).cast::<[u8; 4]>().read(), green);
            }
            single_readback.Unmap(0, None);
        }
        for (_, readback, layout, w, h) in captures {
            let checks = [
                (0, h / 8 - 1, black),
                (0, h / 8, green),
                (w / 4 - 1, h / 4, green),
                (w / 4, h / 4 - 1, green),
                (w / 4, h / 4, blue_over_green),
                (w * 3 / 4 - 1, h * 3 / 4 - 1, blue_over_green),
                (w * 3 / 4, h * 3 / 4 - 1, green),
                (w * 3 / 4 - 1, h * 3 / 4, green),
                (w / 2 - 1, h / 2, blue_over_green),
                (w / 2, h / 2, red_over_blue),
                (w * 5 / 8 - 1, h * 5 / 8 - 1, red_over_blue),
                (w * 5 / 8, h * 5 / 8 - 1, blue_over_green),
                (w * 5 / 8 - 1, h * 5 / 8, blue_over_green),
            ];
            // SAFETY: submission completed and every sample is inside the mapped footprint.
            unsafe {
                let mut data = std::ptr::null_mut();
                readback.Map(0, None, Some(&mut data)).unwrap();
                for (x, y, expected) in checks {
                    let at =
                        layout.Offset as usize + (y * layout.Footprint.RowPitch + x * 4) as usize;
                    let actual = data.cast::<u8>().add(at).cast::<[u8; 4]>().read();
                    assert!(
                        actual
                            .into_iter()
                            .zip(expected)
                            .all(|(a, e)| a.abs_diff(e) <= 1),
                        "pixel ({x}, {y}) was {actual:?}, expected {expected:?}"
                    );
                }
                readback.Unmap(0, None);
            }
        }
    }
}
