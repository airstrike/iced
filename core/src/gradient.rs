//! Colors that transition progressively.
use crate::{Color, Point, Radians};

use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq)]
/// A fill which transitions colors progressively along a direction, either linearly, radially,
/// or conically.
pub enum Gradient {
    /// A linear gradient interpolates colors along a direction at a specific angle.
    Linear(Linear),
    /// A radial gradient interpolates colors in an outward circular pattern from a center point.
    Radial(Radial),
    /// A conic gradient interpolates colors around a center point by angle.
    Conic(Conic),
}

impl Gradient {
    /// Scales the alpha channel of the [`Gradient`] by the given factor.
    pub fn scale_alpha(self, factor: f32) -> Self {
        match self {
            Gradient::Linear(linear) => Gradient::Linear(linear.scale_alpha(factor)),
            Gradient::Radial(radial) => Gradient::Radial(radial.scale_alpha(factor)),
            Gradient::Conic(conic) => Gradient::Conic(conic.scale_alpha(factor)),
        }
    }
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

#[derive(Debug, Default, Clone, Copy, PartialEq)]
/// A point along the gradient vector where the specified [`color`] is unmixed.
///
/// [`color`]: Self::color
pub struct ColorStop {
    /// Offset along the gradient vector.
    pub offset: f32,

    /// The color of the gradient at the specified [`offset`].
    ///
    /// [`offset`]: Self::offset
    pub color: Color,
}

/// A linear gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Linear {
    /// How the [`Gradient`] is angled within its bounds.
    pub angle: Radians,
    /// [`ColorStop`]s along the linear gradient path.
    pub stops: [Option<ColorStop>; 8],
}

impl Linear {
    /// Creates a new [`Linear`] gradient with the given angle in [`Radians`].
    pub fn new(angle: impl Into<Radians>) -> Self {
        Self {
            angle: angle.into(),
            stops: [None; 8],
        }
    }

    /// Adds a new [`ColorStop`], defined by an offset and a color, to the gradient.
    ///
    /// Any `offset` that is not within `0.0..=1.0` will be silently ignored.
    ///
    /// Any stop added after the 8th will be silently ignored.
    pub fn add_stop(mut self, offset: f32, color: Color) -> Self {
        if offset.is_finite() && (0.0..=1.0).contains(&offset) {
            let (Ok(index) | Err(index)) = self.stops.binary_search_by(|stop| match stop {
                None => Ordering::Greater,
                Some(stop) => stop.offset.partial_cmp(&offset).unwrap(),
            });

            if index < 8 {
                self.stops[index] = Some(ColorStop { offset, color });
            }
        } else {
            log::warn!("Gradient color stop must be within 0.0..=1.0 range.");
        };

        self
    }

    /// Adds multiple [`ColorStop`]s to the gradient.
    ///
    /// Any stop added after the 8th will be silently ignored.
    pub fn add_stops(mut self, stops: impl IntoIterator<Item = ColorStop>) -> Self {
        for stop in stops {
            self = self.add_stop(stop.offset, stop.color);
        }

        self
    }

    /// Scales the alpha channel of the [`Linear`] gradient by the given
    /// factor.
    pub fn scale_alpha(mut self, factor: f32) -> Self {
        for stop in self.stops.iter_mut().flatten() {
            stop.color.a *= factor;
        }

        self
    }
}

/// A radial gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radial {
    /// The center of the gradient, as a ratio of the bounding box.
    ///
    /// `Point::new(0.5, 0.5)` means the center of the bounding box.
    pub center: Point,
    /// The radius of the gradient, as a fraction of the half-diagonal
    /// of the bounding box.
    ///
    /// `1.0` means the gradient extends to the half-diagonal length.
    pub radius: f32,
    /// [`ColorStop`]s along the radial gradient.
    pub stops: [Option<ColorStop>; 8],
}

impl Default for Radial {
    fn default() -> Self {
        Self {
            center: Point::new(0.5, 0.5),
            radius: 1.0,
            stops: [None; 8],
        }
    }
}

impl Radial {
    /// Creates a new [`Radial`] gradient centered within its bounds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new [`ColorStop`], defined by an offset and a color, to the gradient.
    ///
    /// Any `offset` that is not within `0.0..=1.0` will be silently ignored.
    ///
    /// Any stop added after the 8th will be silently ignored.
    pub fn add_stop(mut self, offset: f32, color: Color) -> Self {
        if offset.is_finite() && (0.0..=1.0).contains(&offset) {
            let (Ok(index) | Err(index)) = self.stops.binary_search_by(|stop| match stop {
                None => Ordering::Greater,
                Some(stop) => stop.offset.partial_cmp(&offset).unwrap(),
            });

            if index < 8 {
                self.stops[index] = Some(ColorStop { offset, color });
            }
        } else {
            log::warn!("Gradient color stop must be within 0.0..=1.0 range.");
        };

        self
    }

    /// Adds multiple [`ColorStop`]s to the gradient.
    ///
    /// Any stop added after the 8th will be silently ignored.
    pub fn add_stops(mut self, stops: impl IntoIterator<Item = ColorStop>) -> Self {
        for stop in stops {
            self = self.add_stop(stop.offset, stop.color);
        }

        self
    }

    /// Scales the alpha channel of the [`Radial`] gradient by the given
    /// factor.
    pub fn scale_alpha(mut self, factor: f32) -> Self {
        for stop in self.stops.iter_mut().flatten() {
            stop.color.a *= factor;
        }

        self
    }
}

/// A conic gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conic {
    /// The center of the gradient, as a ratio of the bounding box.
    ///
    /// `Point::new(0.5, 0.5)` means the center of the bounding box.
    pub center: Point,
    /// The angle the gradient starts from, in [`Radians`].
    ///
    /// `0.0` means the top (12 o'clock), matching CSS `from` semantics.
    pub angle: Radians,
    /// [`ColorStop`]s along the conic gradient.
    pub stops: [Option<ColorStop>; 8],
}

impl Default for Conic {
    fn default() -> Self {
        Self {
            center: Point::new(0.5, 0.5),
            angle: Radians(0.0),
            stops: [None; 8],
        }
    }
}

impl Conic {
    /// Creates a new [`Conic`] gradient centered within its bounds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new [`ColorStop`], defined by an offset and a color, to the gradient.
    ///
    /// Any `offset` that is not within `0.0..=1.0` will be silently ignored.
    ///
    /// Any stop added after the 8th will be silently ignored.
    pub fn add_stop(mut self, offset: f32, color: Color) -> Self {
        if offset.is_finite() && (0.0..=1.0).contains(&offset) {
            let (Ok(index) | Err(index)) = self.stops.binary_search_by(|stop| match stop {
                None => Ordering::Greater,
                Some(stop) => stop.offset.partial_cmp(&offset).unwrap(),
            });

            if index < 8 {
                self.stops[index] = Some(ColorStop { offset, color });
            }
        } else {
            log::warn!("Gradient color stop must be within 0.0..=1.0 range.");
        };

        self
    }

    /// Adds multiple [`ColorStop`]s to the gradient.
    ///
    /// Any stop added after the 8th will be silently ignored.
    pub fn add_stops(mut self, stops: impl IntoIterator<Item = ColorStop>) -> Self {
        for stop in stops {
            self = self.add_stop(stop.offset, stop.color);
        }

        self
    }

    /// Scales the alpha channel of the [`Conic`] gradient by the given
    /// factor.
    pub fn scale_alpha(mut self, factor: f32) -> Self {
        for stop in self.stops.iter_mut().flatten() {
            stop.color.a *= factor;
        }

        self
    }
}
