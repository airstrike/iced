//! The ordered colors of a gradient.

use super::ColorStop;
use crate::Color;

use std::slice;
use std::sync::Arc;

/// Gradient stops in increasing offset order. At equal offsets the authored order is retained.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stops(Option<Arc<Vec<ColorStop>>>);

impl Stops {
    /// The authored stops, in painting order.
    pub fn as_slice(&self) -> &[ColorStop] {
        self.0.as_deref().map_or(&[], Vec::as_slice)
    }

    /// Iterate over all the authored stops.
    pub fn iter(&self) -> slice::Iter<'_, ColorStop> {
        self.as_slice().iter()
    }

    /// Add a stop, ignoring offsets outside the unit interval.
    pub fn add(&mut self, offset: f32, color: Color) {
        if !valid_offset(offset) {
            return;
        }

        let stops = Arc::make_mut(self.0.get_or_insert_with(Arc::default));
        let stop = ColorStop { offset, color };
        if stops.last().is_none_or(|previous| previous.offset <= offset) {
            stops.push(stop);
        } else {
            let after = stops.partition_point(|previous| previous.offset <= offset);
            stops.insert(after, stop);
        }
    }

    /// Multiply each stop's alpha by the given factor.
    pub fn scale_alpha(&mut self, factor: f32) {
        if let Some(stops) = &mut self.0 {
            for stop in Arc::make_mut(stops) {
                stop.color.a *= factor;
            }
        }
    }
}

impl Extend<ColorStop> for Stops {
    fn extend<T: IntoIterator<Item = ColorStop>>(&mut self, stops: T) {
        let mut incoming = stops.into_iter().filter(|stop| valid_offset(stop.offset));
        let Some(first) = incoming.next() else {
            return;
        };
        let stops = Arc::make_mut(self.0.get_or_insert_with(Arc::default));
        let boundary = stops.len().saturating_sub(1);
        stops.push(first);
        stops.extend(incoming);
        if !stops[boundary..].is_sorted_by(|a, b| a.offset <= b.offset) {
            stops.sort_by(|a, b| a.offset.partial_cmp(&b.offset).expect("validated finite offsets"));
        }
    }
}

impl<'a> IntoIterator for &'a Stops {
    type Item = &'a ColorStop;
    type IntoIter = slice::Iter<'a, ColorStop>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

fn valid_offset(offset: f32) -> bool {
    let valid = (0.0..=1.0).contains(&offset);
    if !valid {
        log::warn!("Gradient color stop must be within 0.0..=1.0 range.");
    }
    valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gradient::{Conic, Linear, Radial};

    #[test]
    fn all_three_builders_retain_every_stop_and_the_order_at_a_hard_edge() {
        let positions = (0..17).rev().map(|index| ColorStop {
            offset: index as f32 / 16.0,
            color: Color::BLACK,
        });
        let close = f32::from_bits(0.5f32.to_bits() + 1);
        let stops: Vec<_> = positions
            .chain([
                ColorStop {
                    offset: close,
                    color: Color::WHITE,
                },
                ColorStop {
                    offset: 0.5,
                    color: Color::WHITE,
                },
                ColorStop {
                    offset: 0.5,
                    color: Color::TRANSPARENT,
                },
                ColorStop {
                    offset: f32::NAN,
                    color: Color::WHITE,
                },
                ColorStop {
                    offset: f32::INFINITY,
                    color: Color::WHITE,
                },
                ColorStop {
                    offset: -0.1,
                    color: Color::WHITE,
                },
            ])
            .collect();

        for stops in [
            Linear::new(0.).add_stops(stops.iter().copied()).stops,
            Radial::new().add_stops(stops.iter().copied()).stops,
            Conic::new().add_stops(stops).stops,
        ] {
            let stops = stops.as_slice();
            assert_eq!(stops.len(), 20);
            assert!(stops.is_sorted_by_key(|stop| stop.offset.to_bits()));
            assert_eq!(stops[8].color, Color::BLACK);
            assert_eq!(
                stops[9],
                ColorStop {
                    offset: 0.5,
                    color: Color::WHITE
                }
            );
            assert_eq!(
                stops[10],
                ColorStop {
                    offset: 0.5,
                    color: Color::TRANSPARENT
                }
            );
            assert_eq!(stops[11].offset.to_bits(), close.to_bits());
        }
    }

    #[test]
    fn editing_a_clone_preserves_the_original_stop_colors() {
        let original = Linear::new(0.).add_stop(0.5, Color::WHITE);
        let changed = original.clone().add_stop(0.5, Color::BLACK).scale_alpha(0.5);
        assert_eq!(
            original.stops.as_slice(),
            &[ColorStop {
                offset: 0.5,
                color: Color::WHITE
            }]
        );
        assert_eq!(changed.stops.as_slice().len(), 2);
        assert!(changed.stops.iter().all(|stop| stop.color.a == 0.5));
    }
}
