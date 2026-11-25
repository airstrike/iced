//! A gradient that can be used as a fill for some geometry.
//!
//! For a gradient that you can use as a background variant for a widget, see [`Gradient`].
use crate::core::gradient::{ColorStop, Stops};
use crate::core::{self, Color, Point, Rectangle};

use bytemuck::{Pod, Zeroable};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
/// A fill which interpolates colors along a direction.
///
/// For a gradient which can be used as a fill for a background of a widget, see [`crate::core::Gradient`].
pub enum Gradient {
    /// A linear gradient interpolates colors along a direction from its `start` to its `end`
    /// point.
    Linear(Linear),
    /// A radial gradient interpolates colors in an outward circular pattern from a center point.
    Radial(Radial),
    /// A conic gradient interpolates colors around a center point by angle.
    Conic(Conic),
}

impl From<Linear> for Gradient {
    fn from(gradient: Linear) -> Self {
        Self::Linear(gradient)
    }
}

impl From<Radial> for Gradient {
    fn from(gradient: Radial) -> Self {
        Self::Radial(gradient)
    }
}

impl From<Conic> for Gradient {
    fn from(gradient: Conic) -> Self {
        Self::Conic(gradient)
    }
}

impl Gradient {
    /// The ordered stops shared with a renderer's long-gradient storage.
    pub fn stops(&self) -> &Stops {
        match self {
            Self::Linear(linear) => &linear.stops,
            Self::Radial(radial) => &radial.stops,
            Self::Conic(conic) => &conic.stops,
        }
    }

    /// Packs the [`Gradient`] for use in shader code.
    pub fn pack(&self) -> Packed {
        match self {
            Gradient::Linear(linear) => linear.pack(),
            Gradient::Radial(radial) => radial.pack(),
            Gradient::Conic(conic) => conic.pack(),
        }
    }
}

/// A linear gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct Linear {
    /// The absolute starting position of the gradient.
    pub start: Point,

    /// The absolute ending position of the gradient.
    pub end: Point,

    /// [`ColorStop`]s along the linear gradient direction.
    pub stops: Stops,
}

impl Linear {
    /// Creates a new [`Linear`] builder.
    pub fn new(start: Point, end: Point) -> Self {
        Self {
            start,
            end,
            stops: Stops::default(),
        }
    }

    /// Adds a new [`ColorStop`], defined by an offset and a color, to the gradient.
    ///
    /// Invalid offsets are ignored. Stops with the same offset keep their insertion order.
    pub fn add_stop(mut self, offset: f32, color: Color) -> Self {
        self.stops.add(offset, color);
        self
    }

    /// Adds multiple [`ColorStop`]s to the gradient.
    pub fn add_stops(mut self, stops: impl IntoIterator<Item = ColorStop>) -> Self {
        self.stops.extend(stops);
        self
    }

    /// Packs the [`Gradient`] for use in shader code.
    pub fn pack(&self) -> Packed {
        Packed::new(
            &self.stops,
            [self.start.x, self.start.y, self.end.x, self.end.y],
            0,
        )
    }
}

/// A radial gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct Radial {
    /// The absolute center of the gradient.
    pub center: Point,

    /// The absolute radius of the gradient.
    pub radius: f32,

    /// [`ColorStop`]s along the radial gradient.
    pub stops: Stops,
}

impl Radial {
    /// Creates a new [`Radial`] builder.
    pub fn new(center: Point, radius: f32) -> Self {
        Self {
            center,
            radius,
            stops: Stops::default(),
        }
    }

    /// Adds a new [`ColorStop`], defined by an offset and a color, to the gradient.
    ///
    /// Invalid offsets are ignored. Stops with the same offset keep their insertion order.
    pub fn add_stop(mut self, offset: f32, color: Color) -> Self {
        self.stops.add(offset, color);
        self
    }

    /// Adds multiple [`ColorStop`]s to the gradient.
    pub fn add_stops(mut self, stops: impl IntoIterator<Item = ColorStop>) -> Self {
        self.stops.extend(stops);
        self
    }

    /// Packs the [`Gradient`] for use in shader code.
    pub fn pack(&self) -> Packed {
        Packed::new(
            &self.stops,
            [self.center.x, self.center.y, self.radius, 0.0],
            1,
        )
    }
}

/// A conic gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct Conic {
    /// The absolute center of the gradient.
    pub center: Point,

    /// The angle the gradient starts from, in radians.
    ///
    /// `0.0` means the top (12 o'clock).
    pub angle: f32,

    /// [`ColorStop`]s along the conic gradient.
    pub stops: Stops,
}

impl Conic {
    /// Creates a new [`Conic`] builder.
    pub fn new(center: Point, angle: f32) -> Self {
        Self {
            center,
            angle,
            stops: Stops::default(),
        }
    }

    /// Adds a new [`ColorStop`], defined by an offset and a color, to the gradient.
    ///
    /// Invalid offsets are ignored. Stops with the same offset keep their insertion order.
    pub fn add_stop(mut self, offset: f32, color: Color) -> Self {
        self.stops.add(offset, color);
        self
    }

    /// Adds multiple [`ColorStop`]s to the gradient.
    pub fn add_stops(mut self, stops: impl IntoIterator<Item = ColorStop>) -> Self {
        self.stops.extend(stops);
        self
    }

    /// Packs the [`Gradient`] for use in shader code.
    pub fn pack(&self) -> Packed {
        Packed::new(
            &self.stops,
            [self.center.x, self.center.y, self.angle, 0.0],
            2,
        )
    }
}

/// Packed [`Gradient`] data for use in shader code.
#[derive(Debug, Copy, Clone, PartialEq, Zeroable, Pod)]
#[repr(C)]
pub struct Packed {
    colors: [[f32; 4]; 2],
    offsets: [f32; 2],
    direction: [f32; 4],
    // Kind, stop-table base, number of authored stops.
    meta: [u32; 3],
}

impl Packed {
    /// The number of stops carried directly in the vertex or instance.
    pub const INLINE: usize = 2;

    fn new(stops: &Stops, direction: [f32; 4], kind: u32) -> Self {
        let mut colors = [[0.; 4]; 2];
        let mut offsets = [0.; 2];
        let stops = stops.as_slice();
        if stops.len() <= Self::INLINE {
            for (index, stop) in stops.iter().enumerate() {
                colors[index] = stop.color.into_linear();
                offsets[index] = stop.offset;
            }
        }
        Self {
            colors,
            offsets,
            direction,
            meta: [kind, 0, u32::try_from(stops.len()).unwrap_or(u32::MAX)],
        }
    }
}

/// Creates a new [`Packed`] gradient for use in shader code.
pub fn pack(gradient: &core::Gradient, bounds: Rectangle) -> Packed {
    let (stops, direction, gradient_type) = match gradient {
        core::Gradient::Linear(linear) => {
            let (start, end) = bounds.chord(linear.angle);

            (&linear.stops, [start.x, start.y, end.x, end.y], 0)
        }
        core::Gradient::Radial(radial) => {
            let center = Point::new(
                bounds.x + bounds.width * radial.center.x,
                bounds.y + bounds.height * radial.center.y,
            );

            let half_diagonal =
                (bounds.width * bounds.width + bounds.height * bounds.height).sqrt() / 2.0;
            let radius = half_diagonal * radial.radius;

            (&radial.stops, [center.x, center.y, radius, 0.0], 1)
        }
        core::Gradient::Conic(conic) => {
            let center = Point::new(
                bounds.x + bounds.width * conic.center.x,
                bounds.y + bounds.height * conic.center.y,
            );

            (&conic.stops, [center.x, center.y, conic.angle.0, 0.0], 2)
        }
    };

    Packed::new(stops, direction, gradient_type)
}
