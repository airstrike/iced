//! Build and draw geometry for Vello.

use crate::core::{Point, Radians, Rectangle, Size, Vector};
use crate::graphics::cache::{self, Cached};
use crate::graphics::color;
use crate::graphics::geometry::fill::{self, Fill};
use crate::graphics::geometry::path::lyon_path;
use crate::graphics::geometry::{self, LineCap, LineJoin, Path, Stroke, Style, Text};
use crate::graphics::gradient::Gradient;

use std::sync::Arc;
use vello::kurbo::{self, Affine, BezPath};
use vello::peniko::color::DynamicColor;
use vello::peniko::{self, Brush, ColorStop};

/// A geometry primitive for the Vello renderer.
#[derive(Clone)]
pub enum Geometry {
    /// A live, non-cached geometry.
    Live {
        /// The Vello scene containing all drawing commands.
        scene: vello::Scene,
    },
    /// A cached geometry.
    Cached(Cache),
}

impl std::fmt::Debug for Geometry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Live { .. } => f.debug_struct("Geometry::Live").finish_non_exhaustive(),
            Self::Cached(_) => f.debug_struct("Geometry::Cached").finish_non_exhaustive(),
        }
    }
}

/// A cache for geometry.
#[derive(Clone)]
pub struct Cache {
    /// The cached Vello scene.
    pub scene: Arc<vello::Scene>,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            scene: Arc::new(vello::Scene::new()),
        }
    }
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache").finish_non_exhaustive()
    }
}

impl Cached for Geometry {
    type Cache = Cache;

    fn load(cache: &Self::Cache) -> Self {
        Geometry::Cached(cache.clone())
    }

    fn cache(self, _group: cache::Group, _previous: Option<Self::Cache>) -> Self::Cache {
        match self {
            Self::Live { scene } => Cache {
                scene: Arc::new(scene),
            },
            Self::Cached(cache) => cache,
        }
    }
}

/// A frame for drawing geometry using Vello.
pub struct Frame {
    clip_bounds: Rectangle,
    scene: vello::Scene,
    transforms: Transforms,
}

#[derive(Debug, Clone)]
struct Transforms {
    previous: Vec<Transform>,
    current: Transform,
}

#[derive(Debug, Clone, Copy)]
struct Transform(Affine);

impl Transform {
    fn is_identity(&self) -> bool {
        self.0 == Affine::IDENTITY
    }

    fn is_scale_translation(&self) -> bool {
        let [_, b, c, _, _, _] = self.0.as_coeffs();
        b.abs() < 2.0 * f64::EPSILON && c.abs() < 2.0 * f64::EPSILON
    }

    fn scale(&self) -> (f32, f32) {
        let [a, _, _, d, _, _] = self.0.as_coeffs();
        (a as f32, d as f32)
    }

    fn transform_point(&self, point: Point) -> Point {
        let p = kurbo::Point::new(point.x as f64, point.y as f64);
        let transformed = self.0 * p;
        Point::new(transformed.x as f32, transformed.y as f32)
    }

    fn transform_vector(&self, vector: Vector) -> kurbo::Vec2 {
        let [a, b, c, d, _, _] = self.0.as_coeffs();
        kurbo::Vec2::new(
            a * vector.x as f64 + c * vector.y as f64,
            b * vector.x as f64 + d * vector.y as f64,
        )
    }

    fn transform_style(&self, style: Style) -> Style {
        match style {
            Style::Solid(color) => Style::Solid(color),
            Style::Gradient(gradient) => Style::Gradient(self.transform_gradient(gradient)),
        }
    }

    fn transform_gradient(&self, mut gradient: Gradient) -> Gradient {
        match &mut gradient {
            Gradient::Linear(linear) => {
                linear.start = self.transform_point(linear.start);
                linear.end = self.transform_point(linear.end);
            }
        }
        gradient
    }

    fn transform_rectangle(&self, rectangle: Rectangle) -> (Rectangle, Radians) {
        let top_left = self.transform_point(rectangle.position());
        let top_right =
            self.transform_point(rectangle.position() + Vector::new(rectangle.width, 0.0));
        let bottom_left =
            self.transform_point(rectangle.position() + Vector::new(0.0, rectangle.height));
        Rectangle::with_vertices(top_left, top_right, bottom_left)
    }
}

impl Frame {
    /// Creates a new frame with the given clip bounds.
    pub fn new(clip_bounds: Rectangle) -> Self {
        Self {
            clip_bounds,
            scene: vello::Scene::new(),
            transforms: Transforms {
                previous: Vec::new(),
                current: Transform(Affine::IDENTITY),
            },
        }
    }

    fn to_vello_color(color: crate::core::Color) -> peniko::Color {
        let [r, g, b, a] = color::pack(color).components();
        peniko::Color::from_rgba8(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            (a * 255.0) as u8,
        )
    }

    fn style_to_brush(&self, style: Style) -> Brush {
        match style {
            Style::Solid(color) => Brush::Solid(Self::to_vello_color(color)),
            Style::Gradient(gradient) => match gradient {
                Gradient::Linear(linear) => {
                    let stops: Vec<ColorStop> = linear
                        .stops
                        .iter()
                        .filter_map(|s| *s)
                        .map(|stop| ColorStop {
                            offset: stop.offset,
                            color: DynamicColor::from_alpha_color(Self::to_vello_color(stop.color)),
                        })
                        .collect();

                    if stops.is_empty() {
                        return Brush::Solid(peniko::Color::BLACK);
                    }

                    peniko::Gradient::new_linear(
                        (linear.start.x as f64, linear.start.y as f64),
                        (linear.end.x as f64, linear.end.y as f64),
                    )
                    .with_stops(stops.as_slice())
                    .into()
                }
            },
        }
    }

    fn lyon_path_to_kurbo(path: &Path) -> BezPath {
        use lyon_path::PathEvent;

        let mut bez_path = BezPath::new();

        for event in path.raw().iter() {
            match event {
                PathEvent::Begin { at } => {
                    bez_path.move_to((at.x as f64, at.y as f64));
                }
                PathEvent::Line { to, .. } => {
                    bez_path.line_to((to.x as f64, to.y as f64));
                }
                PathEvent::Quadratic { ctrl, to, .. } => {
                    bez_path.quad_to((ctrl.x as f64, ctrl.y as f64), (to.x as f64, to.y as f64));
                }
                PathEvent::Cubic {
                    ctrl1, ctrl2, to, ..
                } => {
                    bez_path.curve_to(
                        (ctrl1.x as f64, ctrl1.y as f64),
                        (ctrl2.x as f64, ctrl2.y as f64),
                        (to.x as f64, to.y as f64),
                    );
                }
                PathEvent::End { close, .. } => {
                    if close {
                        bez_path.close_path();
                    }
                }
            }
        }

        bez_path
    }
}

impl geometry::frame::Backend for Frame {
    type Geometry = Geometry;

    #[inline]
    fn width(&self) -> f32 {
        self.clip_bounds.width
    }

    #[inline]
    fn height(&self) -> f32 {
        self.clip_bounds.height
    }

    #[inline]
    fn size(&self) -> Size {
        self.clip_bounds.size()
    }

    #[inline]
    fn center(&self) -> Point {
        Point::new(self.clip_bounds.width / 2.0, self.clip_bounds.height / 2.0)
    }

    #[inline]
    fn push_transform(&mut self) {
        self.transforms.previous.push(self.transforms.current);
    }

    #[inline]
    fn pop_transform(&mut self) {
        self.transforms.current = self.transforms.previous.pop().unwrap();
    }

    #[inline]
    fn translate(&mut self, translation: Vector) {
        self.transforms.current.0 = self
            .transforms
            .current
            .0
            .pre_translate((translation.x as f64, translation.y as f64).into());
    }

    #[inline]
    fn rotate(&mut self, angle: impl Into<Radians>) {
        self.transforms.current.0 = self.transforms.current.0.pre_rotate(angle.into().0 as f64);
    }

    #[inline]
    fn scale(&mut self, scale: impl Into<f32>) {
        let scale = scale.into();
        self.scale_nonuniform(Vector { x: scale, y: scale });
    }

    #[inline]
    fn scale_nonuniform(&mut self, scale: impl Into<Vector>) {
        let scale = scale.into();
        self.transforms.current.0 = self
            .transforms
            .current
            .0
            .pre_scale_non_uniform(scale.x as f64, scale.y as f64);
    }

    fn draft(&mut self, clip_bounds: Rectangle) -> Self {
        Frame::new(clip_bounds)
    }

    fn paste(&mut self, frame: Self) {
        self.scene.append(&frame.scene, None);
    }

    fn fill(&mut self, path: &Path, fill: impl Into<Fill>) {
        let fill = fill.into();
        let brush = self.style_to_brush(fill.style);
        let bez_path = Self::lyon_path_to_kurbo(path);

        let transform = if self.transforms.current.is_identity() {
            None
        } else {
            Some(self.transforms.current.0)
        };

        self.scene.fill(
            match fill.rule {
                fill::Rule::NonZero => peniko::Fill::NonZero,
                fill::Rule::EvenOdd => peniko::Fill::EvenOdd,
            },
            transform.unwrap_or(Affine::IDENTITY),
            &brush,
            None,
            &bez_path,
        );
    }

    fn fill_rectangle(&mut self, top_left: Point, size: Size, fill: impl Into<Fill>) {
        let fill = fill.into();
        let brush = self.style_to_brush(self.transforms.current.transform_style(fill.style));

        let top_left = self.transforms.current.transform_point(top_left);
        let size = self
            .transforms
            .current
            .transform_vector(Vector::new(size.width, size.height));

        let rect = kurbo::Rect::new(
            top_left.x as f64,
            top_left.y as f64,
            top_left.x as f64 + size.x,
            top_left.y as f64 + size.y,
        );

        self.scene.fill(
            match fill.rule {
                fill::Rule::NonZero => peniko::Fill::NonZero,
                fill::Rule::EvenOdd => peniko::Fill::EvenOdd,
            },
            Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    fn fill_text(&mut self, text: impl Into<Text>) {
        let text = text.into();
        // TODO: Add cached glyph-based text rendering for scale+translation transforms
        // Currently always falls back to path-based rendering
        text.draw_with(|path, color| self.fill(&path, color));
    }

    fn stroke<'a>(&mut self, path: &Path, stroke: impl Into<Stroke<'a>>) {
        let stroke = stroke.into();
        let brush = self.style_to_brush(stroke.style);
        let bez_path = Self::lyon_path_to_kurbo(path);

        let transform = if self.transforms.current.is_identity() {
            Affine::IDENTITY
        } else {
            self.transforms.current.0
        };

        let mut kurbo_stroke = kurbo::Stroke::new(stroke.width as f64)
            .with_caps(match stroke.line_cap {
                LineCap::Butt => kurbo::Cap::Butt,
                LineCap::Square => kurbo::Cap::Square,
                LineCap::Round => kurbo::Cap::Round,
            })
            .with_join(match stroke.line_join {
                LineJoin::Miter => kurbo::Join::Miter,
                LineJoin::Round => kurbo::Join::Round,
                LineJoin::Bevel => kurbo::Join::Bevel,
            });

        // Handle dashed lines using kurbo's built-in support
        if !stroke.line_dash.segments.is_empty() {
            let dashes: Vec<f64> = stroke
                .line_dash
                .segments
                .iter()
                .map(|&s| s as f64)
                .collect();
            kurbo_stroke = kurbo_stroke.with_dashes(stroke.line_dash.offset as f64, dashes);
        }

        self.scene
            .stroke(&kurbo_stroke, transform, &brush, None, &bez_path);
    }

    fn stroke_rectangle<'a>(&mut self, top_left: Point, size: Size, stroke: impl Into<Stroke<'a>>) {
        let stroke = stroke.into();
        let brush = self.style_to_brush(self.transforms.current.transform_style(stroke.style));

        let top_left = self.transforms.current.transform_point(top_left);
        let size = self
            .transforms
            .current
            .transform_vector(Vector::new(size.width, size.height));

        let rect = kurbo::Rect::new(
            top_left.x as f64,
            top_left.y as f64,
            top_left.x as f64 + size.x,
            top_left.y as f64 + size.y,
        );

        let kurbo_stroke = kurbo::Stroke::new(stroke.width as f64)
            .with_caps(match stroke.line_cap {
                LineCap::Butt => kurbo::Cap::Butt,
                LineCap::Square => kurbo::Cap::Square,
                LineCap::Round => kurbo::Cap::Round,
            })
            .with_join(match stroke.line_join {
                LineJoin::Miter => kurbo::Join::Miter,
                LineJoin::Round => kurbo::Join::Round,
                LineJoin::Bevel => kurbo::Join::Bevel,
            });

        self.scene
            .stroke(&kurbo_stroke, Affine::IDENTITY, &brush, None, &rect);
    }

    fn stroke_text<'a>(&mut self, text: impl Into<Text>, stroke: impl Into<Stroke<'a>>) {
        let text = text.into();
        let stroke = stroke.into();
        text.draw_with(|path, _color| self.stroke(&path, stroke));
    }

    fn draw_image(&mut self, bounds: Rectangle, image: impl Into<geometry::Image>) {
        // TODO: Implement image drawing - requires storing images for deferred rendering
        // like wgpu does, or access to Engine for immediate rendering
        let _ = (bounds, image.into());
    }

    fn draw_svg(&mut self, bounds: Rectangle, svg: impl Into<geometry::Svg>) {
        // TODO: Implement SVG drawing - requires storing SVGs for deferred rendering
        // like wgpu does, or access to Engine for immediate rendering
        let _ = (bounds, svg.into());
    }

    fn into_geometry(self) -> Self::Geometry {
        Geometry::Live { scene: self.scene }
    }
}
