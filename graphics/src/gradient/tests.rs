use super::*;

#[test]
fn geometry_builders_keep_the_complete_authored_ramp() {
    let center = Point::new(16., 24.);
    let input = (0..17).rev().map(|index| ColorStop {
        offset: index as f32 / 16.,
        color: Color::BLACK,
    });
    let stops: Vec<_> = input
        .chain([ColorStop {
            offset: 0.5,
            color: Color::WHITE,
        }])
        .collect();
    let linear = Linear::new(Point::ORIGIN, center).add_stops(stops.iter().copied());
    let radial = Radial::new(center, 5.).add_stops(stops.iter().copied());
    let conic = Conic::new(center, 0.).add_stops(stops);

    for stops in [linear.stops, radial.stops, conic.stops] {
        let stops = stops.as_slice();
        assert_eq!(stops.len(), 18);
        assert!(stops.is_sorted_by(|a, b| a.offset <= b.offset));
        assert_eq!(
            stops[8],
            ColorStop {
                offset: 0.5,
                color: Color::BLACK
            }
        );
        assert_eq!(
            stops[9],
            ColorStop {
                offset: 0.5,
                color: Color::WHITE
            }
        );
    }
}

#[test]
fn inline_transport_keeps_f32_offsets_and_straight_linear_color() {
    let transparent = Color::from_rgba(0.1728395, 0.62741, 0.83391, 0.0);
    let translucent = Color::from_rgba(0.91834, 0.35008, 0.21073, 0.62917);
    let packed = Linear::new(Point::ORIGIN, Point::new(1.0, 0.0))
        .add_stop(0.50001, transparent)
        .add_stop(0.50002, translucent)
        .pack();

    assert_eq!(packed.meta, [0, 0, 2]);
    assert_eq!(packed.offsets, [0.50001, 0.50002]);
    assert_eq!(
        packed.colors,
        [transparent.into_linear(), translucent.into_linear()]
    );
}
