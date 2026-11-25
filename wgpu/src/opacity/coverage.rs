use crate::core::{Rectangle, Size};
use crate::graphics::geometry::{Path, fill, path::lyon_path::PathEvent};

/// The coverage cache key for one isolated surface. The target dimensions are
/// already part of that surface's allocation key.
#[derive(Debug, PartialEq)]
pub(crate) struct Coverage {
    path: Option<tiny_skia::Path>,
    rule: fill::Rule,
    transform: tiny_skia::Transform,
}

impl Coverage {
    pub(crate) fn new(path: &Path, rule: fill::Rule, visible: Rectangle, scale: f32) -> Self {
        let mut builder = tiny_skia::PathBuilder::new();
        for event in path.raw().iter() {
            match event {
                PathEvent::Begin { at } => builder.move_to(at.x, at.y),
                PathEvent::Line { to, .. } => builder.line_to(to.x, to.y),
                PathEvent::Quadratic { ctrl, to, .. } => {
                    builder.quad_to(ctrl.x, ctrl.y, to.x, to.y)
                }
                PathEvent::Cubic {
                    ctrl1, ctrl2, to, ..
                } => builder.cubic_to(ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, to.x, to.y),
                PathEvent::End { close: true, .. } => builder.close(),
                PathEvent::End { close: false, .. } => {}
            }
        }
        Self {
            path: builder.finish(),
            rule,
            transform: tiny_skia::Transform::from_row(
                scale,
                0.,
                0.,
                scale,
                -visible.x * scale,
                -visible.y * scale,
            ),
        }
    }

    fn rasterize(&self, size: Size<u32>) -> Option<tiny_skia::Mask> {
        let mut mask = tiny_skia::Mask::new(size.width, size.height)?;
        if let Some(path) = self.path.as_ref() {
            let rule = match self.rule {
                fill::Rule::NonZero => tiny_skia::FillRule::Winding,
                fill::Rule::EvenOdd => tiny_skia::FillRule::EvenOdd,
            };
            mask.fill_path(path, rule, true, self.transform);
        }
        Some(mask)
    }

    pub(super) fn upload(
        &self,
        texture: &wgpu::Texture,
        queue: &wgpu::Queue,
        size: Size<u32>,
    ) -> bool {
        let Some(mask) = self.rasterize(size) else {
            return false;
        };
        queue.write_texture(
            texture.as_image_copy(),
            mask.data(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.width),
                rows_per_image: None,
            },
            texture.size(),
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn coverage_respects_winding_zoom_and_a_cropped_viewport() {
        let outline = Path::new(|p| {
            p.rectangle(Point::new(4., 2.), Size::new(12., 12.));
            p.rectangle(Point::new(8., 6.), Size::new(4., 4.));
        });
        let visible = Rectangle::new(Point::new(6., 4.), Size::new(14., 12.));
        for (rule, center) in [(fill::Rule::EvenOdd, 0), (fill::Rule::NonZero, 255)] {
            let key = Coverage::new(&outline, rule, visible, 2.);
            let pixels = key.rasterize(Size::new(28, 24)).expect("small path mask");
            assert_eq!(pixels.data()[8 * 28 + 8], center, "hole at (10, 8)");
            assert_eq!(pixels.data()[1], 255, "full frame, clipped viewport");
            assert_eq!(pixels.data()[26], 0, "outside the path");
        }
    }
}
