//! Isolated group blend modes against premultiplied reference math.
//!
//! Every scene is drawn by the headless `wgpu` renderer and read back. The
//! reference composites premultiplied colors with the W3C Compositing and
//! Blending Level 1 formulas, in the renderer's working color space.
use iced_wgpu::Renderer;
use iced_wgpu::core::renderer::{self, BlendMode, Headless, Renderer as _};
use iced_wgpu::core::{Background, Color, Rectangle, Size};
use iced_wgpu::graphics::color;

use std::pin::pin;
use std::task::{Context, Poll, Waker};

#[cfg(feature = "geometry")]
#[path = "blend/gradient.rs"]
mod gradient;

/// A premultiplied color in the renderer's working space.
type Pixel = [f64; 4];

const MODES: [BlendMode; 12] = [
    BlendMode::Normal,
    BlendMode::Darken,
    BlendMode::Multiply,
    BlendMode::ColorBurn,
    BlendMode::Lighten,
    BlendMode::Screen,
    BlendMode::ColorDodge,
    BlendMode::Overlay,
    BlendMode::SoftLight,
    BlendMode::HardLight,
    BlendMode::Difference,
    BlendMode::Exclusion,
];

/// Colors chosen so each piecewise mode takes both of its branches. Each
/// component is a whole 8-bit step, so no surface rounds a tie.
const BACKDROPS: [[u8; 3]; 2] = [[51, 153, 191], [217, 89, 26]];
const SOURCES: [[u8; 3]; 2] = [[204, 77, 140], [38, 166, 115]];

/// Surfaces round in the working space. Gamma-correct surfaces round sRGB
/// encodings of linear values, which the reference only approximates.
const TOLERANCE: u8 = if color::GAMMA_CORRECTION { 2 } else { 1 };

#[test]
fn every_mode_matches_the_reference_over_opaque_and_translucent_backdrops() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let mut failures = Vec::new();

    for mode in MODES {
        for backdrop in BACKDROPS {
            for source in SOURCES {
                for backdrop_alpha in [255, 128] {
                    for source_alpha in [255, 204] {
                        for opacity in [1., 0.6] {
                            let backdrop = rgb8(backdrop, backdrop_alpha);
                            let source = rgb8(source, source_alpha);
                            let actual = draw(&mut renderer, Size::new(4, 4), |renderer| {
                                fill(renderer, square(4.), backdrop);
                                group(renderer, [0., 0., 4., 4.], opacity, mode, |renderer| {
                                    fill(renderer, square(4.), source);
                                });
                            })[5];
                            let target = store(premultiplied(backdrop));
                            let layer = scale(store(premultiplied(source)), opacity.into());
                            let expected = encode(composite(mode, layer, target));
                            if !close(actual, expected, TOLERANCE) {
                                failures.push(format!(
                                    "{mode:?} source {source:?} at {opacity} over {backdrop:?}: \
                                     {actual:?}, expected {expected:?}"
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn backdrop_modes_read_the_isolated_group_they_are_drawn_in() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let page = rgba([0.9, 0.9, 0.2], 1.);
    let inner = rgba([0.2, 0.5, 0.8], 0.5);
    let source = rgba([0.7, 0.2, 0.4], 0.8);

    for mode in [BlendMode::Darken, BlendMode::Overlay, BlendMode::Difference] {
        let actual = draw(&mut renderer, Size::new(4, 4), |renderer| {
            fill(renderer, square(4.), page);
            group(
                renderer,
                [0., 0., 4., 4.],
                0.7,
                BlendMode::Normal,
                |renderer| {
                    fill(renderer, square(4.), inner);
                    group(renderer, [0., 0., 4., 4.], 1., mode, |renderer| {
                        fill(renderer, square(4.), source);
                    });
                },
            );
        })[5];
        let isolated = composite(
            mode,
            store(premultiplied(source)),
            store(premultiplied(inner)),
        );
        let expected = encode(over(
            scale(store(isolated), 0.7),
            store(premultiplied(page)),
        ));

        assert!(
            close(actual, expected, TOLERANCE),
            "{mode:?}: {actual:?}, expected {expected:?}"
        );
    }
}

#[test]
fn a_later_sibling_draws_over_a_backdrop_blend_with_the_same_bounds() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let page = rgba([0.3, 0.6, 0.9], 0.5);
    let dark = rgba([0.1, 0.8, 0.4], 1.);
    let veil = rgba([0.9, 0.1, 0.1], 0.5);

    let actual = draw(&mut renderer, Size::new(4, 4), |renderer| {
        fill(renderer, square(4.), page);
        group(
            renderer,
            [0., 0., 4., 4.],
            1.,
            BlendMode::Darken,
            |renderer| {
                fill(renderer, square(4.), dark);
            },
        );
        group(
            renderer,
            [0., 0., 4., 4.],
            0.5,
            BlendMode::Normal,
            |renderer| {
                fill(renderer, square(4.), veil);
            },
        );
    })[5];
    let darkened = store(composite(
        BlendMode::Darken,
        store(premultiplied(dark)),
        store(premultiplied(page)),
    ));
    let expected = encode(over(scale(store(premultiplied(veil)), 0.5), darkened));

    assert!(
        close(actual, expected, TOLERANCE),
        "{actual:?}, expected {expected:?}"
    );
}

/// D115: a fractional pass-through frame mixes its backdrop with the result
/// of compositing its children directly onto that backdrop. The second
/// child's green takes Hard Light's screen branch and its red and blue take
/// the multiply branch.
#[test]
fn fractional_pass_through_preserves_the_backdrop_of_affine_children() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let backdrop = rgba([0.25, 0.5, 0.9], 0.5);
    let first = rgba([0.9, 0.2, 0.2], 1.);
    let second = rgba([0.2, 0.8, 0.3], 0.8);

    for mode in [BlendMode::Multiply, BlendMode::Screen, BlendMode::HardLight] {
        let actual = pass_through_scene(&mut renderer, backdrop, first, second, mode);
        let d = premultiplied(backdrop);
        let a = premultiplied(first);
        let b = premultiplied(second);
        let expected = [
            mix(d, over(a, d), 0.5),
            mix(d, composite(mode, b, over(a, d)), 0.5),
            mix(d, composite(mode, b, d), 0.5),
        ]
        .map(encode);

        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                close(actual, expected, 3),
                "{mode:?}: {actual:?}, expected {expected:?}"
            );
        }
    }
}

/// A nonlinear child cannot be recovered from fixed backdrop evaluations, so
/// its pass-through frame isolates the children and composites normally.
#[test]
fn fractional_pass_through_isolates_nonlinear_children() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let backdrop = rgba([0.25, 0.5, 0.9], 0.5);
    let first = rgba([0.9, 0.2, 0.2], 1.);
    let second = rgba([0.2, 0.8, 0.3], 0.8);

    for mode in [BlendMode::Darken, BlendMode::Overlay, BlendMode::Exclusion] {
        let actual = pass_through_scene(&mut renderer, backdrop, first, second, mode);
        let d = premultiplied(backdrop);
        let a = premultiplied(first);
        let b = premultiplied(second);
        let expected =
            [a, composite(mode, b, a), b].map(|isolated| encode(over(scale(isolated, 0.5), d)));

        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                close(actual, expected, 3),
                "{mode:?}: {actual:?}, expected {expected:?}"
            );
        }
    }
}

/// A rounded clip composites as a full-opacity pass-through frame.
#[test]
fn rounded_clips_keep_the_backdrop_of_screen_children() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let backdrop = rgba([0.25, 0.5, 0.9], 0.5);
    let source = rgba([0.8, 0.3, 0.1], 0.8);

    let actual = draw(&mut renderer, Size::new(16, 16), |renderer| {
        fill(renderer, square(16.), backdrop);
        renderer.start_content_clip(square(16.), square(16.), [4.; 4]);
        group(
            renderer,
            [0., 0., 16., 16.],
            1.,
            BlendMode::Screen,
            |renderer| {
                fill(renderer, square(16.), source);
            },
        );
        renderer.end_content_clip();
    })[8 * 16 + 8];
    let d = premultiplied(backdrop);
    let expected = encode(composite(BlendMode::Screen, premultiplied(source), d));

    assert!(
        close(actual, expected, 3),
        "{actual:?}, expected {expected:?}"
    );
}

/// Draws a 50% pass-through frame over `backdrop` holding a Normal child on
/// columns 0-3 and a `mode` child on columns 2-5, and samples columns 1, 3, 5.
fn pass_through_scene(
    renderer: &mut Renderer,
    backdrop: Color,
    first: Color,
    second: Color,
    mode: BlendMode,
) -> [[u8; 4]; 3] {
    let pixels = draw(renderer, Size::new(6, 2), |renderer| {
        fill(
            renderer,
            Rectangle::new([0., 0.].into(), [6., 2.].into()),
            backdrop,
        );
        group(
            renderer,
            [0., 0., 6., 2.],
            0.5,
            BlendMode::PassThrough,
            |renderer| {
                fill(
                    renderer,
                    Rectangle::new([0., 0.].into(), [4., 2.].into()),
                    first,
                );
                group(renderer, [2., 0., 4., 2.], 1., mode, |renderer| {
                    fill(
                        renderer,
                        Rectangle::new([2., 0.].into(), [4., 2.].into()),
                        second,
                    );
                });
            },
        );
    });
    [pixels[1], pixels[3], pixels[5]]
}

/// The same corner can cut pixels across the frame's centreline. The image
/// sampler and a clipped group must agree away from the antialiasing band.
#[cfg(feature = "image")]
#[test]
fn rounded_images_and_clips_fit_each_incident_edge() {
    use iced_wgpu::core::image::{self, Renderer as _};

    let Some(mut renderer) = headless() else {
        return;
    };
    let white = image::Handle::from_rgba(1, 1, vec![255; 4]);
    for (width, height, radii, fitted) in [
        (100, 400, [100., 0., 0., 0.], [100., 0., 0., 0.]),
        (100, 400, [200., 0., 0., 0.], [100., 0., 0., 0.]),
        (100, 400, [80., 40., 0., 0.], [200. / 3., 100. / 3., 0., 0.]),
        (
            100,
            400,
            [200. / 3., 100. / 3., 0., 0.],
            [200. / 3., 100. / 3., 0., 0.],
        ),
        (100, 400, [0., 0., 200., 0.], [0., 0., 100., 0.]),
        (400, 100, [0., 200., 0., 50.], [0., 100., 0., 50.]),
        (100, 100, [100., 0., 100., 0.], [100., 0., 100., 0.]),
        (24, 40, [3., 4., 5., 7.], [3., 4., 5., 7.]),
        (24, 40, [0.; 4], [0.; 4]),
        (24, 40, [40.; 4], [12.; 4]),
    ] {
        let size = Size::new(width, height);
        let bounds = Rectangle::with_size(Size::new(width as f32, height as f32));
        let raster = draw(&mut renderer, size, |renderer| {
            let mut source = image::Image::new(white.clone());
            let [top_left, top_right, bottom_right, bottom_left] = radii;
            source.border_radius = iced_wgpu::core::border::Radius {
                top_left,
                top_right,
                bottom_right,
                bottom_left,
            };
            renderer.draw_image(source, bounds, bounds);
        });
        let clipped = draw(&mut renderer, size, |renderer| {
            renderer.start_content_clip(bounds, bounds, radii);
            fill(renderer, bounds, Color::WHITE);
            renderer.end_content_clip();
        });

        for (i, (image, clip)) in raster.iter().zip(clipped).enumerate() {
            let x = (i % width as usize) as f32 + 0.5;
            let y = (i / width as usize) as f32 + 0.5;
            let right = width as f32 - x;
            let bottom = height as f32 - y;
            let mut inside = true;
            let mut near_arc = false;
            for ((dx, dy), r) in [(x, y), (right, y), (right, bottom), (x, bottom)]
                .into_iter()
                .zip(fitted)
            {
                if dx < r && dy < r {
                    let distance = (dx - r).hypot(dy - r) - r;
                    inside &= distance <= 0.;
                    near_arc |= distance.abs() <= 1.5;
                }
            }
            if !near_arc {
                let alpha = if inside { 255 } else { 0 };
                assert_eq!(
                    (image[3], clip[3]),
                    (alpha, alpha),
                    "radii {radii:?} at ({x}, {y})"
                );
            }
        }
    }
}

fn headless() -> Option<Renderer> {
    let renderer = block_on(<Renderer as Headless>::new(
        renderer::Settings::default(),
        None,
    ));
    if renderer.is_none() {
        eprintln!("skipped: no wgpu adapter is available");
    }
    renderer
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}

fn draw(
    renderer: &mut Renderer,
    size: Size<u32>,
    scene: impl FnOnce(&mut Renderer),
) -> Vec<[u8; 4]> {
    renderer.reset(Rectangle::with_size(Size::new(
        size.width as f32,
        size.height as f32,
    )));
    scene(renderer);
    Headless::screenshot(renderer, size, 1., Color::TRANSPARENT)
        .chunks_exact(4)
        .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
        .collect()
}

fn group(
    renderer: &mut Renderer,
    [x, y, width, height]: [f32; 4],
    opacity: f32,
    blend: BlendMode,
    children: impl FnOnce(&mut Renderer),
) {
    renderer.start_composite(
        Rectangle::new([x, y].into(), [width, height].into()),
        opacity,
        blend,
    );
    children(renderer);
    renderer.end_opacity();
}

fn fill(renderer: &mut Renderer, bounds: Rectangle, color: Color) {
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            ..renderer::Quad::default()
        },
        Background::Color(color),
    );
}

fn square(size: f32) -> Rectangle {
    Rectangle::with_size(Size::new(size, size))
}

fn rgba([r, g, b]: [f32; 3], a: f32) -> Color {
    Color::from_rgba(r, g, b, a)
}

fn rgb8([r, g, b]: [u8; 3], a: u8) -> Color {
    Color::from_rgba8(r, g, b, f32::from(a) / 255.)
}

fn premultiplied(color: Color) -> Pixel {
    let [r, g, b, a] = color::pack(color).components().map(f64::from);
    [r * a, g * a, b * a, a]
}

/// Source-over.
fn over(source: Pixel, backdrop: Pixel) -> Pixel {
    composite(BlendMode::Normal, source, backdrop)
}

/// The W3C separable composite of premultiplied `source` over `backdrop`.
fn composite(mode: BlendMode, source: Pixel, backdrop: Pixel) -> Pixel {
    let straight = |pixel: Pixel, channel: usize| {
        if pixel[3] > 0. {
            (pixel[channel] / pixel[3]).clamp(0., 1.)
        } else {
            0.
        }
    };
    let (sa, ba) = (source[3], backdrop[3]);
    let mut result = [0., 0., 0., sa + ba * (1. - sa)];
    for (channel, value) in result.iter_mut().take(3).enumerate() {
        let blended = separable(mode, straight(backdrop, channel), straight(source, channel));
        *value = source[channel] * (1. - ba) + backdrop[channel] * (1. - sa) + sa * ba * blended;
    }
    result
}

fn separable(mode: BlendMode, cb: f64, cs: f64) -> f64 {
    let screen = |cb: f64, cs: f64| cb + cs - cb * cs;
    let hard_light = |cb: f64, cs: f64| {
        if cs <= 0.5 {
            cb * 2. * cs
        } else {
            screen(cb, 2. * cs - 1.)
        }
    };
    match mode {
        BlendMode::Normal | BlendMode::PassThrough => cs,
        BlendMode::Darken => cb.min(cs),
        BlendMode::Multiply => cb * cs,
        BlendMode::ColorBurn if cb >= 1. => 1.,
        BlendMode::ColorBurn if cs <= 0. => 0.,
        BlendMode::ColorBurn => 1. - ((1. - cb) / cs).min(1.),
        BlendMode::Lighten => cb.max(cs),
        BlendMode::Screen => screen(cb, cs),
        BlendMode::ColorDodge if cb <= 0. => 0.,
        BlendMode::ColorDodge if cs >= 1. => 1.,
        BlendMode::ColorDodge => (cb / (1. - cs)).min(1.),
        BlendMode::Overlay => hard_light(cs, cb),
        BlendMode::SoftLight if cs <= 0.5 => cb - (1. - 2. * cs) * cb * (1. - cb),
        BlendMode::SoftLight => {
            let d = if cb <= 0.25 {
                ((16. * cb - 12.) * cb + 4.) * cb
            } else {
                cb.sqrt()
            };
            cb + (2. * cs - 1.) * (d - cb)
        }
        BlendMode::HardLight => hard_light(cb, cs),
        BlendMode::Difference => (cb - cs).abs(),
        BlendMode::Exclusion => cb + cs - 2. * cb * cs,
    }
}

fn scale(pixel: Pixel, factor: f64) -> Pixel {
    pixel.map(|channel| channel * factor)
}

fn mix(backdrop: Pixel, result: Pixel, factor: f64) -> Pixel {
    std::array::from_fn(|channel| {
        backdrop[channel] + factor * (result[channel] - backdrop[channel])
    })
}

/// Rounds a pixel the way an 8-bit surface of the working format stores it.
fn store(pixel: Pixel) -> Pixel {
    let bytes = encode(pixel);
    std::array::from_fn(|channel| {
        let value = f64::from(bytes[channel]) / 255.;
        if channel < 3 && color::GAMMA_CORRECTION {
            decode(value)
        } else {
            value
        }
    })
}

fn encode(pixel: Pixel) -> [u8; 4] {
    std::array::from_fn(|channel| {
        let value = pixel[channel].clamp(0., 1.);
        let value = if channel < 3 && color::GAMMA_CORRECTION {
            if value <= 0.003_130_8 {
                value * 12.92
            } else {
                1.055 * value.powf(1. / 2.4) - 0.055
            }
        } else {
            value
        };
        (value * 255.).round() as u8
    })
}

fn decode(value: f64) -> f64 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn close(actual: [u8; 4], expected: [u8; 4], tolerance: u8) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
}
