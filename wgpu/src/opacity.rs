//! Isolated opacity surfaces share the renderer's device and queue.
use super::{Engine, layer, nudge, primitive};
use crate::core::{Rectangle, Size, renderer};
use crate::graphics::Viewport;
use std::sync::{Arc, Mutex};

/// How the color a composited subtree produces depends on its backdrop.
///
/// A pass-through group evaluates its children over fixed backdrops and
/// recovers their response from those evaluations (D115). Responses are
/// ordered by the evaluations that takes. A subtree responds like the
/// greatest response among the groups it composites without isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(super) enum Response {
    /// Source-over: the backdrop only shows through the uncovered fraction.
    #[default]
    Over,
    /// Affine, and like source-over on opaque white (Normal and Multiply).
    /// The transparent and black evaluations recover it.
    Multiply,
    /// Affine, scaling the backdrop by a factor in `[0, 1]` and offsetting it
    /// by a nonpositive multiple of its alpha (Screen and Hard Light). This
    /// also needs an evaluation over opaque white.
    Affine,
    /// Not affine in the backdrop (Darken, Lighten, Overlay, Soft Light,
    /// Color Dodge, Color Burn and Difference), or affine with a backdrop
    /// factor below zero that the fixed-function recovery would clamp
    /// (Exclusion, `1 - 2cs`). No evaluation set recovers it, so a
    /// pass-through ancestor isolates its children.
    Nonlinear,
}

impl Response {
    /// Resolves a group that blends with `blend` over children responding
    /// like `self`. A pass-through group that cannot recover its children's
    /// response isolates them and composites as [`Normal`].
    ///
    /// [`Normal`]: renderer::BlendMode::Normal
    pub(super) fn group(self, blend: renderer::BlendMode) -> (renderer::BlendMode, Self) {
        use renderer::BlendMode;
        match blend {
            BlendMode::PassThrough => match self {
                Self::Over | Self::Nonlinear => (BlendMode::Normal, Self::Over),
                children => (blend, children),
            },
            BlendMode::Normal => (blend, Self::Over),
            BlendMode::Multiply => (blend, Self::Multiply),
            BlendMode::Screen | BlendMode::HardLight => (blend, Self::Affine),
            BlendMode::Darken
            | BlendMode::ColorBurn
            | BlendMode::Lighten
            | BlendMode::ColorDodge
            | BlendMode::Overlay
            | BlendMode::SoftLight
            | BlendMode::Difference
            | BlendMode::Exclusion => (blend, Self::Nonlinear),
        }
    }
}

/// The `fs_backdrop` operator of a mode that needs a copy of its backdrop.
fn backdrop_operator(blend: renderer::BlendMode) -> Option<u32> {
    use renderer::BlendMode;
    match blend {
        BlendMode::Normal | BlendMode::PassThrough | BlendMode::Multiply | BlendMode::Screen => {
            None
        }
        BlendMode::Darken => Some(1),
        BlendMode::ColorBurn => Some(2),
        BlendMode::Lighten => Some(3),
        BlendMode::ColorDodge => Some(4),
        BlendMode::Overlay => Some(5),
        BlendMode::SoftLight => Some(6),
        BlendMode::HardLight => Some(7),
        BlendMode::Difference => Some(8),
        BlendMode::Exclusion => Some(9),
    }
}

#[derive(Default)]
pub(super) struct State {
    pub parents: Vec<(
        layer::Stack,
        Option<renderer::Scale>,
        Rectangle,
        f32,
        renderer::BlendMode,
        Response,
        Option<(renderer::Rotation, [f32; 4])>,
        Option<renderer::Rotation>,
    )>,
    pub response: Response,
    pub rounded_clips: Vec<bool>,
    pub surfaces: Vec<Surface>,
    pub next: usize,
    compositor: Option<Arc<Compositor>>,
}

#[derive(Debug)]
struct Compositor {
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    multiply_backdrop: wgpu::RenderPipeline,
    multiply_source: wgpu::RenderPipeline,
    screen: wgpu::RenderPipeline,
    pass_scale: wgpu::RenderPipeline,
    pass_scale_white: wgpu::RenderPipeline,
    pass_black: wgpu::RenderPipeline,
    pass_uncovered: wgpu::RenderPipeline,
    backdrop: wgpu::RenderPipeline,
    backdrop_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Compositor {
    fn new(engine: &Engine) -> Self {
        let device = &engine.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iced opacity"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("shader/rounded.wgsl"),
                    "\n",
                    include_str!("opacity.wgsl"),
                    "\n",
                    include_str!("shader/backdrop.wgsl"),
                )
                .into(),
            ),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            ty,
            visibility: if binding == 2 {
                wgpu::ShaderStages::VERTEX_FRAGMENT
            } else {
                wgpu::ShaderStages::FRAGMENT
            },
            count: None,
        };
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iced isolated layer inputs"),
            entries: &[
                entry(0, texture),
                entry(3, texture),
                entry(4, texture),
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
        let backdrop_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iced isolated layer backdrop"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("iced isolated layer composition"),
            bind_group_layouts: &[Some(&bindings)],
            immediate_size: 0,
        });
        let backdrop_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("iced isolated layer backdrop composition"),
                bind_group_layouts: &[Some(&bindings), Some(&backdrop_layout)],
                immediate_size: 0,
            });
        let pipeline_with = |layout, entry_point, blend, write_mask| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("iced isolated opacity"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry_point),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: engine.format,
                        blend,
                        write_mask,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline = |entry_point, blend, write_mask| {
            pipeline_with(&layout, entry_point, Some(blend), write_mask)
        };
        let color = |src_factor, dst_factor| wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor,
                dst_factor,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        use wgpu::BlendFactor::{
            Dst, One, OneMinusDstAlpha, OneMinusSrc, OneMinusSrcAlpha, Src, Zero,
        };
        Self {
            format: engine.format,
            pipeline: pipeline(
                "fs_main",
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
                wgpu::ColorWrites::ALL,
            ),
            multiply_backdrop: pipeline(
                "fs_main",
                color(Dst, OneMinusSrcAlpha),
                wgpu::ColorWrites::COLOR,
            ),
            multiply_source: pipeline(
                "fs_main",
                color(OneMinusDstAlpha, One),
                wgpu::ColorWrites::ALL,
            ),
            // Premultiplied screen is Cs + Cb - Cs*Cb.
            screen: pipeline("fs_main", color(One, OneMinusSrc), wgpu::ColorWrites::ALL),
            pass_scale: pipeline("fs_pass_scale", color(Zero, Src), wgpu::ColorWrites::COLOR),
            pass_scale_white: pipeline(
                "fs_pass_scale_white",
                color(Zero, Src),
                wgpu::ColorWrites::COLOR,
            ),
            pass_black: pipeline("fs_pass_black", color(One, One), wgpu::ColorWrites::COLOR),
            pass_uncovered: pipeline(
                "fs_pass_uncovered",
                color(OneMinusDstAlpha, One),
                wgpu::ColorWrites::ALL,
            ),
            backdrop: pipeline_with(
                &backdrop_pipeline_layout,
                "fs_backdrop",
                None,
                wgpu::ColorWrites::ALL,
            ),
            backdrop_layout,
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
    pub black: Option<wgpu::TextureView>,
    pub white: Option<wgpu::TextureView>,
    alpha: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    backdrop: Arc<Mutex<Option<Backdrop>>>,
    compositor: Arc<Compositor>,
}

/// The copy of a surface's target that `fs_backdrop` blends with.
///
/// It is sized in `prepare`, the first point where the surface's physical
/// placement in its target is known, and filled in `render`.
#[derive(Debug)]
struct Backdrop {
    texture: wgpu::Texture,
    bindings: wgpu::BindGroup,
    /// The surface's physical bounds in its target.
    bounds: Rectangle,
    /// The target pixels the surface can touch, copied to the origin of `texture`.
    region: Rectangle<u32>,
}

impl State {
    pub(super) fn surface(
        &mut self,
        engine: &Engine,
        size: Size<u32>,
        mut parameters: [f32; 20],
        blend: renderer::BlendMode,
        response: Response,
    ) -> Surface {
        let compositor = self
            .compositor
            .get_or_insert_with(|| Arc::new(Compositor::new(engine)));
        let pass_through = blend == renderer::BlendMode::PassThrough;
        let white = pass_through && response == Response::Affine;
        if self.surfaces.get(self.next).is_none_or(|s| {
            s.size != size || s.black.is_some() != pass_through || s.white.is_some() != white
        }) {
            let device = &engine.device;
            let make_view = || {
                device
                    .create_texture(&wgpu::TextureDescriptor {
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
                            | wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    })
                    .create_view(&wgpu::TextureViewDescriptor::default())
            };
            let view = make_view();
            let black = pass_through.then(make_view);
            let white = white.then(make_view);
            let alpha = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("iced group opacity"),
                size: 96,
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
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(
                            black.as_ref().unwrap_or(&view),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            white.as_ref().unwrap_or(&view),
                        ),
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
                black,
                white,
                alpha,
                bindings,
                backdrop: Arc::new(Mutex::new(None)),
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
        parameters[1] = backdrop_operator(blend).map_or(0., |operator| operator as f32);
        engine
            .queue
            .write_buffer(&surface.alpha, 0, bytemuck::bytes_of(&parameters));
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
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        if backdrop_operator(self.blend).is_none() {
            return;
        }
        let Ok(mut backdrop) = self.backdrop.lock() else {
            return;
        };
        let bounds = *bounds * viewport.scale_factor();
        let Some(region) = Rectangle::<f32>::from(Rectangle::with_size(viewport.physical_size()))
            .intersection(&bounds)
            .and_then(nudge::snap)
        else {
            // A placement from an earlier frame must not be blended with.
            *backdrop = None;
            return;
        };
        let reusable = backdrop.as_ref().filter(|backdrop| {
            backdrop.texture.width() >= region.width && backdrop.texture.height() >= region.height
        });
        let (texture, bindings) = if let Some(backdrop) = reusable {
            (backdrop.texture.clone(), backdrop.bindings.clone())
        } else {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("iced isolated layer backdrop"),
                size: wgpu::Extent3d {
                    width: region.width,
                    height: region.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.compositor.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("iced isolated layer backdrop"),
                layout: &self.compositor.backdrop_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                }],
            });
            (texture, bindings)
        };
        queue.write_buffer(
            &self.alpha,
            80,
            bytemuck::bytes_of(&[region.x as f32, region.y as f32, 0., 0.]),
        );
        *backdrop = Some(Backdrop {
            texture,
            bindings,
            bounds,
            region,
        });
    }
    fn draw(&self, _: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        match self.blend {
            renderer::BlendMode::Normal => {
                pass.set_pipeline(&self.compositor.pipeline);
                pass.set_bind_group(0, &self.bindings, &[]);
                pass.draw(0..3, 0..1);
            }
            renderer::BlendMode::PassThrough => {
                pass.set_bind_group(0, &self.bindings, &[]);
                let scale = if self.white.is_some() {
                    &self.compositor.pass_scale_white
                } else {
                    &self.compositor.pass_scale
                };
                for pipeline in [
                    scale,
                    &self.compositor.pass_black,
                    &self.compositor.pass_uncovered,
                ] {
                    pass.set_pipeline(pipeline);
                    pass.draw(0..3, 0..1);
                }
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
            renderer::BlendMode::Screen => {
                pass.set_pipeline(&self.compositor.screen);
                pass.set_bind_group(0, &self.bindings, &[]);
                pass.draw(0..3, 0..1);
            }
            renderer::BlendMode::Darken
            | renderer::BlendMode::ColorBurn
            | renderer::BlendMode::Lighten
            | renderer::BlendMode::ColorDodge
            | renderer::BlendMode::Overlay
            | renderer::BlendMode::SoftLight
            | renderer::BlendMode::HardLight
            | renderer::BlendMode::Difference
            | renderer::BlendMode::Exclusion => return false,
        }
        true
    }

    /// Copies the target under the surface and replaces it with the blend.
    ///
    /// A target that cannot be copied, such as a window surface without
    /// [`wgpu::TextureUsages::COPY_SRC`], gets the surface source-over.
    fn render(
        &self,
        _: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let Ok(backdrop) = self.backdrop.lock() else {
            return;
        };
        let Some(backdrop) = backdrop.as_ref() else {
            return;
        };
        let region = backdrop.region;
        let x = clip_bounds.x.max(region.x);
        let y = clip_bounds.y.max(region.y);
        let right = (clip_bounds.x + clip_bounds.width).min(region.x + region.width);
        let bottom = (clip_bounds.y + clip_bounds.height).min(region.y + region.height);
        if right <= x || bottom <= y {
            return;
        }
        let texture = target.texture();
        let copyable = texture.usage().contains(wgpu::TextureUsages::COPY_SRC)
            && texture.format().remove_srgb_suffix() == self.compositor.format.remove_srgb_suffix();
        if copyable {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &backdrop.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: x - region.x,
                        y: y - region.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: right - x,
                    height: bottom - y,
                    depth_or_array_layers: 1,
                },
            );
        } else {
            log::debug!(
                "composite {:?} source-over: its target cannot be copied",
                self.blend
            );
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("iced isolated layer backdrop composition"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let bounds = backdrop.bounds;
        pass.set_viewport(bounds.x, bounds.y, bounds.width, bounds.height, 0., 1.);
        pass.set_scissor_rect(x, y, right - x, bottom - y);
        pass.set_bind_group(0, &self.bindings, &[]);
        if copyable {
            pass.set_pipeline(&self.compositor.backdrop);
            pass.set_bind_group(1, &backdrop.bindings, &[]);
        } else {
            pass.set_pipeline(&self.compositor.pipeline);
        }
        pass.draw(0..3, 0..1);
    }
}
