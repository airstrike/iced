//! Immutable, unfiltered stop tables shared by the quad and mesh pipelines.
use crate::core::gradient::{ColorStop, Stops};
use crate::graphics::color;
use rustc_hash::{FxHashMap, FxHasher};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, Weak};
use wgpu::util::DeviceExt as _;

/// A page owns [first, last), except the outermost pages, which extend to infinity.
/// Adjacent pages repeat the boundary stop, providing both ends of every interval.
pub(crate) struct Pages {
    stops: Stops,
    pub groups: Vec<wgpu::BindGroup>,
}

#[derive(Clone)]
pub(crate) struct Tables(Arc<Storage>);

impl std::fmt::Debug for Tables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("gradient::Tables")
    }
}

impl std::fmt::Debug for Pages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pages")
            .field("pages", &self.groups.len())
            .finish()
    }
}

struct Storage {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    inline: Arc<Pages>,
    cache: Mutex<FxHashMap<u64, Vec<Weak<Pages>>>>,
}

impl Tables {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iced.gradient.stops.layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });
        let inline = Arc::new(Pages {
            stops: Stops::default(),
            groups: vec![page(device, queue, &layout, &[], false, false)],
        });
        Self(Arc::new(Storage {
            device: device.clone(),
            queue: queue.clone(),
            layout,
            inline,
            cache: Mutex::default(),
        }))
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.0.layout
    }

    pub fn get(&self, stops: Option<&Stops>) -> Arc<Pages> {
        let Some(stops) = stops else {
            return Arc::clone(&self.0.inline);
        };
        let mut hash = FxHasher::default();
        for stop in stops.as_slice() {
            for component in [
                stop.offset,
                stop.color.r,
                stop.color.g,
                stop.color.b,
                stop.color.a,
            ] {
                component.to_bits().hash(&mut hash);
            }
        }
        let mut cache = self.0.cache.lock().expect("gradient cache is available");
        let bucket = cache.entry(hash.finish()).or_default();
        if let Some(existing) = bucket
            .iter()
            .filter_map(Weak::upgrade)
            .find(|p| p.stops == *stops)
        {
            return existing;
        }
        let limit = self.0.device.limits().max_texture_dimension_2d;
        // Bound temporary upload memory as well as texture dimensions on small devices.
        let capacity = (u64::from(limit).pow(2) / 2).min(65_536) as usize;
        let source = stops.as_slice();
        let mut groups = Vec::new();
        for range in ranges(source.len(), capacity) {
            groups.push(page(
                &self.0.device,
                &self.0.queue,
                &self.0.layout,
                &source[range.clone()],
                range.start > 0,
                range.end < source.len(),
            ));
        }
        let pages = Arc::new(Pages {
            stops: stops.clone(),
            groups,
        });
        bucket.push(Arc::downgrade(&pages));
        pages
    }

    pub fn trim(&self) {
        self.0
            .cache
            .lock()
            .expect("gradient cache is available")
            .retain(|_, bucket| {
                bucket.retain(|pages| pages.strong_count() > 0);
                !bucket.is_empty()
            });
    }
}

fn ranges(len: usize, capacity: usize) -> impl Iterator<Item = std::ops::Range<usize>> {
    debug_assert!(capacity > 1);
    std::iter::successors((len > 0).then_some(0..len.min(capacity)), move |last| {
        (last.end < len).then(|| {
            let next = last.end - 1;
            next..next.saturating_add(capacity).min(len)
        })
    })
}

fn page(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    stops: &[ColorStop],
    previous: bool,
    next: bool,
) -> wgpu::BindGroup {
    let count = stops.len() as u32;
    let texels = (count * 2).max(1);
    let width = texels.min(device.limits().max_texture_dimension_2d);
    let height = texels.div_ceil(width);
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("iced.gradient.stops.texture"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut data = vec![[0.0f32; 4]; (width * height) as usize];
    encode_texels(stops, &mut data);
    queue.write_texture(
        texture.as_image_copy(),
        bytemuck::cast_slice(&data),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 16),
            rows_per_image: None,
        },
        extent,
    );
    let range = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("iced.gradient.stops.range"),
        contents: bytemuck::cast_slice(&[count, u32::from(previous), u32::from(next), 0]),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("iced.gradient.stops"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&Default::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: range.as_entire_binding(),
            },
        ],
    })
}

fn encode_texels(stops: &[ColorStop], data: &mut [[f32; 4]]) {
    for (texels, stop) in data.chunks_exact_mut(2).zip(stops) {
        texels[0] = stop.color.into_linear();
        texels[1] = [stop.offset, 0.0, 0.0, 0.0];
    }
}

pub(crate) fn fragment_options() -> wgpu::PipelineCompilationOptions<'static> {
    wgpu::PipelineCompilationOptions {
        constants: if color::GAMMA_CORRECTION {
            &[("GRADIENT_WEB_COLORS", 0.)]
        } else {
            &[("GRADIENT_WEB_COLORS", 1.)]
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorStop, encode_texels, ranges};
    use crate::core::Color;

    #[test]
    fn consecutive_stop_pages_overlap_by_one_authored_stop() {
        assert_eq!(ranges(4, 4).collect::<Vec<_>>(), [0..4]);
        assert_eq!(ranges(5, 4).collect::<Vec<_>>(), [0..4, 3..5]);
        assert_eq!(ranges(10, 4).collect::<Vec<_>>(), [0..4, 3..7, 6..10]);
    }

    #[test]
    fn page_upload_keeps_every_channel_and_duplicate_offset_in_order() {
        let stops = (0..18)
            .map(|i| ColorStop {
                offset: 0.50001 + (i / 2) as f32 * 0.00001,
                color: Color::from_rgba(0.13579, i as f32 / 17., 0.97531, i as f32 / 19.),
            })
            .collect::<Vec<_>>();
        let mut texels = vec![[0.; 4]; stops.len() * 2];
        encode_texels(&stops, &mut texels);

        for (stop, pair) in stops.iter().zip(texels.chunks_exact(2)) {
            assert_eq!(pair[0], stop.color.into_linear());
            assert_eq!(pair[1], [stop.offset, 0., 0., 0.]);
        }
    }
}
