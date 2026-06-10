//! Layout tests for bounded lengths on widgets that measure their content.
use iced::Length::{self, Shrink};
use iced::widget::container;
use iced::{Element, Rectangle, Renderer, Theme};
use iced_test::selector::id;
use iced_test::simulator;

/// Lays out `view` and returns the bounds of the widget tagged `"target"`.
fn target_bounds(view: impl Into<Element<'static, (), Theme, Renderer>>) -> Rectangle {
    let mut ui = simulator(view);
    ui.find(id("target"))
        .expect("target widget must be found")
        .bounds()
}

/// A `pick_list` measures its option labels to size itself under `Shrink`. A
/// non-binding `Shrink.max(N)` must keep doing so, not collapse to zero.
#[test]
fn pick_list_bounded_shrink_measures_labels() {
    fn width(length: Length) -> f32 {
        let options = vec!["Apple", "Banana", "Clementine"];

        let view = container(
            iced::widget::pick_list(Some("Banana"), options, |s: &&str| s.to_string())
                .width(length),
        )
        .id("target");

        target_bounds(view).width
    }

    let shrink = width(Shrink);
    let bounded = width(Shrink.max(1000));

    assert!(shrink > 0.0, "Shrink pick_list should measure its labels");
    assert_eq!(
        bounded, shrink,
        "a non-binding max bound should not change the measured width"
    );
}

/// An `svg` shrinks to its content under `Shrink`. A non-binding `Shrink.max(N)`
/// must produce the same size.
#[cfg(feature = "svg")]
#[test]
fn svg_bounded_shrink_fits_like_shrink() {
    const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="60"></svg>"#;

    fn size(width: impl Into<Length>, height: impl Into<Length>) -> Rectangle {
        let handle = iced::widget::svg::Handle::from_memory(SVG);

        let view = container(iced::widget::svg(handle).width(width).height(height)).id("target");

        target_bounds(view)
    }

    let shrink = size(Shrink, 30);
    let bounded = size(Shrink.max(1000), 30);

    assert!(
        shrink.width > 0.0,
        "Shrink svg should fit its content width"
    );
    assert_eq!(bounded.width, shrink.width);
    assert_eq!(bounded.height, shrink.height);
}

/// An image `viewer` shrinks its non-constraining axis to the fitted image. A
/// bounded `Shrink.max(N)` width keeps that shrink-to-content behavior.
#[cfg(feature = "image")]
#[test]
fn image_viewer_bounded_shrink_fits() {
    let handle = iced::widget::image::Handle::from_rgba(100, 100, vec![0; 100 * 100 * 4]);

    let view = container(
        iced::widget::image::viewer(handle)
            .width(Shrink.max(300))
            .height(50),
    )
    .id("target");

    let bounds = target_bounds(view);

    assert_eq!(bounds.width, 50.0, "width shrinks to the fitted image");
    assert_eq!(bounds.height, 50.0);
}
