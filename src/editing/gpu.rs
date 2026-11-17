use crate::editing::ops::Operation;
use anyhow::{Context, Result, anyhow};
use image::{DynamicImage, ImageBuffer, Rgba};
use std::sync::mpsc;
use wgpu::util::DeviceExt;

const SHADER: &str = r#"
struct Params {
    count: u32,
    width: u32,
    height: u32,
    operation: u32,
    amount: f32,
    groups_x: u32,
    padding1: f32,
    padding2: f32,
};

@group(0) @binding(0) var<storage, read> source: array<u32>;
@group(0) @binding(1) var<storage, read_write> destination: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn rounded_channel(value: f32) -> u32 {
    return u32(clamp(round(value), 0.0, 255.0));
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.y * params.groups_x * 256u + id.x;
    if (index >= params.count) {
        return;
    }

    let pixel = source[index];
    let r = f32(pixel & 255u);
    let g = f32((pixel >> 8u) & 255u);
    let b = f32((pixel >> 16u) & 255u);
    let a = pixel & 0xff000000u;
    var nr = r;
    var ng = g;
    var nb = b;

    switch params.operation {
        case 0u: {
            let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            nr = luma;
            ng = luma;
            nb = luma;
        }
        case 1u: {
            nr = 255.0 - r;
            ng = 255.0 - g;
            nb = 255.0 - b;
        }
        case 2u: {
            nr = r + params.amount;
            ng = g + params.amount;
            nb = b + params.amount;
        }
        case 3u: {
            let factor = (259.0 * (params.amount + 255.0)) / (255.0 * (259.0 - params.amount));
            nr = factor * (r - 128.0) + 128.0;
            ng = factor * (g - 128.0) + 128.0;
            nb = factor * (b - 128.0) + 128.0;
        }
        case 4u: {
            let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            nr = luma + (r - luma) * params.amount;
            ng = luma + (g - luma) * params.amount;
            nb = luma + (b - luma) * params.amount;
        }
        case 5u: {
            let inv_gamma = 1.0 / params.amount;
            nr = pow(r / 255.0, inv_gamma) * 255.0;
            ng = pow(g / 255.0, inv_gamma) * 255.0;
            nb = pow(b / 255.0, inv_gamma) * 255.0;
        }
        case 6u: {
            let multiplier = exp2(params.amount);
            nr = r * multiplier;
            ng = g * multiplier;
            nb = b * multiplier;
        }
        case 7u: {
            let x = f32(index % params.width);
            let y = f32(index / params.width);
            let center = vec2<f32>(
                (f32(params.width) - 1.0) * 0.5,
                (f32(params.height) - 1.0) * 0.5
            );
            let max_distance = max(length(vec2<f32>(f32(params.width), f32(params.height))) * 0.5, 1.0);
            let distance = length(vec2<f32>(x, y) - center) / max_distance;
            let edge = smoothstep(0.3, 1.0, distance);
            let factor = 1.0 - params.amount * edge;
            nr = r * factor;
            ng = g * factor;
            nb = b * factor;
        }
        default: {}
    }

    destination[index] =
        rounded_channel(nr)
        | (rounded_channel(ng) << 8u)
        | (rounded_channel(nb) << 16u)
        | a;
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    count: u32,
    width: u32,
    height: u32,
    operation: u32,
    amount: f32,
    groups_x: u32,
    padding1: f32,
    padding2: f32,
}

pub struct GpuProcessor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    adapter_name: String,
}

impl GpuProcessor {
    pub fn new() -> Result<Self> {
        pollster::block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .ok_or_else(|| anyhow!("no compatible GPU adapter was found"))?;
            let adapter_name = adapter.get_info().name;
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("forma GPU device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::downlevel_defaults(),
                        memory_hints: wgpu::MemoryHints::Performance,
                    },
                    None,
                )
                .await
                .context("failed to initialize GPU device")?;
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("forma pixel operations"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("forma pixel operations"),
                layout: None,
                module: &shader,
                entry_point: "main",
                compilation_options: Default::default(),
                cache: None,
            });
            Ok(Self {
                device,
                queue,
                pipeline,
                adapter_name,
            })
        })
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    pub fn supports(operation: &Operation) -> bool {
        matches!(
            operation,
            Operation::Grayscale
                | Operation::Invert
                | Operation::Brighten(_)
                | Operation::Contrast(_)
                | Operation::Saturate(_)
                | Operation::Gamma(_)
                | Operation::Exposure(_)
                | Operation::Vignette(_)
        )
    }

    fn parameters(operation: &Operation) -> Result<(u32, f32)> {
        match operation {
            Operation::Grayscale => Ok((0, 0.0)),
            Operation::Invert => Ok((1, 0.0)),
            Operation::Brighten(amount) => Ok((2, *amount as f32)),
            Operation::Contrast(amount) => Ok((3, *amount)),
            Operation::Saturate(amount) => Ok((4, *amount)),
            Operation::Gamma(amount) => Ok((5, *amount)),
            Operation::Exposure(amount) => Ok((6, *amount)),
            Operation::Vignette(amount) => Ok((7, *amount)),
            _ => Err(anyhow!("operation is not supported by the GPU path")),
        }
    }

    pub fn apply(&self, image: &DynamicImage, operation: &Operation) -> Result<DynamicImage> {
        let rgba = image.to_rgba8();
        let width = rgba.width();
        let height = rgba.height();
        let count = width
            .checked_mul(height)
            .ok_or_else(|| anyhow!("image dimensions overflow"))?;
        if count == 0 {
            return Err(anyhow!("cannot GPU-process an empty image"));
        }
        let byte_len = u64::from(count) * 4;
        if byte_len > u64::from(self.device.limits().max_storage_buffer_binding_size) {
            return Err(anyhow!(
                "image exceeds this GPU's storage-buffer limit; use a smaller image or omit --gpu"
            ));
        }
        let (operation_code, amount) = Self::parameters(operation)?;
        let max_groups = self.device.limits().max_compute_workgroups_per_dimension;
        let groups_x = count.div_ceil(256).min(max_groups);
        let groups_y = count.div_ceil(groups_x * 256);
        let params = Params {
            count,
            width,
            height,
            operation: operation_code,
            amount,
            groups_x,
            padding1: 0.0,
            padding2: 0.0,
        };
        let source = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("forma GPU input"),
                contents: rgba.as_raw(),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let destination = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("forma GPU output"),
            size: byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("forma GPU parameters"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("forma GPU image bindings"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: source.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: destination.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("forma GPU readback"),
            size: byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("forma GPU image command"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("forma GPU image operation"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(groups_x, groups_y, 1);
        }
        encoder.copy_buffer_to_buffer(&destination, 0, &readback, 0, byte_len);
        self.queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .context("GPU readback callback did not complete")?
            .context("failed to map GPU output")?;
        let bytes = slice.get_mapped_range().to_vec();
        readback.unmap();
        let output = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, bytes)
            .ok_or_else(|| anyhow!("GPU returned an invalid image buffer"))?;
        Ok(DynamicImage::ImageRgba8(output))
    }
}
