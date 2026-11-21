//! Placement of a rectangular recording at a new center and angle.
use crate::{Point, Radians, Rectangle, Vector};

/// A rectangular surface rotated about its center in the parent plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    /// The unrotated rectangle centered at its final position.
    pub frame: Rectangle,
    /// Positive angles turn counterclockwise in screen coordinates.
    pub radians: f32,
}
impl Rotation {
    /// Axis-aligned extent of the four rotated corners.
    pub fn bounds(self) -> Rectangle {
        self.frame.rotate(Radians(self.radians))
    }

    /// Displacement from the center expressed in the unrotated surface axes.
    pub fn local_offset(self, point: Point) -> Vector {
        let offset = point - self.frame.center();
        let (s, c) = self.radians.sin_cos();
        Vector::new(c * offset.x - s * offset.y, s * offset.x + c * offset.y)
    }

    /// Antialiased rounded-rectangle coverage at one physical pixel center.
    pub fn coverage(self, point: Point, radii: [f32; 4]) -> f32 {
        let local = self.local_offset(point);
        let r = if local.y < 0. {
            if local.x < 0. { radii[0] } else { radii[1] }
        } else if local.x < 0. {
            radii[3]
        } else {
            radii[2]
        };
        let dx = local.x.abs() - self.frame.width * 0.5 + r;
        let dy = local.y.abs() - self.frame.height * 0.5 + r;
        let distance = dx.max(0.).hypot(dy.max(0.)) + dx.max(dy).min(0.) - r;
        (0.5 - distance).clamp(0., 1.)
    }
}
