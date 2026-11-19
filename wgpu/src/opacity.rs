//! Isolated opacity surfaces share the renderer's device and queue.
use super::{Engine, layer, primitive};
use crate::core::{Rectangle, Size, renderer};
use crate::graphics::Viewport;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct State {
    pub parents: Vec<(layer::Stack, Option<renderer::Scale>, Rectangle, f32)>,
    pub surfaces: Vec<Surface>,
    pub next: usize,
    compositor: Option<Arc<Compositor>>,
}

#[derive(Debug)]
struct Compositor {
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl Compositor {
    fn new(engine: &Engine) -> Self {
        let device = &engine.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iced opacity"),
            source: wgpu::ShaderSource::Wgsl(include_str!("opacity.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("iced isolated opacity"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: engine.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Surface {
    size: Size<u32>,
    pub view: wgpu::TextureView,
    alpha: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    compositor: Arc<Compositor>,
}

impl State {
    pub(super) fn surface(&mut self, engine: &Engine, size: Size<u32>, opacity: f32) -> Surface {
        let compositor = self
            .compositor
            .get_or_insert_with(|| Arc::new(Compositor::new(engine)));
        if self.surfaces.get(self.next).is_none_or(|s| s.size != size) {
            let device = &engine.device;
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("iced opacity group"),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: engine.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let alpha = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("iced group opacity"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("iced opacity bindings"),
                layout: &compositor.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&compositor.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: alpha.as_entire_binding(),
                    },
                ],
            });
            let surface = Surface {
                size,
                view,
                alpha,
                bindings,
                compositor: Arc::clone(compositor),
            };
            if let Some(slot) = self.surfaces.get_mut(self.next) {
                *slot = surface;
            } else {
                self.surfaces.push(surface);
            }
        }
        let surface = self.surfaces[self.next].clone();
        self.next += 1;
        engine
            .queue
            .write_buffer(&surface.alpha, 0, bytemuck::bytes_of(&opacity));
        surface
    }
}

#[derive(Debug)]
pub(super) struct Pipeline;
impl primitive::Pipeline for Pipeline {
    fn new(_: &wgpu::Device, _: &wgpu::Queue, _: wgpu::TextureFormat) -> Self {
        Self
    }
}
impl primitive::Primitive for Surface {
    type Pipeline = Pipeline;
    fn prepare(
        &self,
        _: &mut Pipeline,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        _: &Rectangle,
        _: &Viewport,
    ) {
    }
    fn draw(&self, _: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        pass.set_pipeline(&self.compositor.pipeline);
        pass.set_bind_group(0, &self.bindings, &[]);
        pass.draw(0..3, 0..1);
        true
    }
}
