use crate::Buffer;
use crate::graphics::gradient;
use crate::quad::{self, Quad};

use bytemuck::{Pod, Zeroable};
use std::ops::Range;

/// CPU-side gradient quad, retaining the source stops until the layer is prepared.
#[derive(Clone, Debug)]
pub struct Gradient {
    pub instance: Instance,
    pub stops: Option<crate::core::gradient::Stops>,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Instance {
    pub gradient: gradient::Packed,
    pub quad: Quad,
}

#[derive(Debug)]
pub struct Layer {
    instances: Buffer<Instance>,
    runs: Vec<(Range<usize>, std::sync::Arc<crate::gradient::Pages>)>,
}

impl Layer {
    pub fn clear_tables(&mut self) {
        self.runs.clear();
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let instances = Buffer::new(
            device,
            "iced_wgpu.quad.gradient.buffer",
            quad::INITIAL_INSTANCES,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );

        Self {
            instances,
            runs: Vec::new(),
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        instances: &[Gradient],
        tables: &crate::gradient::Tables,
    ) {
        let _ = self.instances.resize(device, instances.len());
        let mut vertices = Vec::with_capacity(instances.len());
        let mut runs: Vec<(Range<usize>, std::sync::Arc<crate::gradient::Pages>)> = Vec::new();
        for (index, gradient) in instances.iter().enumerate() {
            vertices.push(gradient.instance);
            let pages = tables.get(gradient.stops.as_ref());
            if let Some((range, last)) = runs.last_mut()
                && std::sync::Arc::ptr_eq(last, &pages)
                && pages.groups.len() == 1
            {
                range.end += 1;
            } else {
                runs.push((index..index + 1, pages));
            }
        }
        let _ = self.instances.write(encoder, belt, 0, &vertices);
        self.runs = runs;
    }
}

#[derive(Debug, Clone)]
pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    pub tables: crate::gradient::Tables,
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        constants_layout: &wgpu::BindGroupLayout,
        tables: crate::gradient::Tables,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("iced_wgpu.quad.gradient.pipeline"),
            bind_group_layouts: &[Some(constants_layout), Some(tables.layout())],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iced_wgpu.quad.gradient.shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(concat!(
                include_str!("../shader/quad/snap.wgsl"),
                "\n",
                include_str!("../shader/quad.wgsl"),
                "\n",
                include_str!("../shader/vertex.wgsl"),
                "\n",
                include_str!("../shader/quad/shadow.wgsl"),
                "\n",
                include_str!("../shader/quad/gradient.wgsl"),
                "\n",
                include_str!("../shader/color.wgsl"),
                "\n",
                include_str!("../shader/gradient.wgsl")
            ))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("iced_wgpu.quad.gradient.pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("gradient_vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array!(
                        0 => Float32x4, // first two straight linear colors
                        1 => Float32x4,
                        2 => Float32x2, // offsets
                        3 => Float32x4, // gradient direction
                        4 => Uint32x3,  // kind, base, count
                        5 => Float32x4, // position and size
                        6 => Float32x4, // border color
                        7 => Float32x4, // border radius
                        8 => Float32,   // border width
                        9 => Float32x4, // shadow color
                        10 => Float32x2, // shadow offset
                        11 => Float32,  // shadow blur
                        12 => Uint32,   // snap
                    ),
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("gradient_fs_main"),
                targets: &quad::color_target_state(format),
                compilation_options: crate::gradient::fragment_options(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline, tables }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        constants: &'a wgpu::BindGroup,
        layer: &'a Layer,
        range: Range<usize>,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, constants, &[]);
        render_pass.set_vertex_buffer(0, layer.instances.slice(..));

        let first = layer
            .runs
            .partition_point(|(run, _)| run.end <= range.start);
        for (run, pages) in &layer.runs[first..] {
            if run.start >= range.end {
                break;
            }
            let start = range.start.max(run.start) as u32;
            let end = range.end.min(run.end) as u32;
            for group in &pages.groups {
                render_pass.set_bind_group(1, group, &[]);
                render_pass.draw(0..6, start..end);
            }
        }
    }
}
