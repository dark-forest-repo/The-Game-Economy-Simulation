//! GPU 计算模块 — wgpu 基础设施 + 群众队列处理
//!
//! Radeon 780M (RDNA3) via Vulkan. wgpu 抽象了后端差异。

#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};

/// GPU 队列数据 (80 字节对齐)
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct CohortGpu {
    pub count: u32,
    pub avg_collector_lv: u32,
    pub avg_weapon_lv: u32,
    pub avg_shield_lv: u32,
    pub avg_radar_lv: u32,
    pub avg_engine_lv: u32,
    pub _pad: [u32; 2],      // 对齐
    pub total_energy_lo: u64, // 低 64 位 (近似)
    pub total_energy_hi: u64, // 高 64 位
    pub total_dft_lo: u64,
    pub total_dft_hi: u64,
    pub deaths: u32,
    pub upgrades: u32,
    pub _pad2: [u32; 2],
}

/// GPU 上下文
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuContext {
    pub async fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN, // AMD GPU → Vulkan
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No GPU adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .map_err(|e| format!("Device: {}", e))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cohort"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cohort_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cohort_pipeline"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("cohort_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        Ok(Self { device, queue, pipeline, bind_group_layout })
    }

    /// 运行队列计算
    pub fn run_cohort(&self, cohorts: &mut [CohortGpu]) {
        let n = cohorts.len().max(1) as u64;
        let size = std::mem::size_of::<CohortGpu>() as u64;

        // 创建 GPU buffer
        let input_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cohort_input"),
            size: n * size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        // 写数据
        {
            let mut view = input_buf.slice(..).get_mapped_range_mut();
            let bytes = bytemuck::cast_slice_mut::<u8, CohortGpu>(bytemuck::cast_slice_mut(&mut view));
            bytes.copy_from_slice(cohorts);
        }
        input_buf.unmap();

        let output_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cohort_output"),
            size: n * size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cohort_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: input_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: output_buf.as_entire_binding() },
            ],
        });

        // Dispatch
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(n as u32, 1, 1);
        }
        self.queue.submit(Some(encoder.finish()));

        // 读回结果
        {
            let slice = output_buf.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            self.device.poll(wgpu::Maintain::Wait);
            let view = slice.get_mapped_range();
            let result: &[CohortGpu] = bytemuck::cast_slice(&view);
            cohorts.copy_from_slice(result);
        }
        output_buf.unmap();
    }
}

const SHADER_SRC: &str = r#"
struct Cohort {
    count: u32,
    avg_collector_lv: u32,
    avg_weapon_lv: u32,
    avg_shield_lv: u32,
    avg_radar_lv: u32,
    avg_engine_lv: u32,
    _pad: vec2<u32>,
    total_energy_lo: u64,
    total_energy_hi: u64,
    total_dft_lo: u64,
    total_dft_hi: u64,
    deaths: u32,
    upgrades: u32,
    _pad2: vec2<u32>,
}

@group(0) @binding(0) var<storage, read> input: array<Cohort>;
@group(0) @binding(1) var<storage, read_write> output: array<Cohort>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let c = input[i];
    if c.count == 0u32 { return; }

    var out = c;

    // 简单采集: 每队列每人每天 +2000 能量 (近似)
    let collected = u64(c.count) * 2000u64;
    out.total_energy_lo += collected;

    // 简单升级: 能量 > 50000 就升级
    if out.total_energy_lo > 50000u64 && c.avg_collector_lv < 100u32 {
        out.avg_collector_lv += 1u32;
        out.upgrades += 1u32;
        out.total_energy_lo -= 50000u64;
    }

    output[i] = out;
}
"#;
