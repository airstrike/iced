//! Compare the two gradient pipelines against authored stops at physical pixel centers.
use super::{close, encode, headless, premultiplied};
use iced_wgpu::Renderer;
use iced_wgpu::core::renderer::{Headless, Quad, Renderer as _};
use iced_wgpu::core::{Color, Degrees, Gradient, Point, Radians, Rectangle, Size, gradient};
use iced_wgpu::graphics::geometry::{self, Renderer as _};

#[derive(Clone, Copy, Debug)]
enum Surface {
    Quad,
    Geometry,
}

const SURFACES: [Surface; 2] = [Surface::Quad, Surface::Geometry];
const SIDE: u32 = 256;
const RED: Color = Color::from_rgb(1., 0., 0.);
const BLUE: Color = Color::from_rgb(0., 0., 1.);
const GREEN: Color = Color::from_rgb(0., 1., 0.);

fn linear(stops: impl IntoIterator<Item = (f32, Color)>) -> Gradient {
    gradient::Linear::new(Degrees(90.))
        .add_stops(
            stops
                .into_iter()
                .map(|(offset, color)| gradient::ColorStop { offset, color }),
        )
        .into()
}

fn pixels(renderer: &mut Renderer, surface: Surface, gradient: Gradient, scale: u32) -> Vec<u8> {
    let size = Size::new(SIDE as f32, SIDE as f32);
    let bounds = Rectangle::with_size(size);
    renderer.reset(bounds);
    match surface {
        Surface::Quad => renderer.fill_quad(
            Quad {
                bounds,
                ..Quad::default()
            },
            gradient,
        ),
        Surface::Geometry => {
            let gradient: geometry::Gradient = match gradient {
                Gradient::Linear(linear) => {
                    let (start, end) = bounds.chord(linear.angle);
                    geometry::gradient::Linear::new(start, end)
                        .add_stops(linear.stops.iter().copied())
                        .into()
                }
                Gradient::Radial(radial) => {
                    let center =
                        Point::new(radial.center.x * size.width, radial.center.y * size.height);
                    let radius = radial.radius * size.width.hypot(size.height) / 2.;
                    geometry::gradient::Radial::new(center, radius)
                        .add_stops(radial.stops.iter().copied())
                        .into()
                }
                Gradient::Conic(conic) => {
                    let center =
                        Point::new(conic.center.x * size.width, conic.center.y * size.height);
                    geometry::gradient::Conic::new(center, conic.angle.0)
                        .add_stops(conic.stops.iter().copied())
                        .into()
                }
            };
            let mut frame = geometry::Frame::new(renderer, size);
            frame.fill_rectangle(Point::ORIGIN, size, gradient);
            renderer.draw_geometry(frame.into_geometry());
        }
    }
    Headless::screenshot(
        renderer,
        Size::new(SIDE * scale, SIDE * scale),
        scale as f32,
        Color::TRANSPARENT,
    )
}

fn check(bytes: &[u8], scale: u32, surface: Surface, x: u32, y: u32, color: Color) {
    let start = ((y * SIDE * scale + x) * 4) as usize;
    let actual: [u8; 4] = bytes[start..start + 4].try_into().unwrap();
    let expected = encode(premultiplied(color));
    assert!(
        close(actual, expected, 2),
        "{surface:?} at {scale}× ({x}, {y}) expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn every_authored_stop_and_right_hand_edge_reaches_both_pipelines() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let color = |i| [RED, BLUE, GREEN][i % 3];
    let at = |x: u32| (x as f32 + 0.5) / SIDE as f32;
    for surface in SURFACES {
        let ramp = linear((0..17).map(|i| (at(7 + 15 * i), color(i as usize))));
        let bytes = pixels(&mut renderer, surface, ramp, 1);
        for i in 0..17 {
            check(&bytes, 1, surface, 7 + 15 * i, 93, color(i as usize));
        }
        // A sub-f16-width band straddles the exact duplicate offset at this pixel center.
        let edge = at(128);
        let half_band = 1. / 32768.;
        let ramp = linear([
            (0., RED),
            (edge - half_band, RED),
            (edge - half_band, GREEN),
            (edge, GREEN),
            (edge, BLUE),
            (edge + half_band, BLUE),
            (edge + half_band, Color::WHITE),
        ]);
        let bytes = pixels(&mut renderer, surface, ramp, 1);
        for (x, expected) in [(127, RED), (128, BLUE), (129, Color::WHITE)] {
            check(&bytes, 1, surface, x, 93, expected);
        }
    }
}

#[test]
fn texture_pages_share_a_hard_edge_without_losing_or_doubling_opacity() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let red = Color { a: 0.5, ..RED };
    let blue = Color { a: 0.5, ..BLUE };
    let edge = 128.5 / SIDE as f32;
    // The first page ends at index 65,535. A later blue stop at that same exact
    // offset owns the pixel on the boundary. Drawing both pages would raise its alpha.
    let ramp = linear(
        (0..65_534)
            .map(|i| (0.49 * i as f32 / 65_534., red))
            .chain([(edge, red), (edge, GREEN), (edge, blue), (1., blue)]),
    );
    for surface in SURFACES {
        let bytes = pixels(&mut renderer, surface, ramp.clone(), 1);
        for (x, color) in [(7, red), (127, red), (128, blue), (129, blue), (248, blue)] {
            check(&bytes, 1, surface, x, 93, color);
        }
    }
}

#[test]
fn straight_color_and_opacity_interpolate_independently_and_short_ramps_have_defined_ends() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let clear_red = Color { a: 0., ..RED };
    for surface in SURFACES {
        let empty = pixels(&mut renderer, surface, linear([]), 1);
        check(&empty, 1, surface, 110, 20, Color::TRANSPARENT);
        let constant = pixels(&mut renderer, surface, linear([(0.7, GREEN)]), 1);
        for x in [3, 240] {
            check(&constant, 1, surface, x, 20, GREEN);
        }
        // Both variants describe the same line, one inline and one in the texture table.
        let middle = Color::from_linear_rgba(0.5, 0., 0.5, 0.5);
        for ramp in [
            linear([(0., clear_red), (1., BLUE)]),
            linear([(0., clear_red), (0.5, middle), (1., BLUE)]),
        ] {
            let bytes = pixels(&mut renderer, surface, ramp, 1);
            for x in [31, 63, 127, 191, 223] {
                let t = (x as f32 + 0.5) / SIDE as f32;
                let expected = Color::from_linear_rgba(1. - t, 0., t, t);
                check(&bytes, 1, surface, x, 20, expected);
            }
        }
    }
}

#[test]
fn radial_distances_and_conic_angles_use_the_same_logical_coordinates_at_both_scales() {
    let Some(mut renderer) = headless() else {
        return;
    };
    for scale in [1, 2] {
        for surface in SURFACES {
            let radial = gradient::Radial::new()
                .add_stop(0., Color::BLACK)
                .add_stop(1., Color::WHITE);
            let radius = radial.radius * (SIDE as f32).hypot(SIDE as f32) / 2.;
            let conic = gradient::Conic {
                angle: Radians(std::f32::consts::FRAC_PI_2),
                ..gradient::Conic::new()
            }
            .add_stop(0., Color::BLACK)
            .add_stop(1., Color::WHITE);
            let circle = pixels(&mut renderer, surface, radial.into(), scale);
            let sweep = pixels(&mut renderer, surface, conic.into(), scale);
            for (x, y) in [(180, 136), (125, 212), (51, 133), (119, 48)] {
                let (x, y) = (x * scale, y * scale);
                let dx = (x as f32 + 0.5) / scale as f32 - SIDE as f32 / 2.;
                let dy = (y as f32 + 0.5) / scale as f32 - SIDE as f32 / 2.;
                for (bytes, t) in [
                    (&circle, (dx.hypot(dy) / radius).min(1.)),
                    (
                        &sweep,
                        (dy.atan2(dx) / std::f32::consts::TAU).rem_euclid(1.),
                    ),
                ] {
                    check(
                        bytes,
                        scale,
                        surface,
                        x,
                        y,
                        Color::from_linear_rgba(t, t, t, 1.),
                    );
                }
            }
        }
    }
}
