//! Isolated opacity surfaces share the renderer's device and queue.
use super::{Engine, layer, primitive};
use crate::core::{Rectangle, Size, renderer};
use crate::graphics::Viewport;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct State {
    pub parents: Vec<(
        layer::Stack,
        Option<renderer::Scale>,
        Rectangle,
        f32,
        renderer::BlendMode,
    )>,
    pub surfaces: Vec<Surface>,
    pub next: usize,
    compositor: Option<Arc<Compositor>>,
}

#[derive(Debug)]
struct Compositor {
    pipeline: wgpu::RenderPipeline,
    multiply_backdrop: wgpu::RenderPipeline,
    multiply_source: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl Compositor {
    fn new(engine: &Engine) -> Self {
        let device = &engine.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iced opacity"),
            source: wgpu::ShaderSource::Wgsl(include_str!("opacity.wgsl").into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            ty,
            visibility: wgpu::ShaderStages::FRAGMENT,
            count: None,
        };
        let bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iced isolated layer inputs"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    1,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
                entry(
                    2,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("iced isolated layer composition"),
            bind_group_layouts: &[Some(&bindings)],
            immediate_size: 0,
        });
        let pipeline = |blend, write_mask| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("iced isolated opacity"),
                layout: Some(&layout),
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
                        blend: Some(blend),
                        write_mask,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let multiply = |src_factor, dst_factor| wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor,
                dst_factor,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        use wgpu::BlendFactor::{Dst, One, OneMinusDstAlpha, OneMinusSrcAlpha};
        Self {
            pipeline: pipeline(
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
                wgpu::ColorWrites::ALL,
            ),
            multiply_backdrop: pipeline(multiply(Dst, OneMinusSrcAlpha), wgpu::ColorWrites::COLOR),
            multiply_source: pipeline(multiply(OneMinusDstAlpha, One), wgpu::ColorWrites::ALL),
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
    blend: renderer::BlendMode,
    pub view: wgpu::TextureView,
    alpha: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    compositor: Arc<Compositor>,
}

impl State {
    pub(super) fn surface(
        &mut self,
        engine: &Engine,
        size: Size<u32>,
        opacity: f32,
        blend: renderer::BlendMode,
    ) -> Surface {
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
                blend,
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
        let mut surface = self.surfaces[self.next].clone();
        surface.blend = blend;
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
        match self.blend {
            renderer::BlendMode::Normal => {
                pass.set_pipeline(&self.compositor.pipeline);
                pass.set_bind_group(0, &self.bindings, &[]);
                pass.draw(0..3, 0..1);
            }
            renderer::BlendMode::Multiply => {
                // Premultiplied multiply is Cs*Cb + Cb*(1-As) + Cs*(1-Ab).
                // Preserve destination alpha through the first two RGB terms.
                pass.set_pipeline(&self.compositor.multiply_backdrop);
                pass.set_bind_group(0, &self.bindings, &[]);
                pass.draw(0..3, 0..1);
                pass.set_pipeline(&self.compositor.multiply_source);
                pass.draw(0..3, 0..1);
            }
        }
        true
    }
}
