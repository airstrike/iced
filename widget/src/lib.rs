//! Use the built-in widgets or create your own.
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/iced-rs/iced/9ab6923e943f784985e9ef9ca28b10278297225d/docs/logo.svg"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub use iced_renderer as renderer;
pub use iced_renderer::core;
pub use iced_renderer::graphics;

pub use core::widget::Id;

mod action;
mod column;
mod mouse_area;
mod pin;
mod responsive;
mod stack;
mod themer;

pub mod button;
pub mod checkbox;
pub mod combo_box;
pub mod container;
pub mod float;
pub mod grid;
pub mod keyed;
pub mod overlay;
pub mod pane_grid;
pub mod pick_list;
pub mod progress_bar;
pub mod radio;
pub mod row;
pub mod rule;
pub mod scrollable;
pub mod sensor;
pub mod slider;
pub mod space;
pub mod table;
pub mod text;
pub mod text_editor;
pub mod text_input;
pub mod toggler;
pub mod tooltip;
pub mod transition;
pub mod vertical_slider;

mod helpers;

pub use helpers::*;

#[cfg(feature = "lazy")]
mod lazy;

#[cfg(feature = "lazy")]
pub use crate::lazy::helpers::*;

#[doc(no_inline)]
pub use button::Button;
#[doc(no_inline)]
pub use checkbox::Checkbox;
#[doc(no_inline)]
pub use column::Column;
#[doc(no_inline)]
pub use combo_box::ComboBox;
#[doc(no_inline)]
pub use container::Container;
#[doc(no_inline)]
pub use float::Float;
#[doc(no_inline)]
pub use grid::Grid;
#[doc(no_inline)]
pub use mouse_area::MouseArea;
#[doc(no_inline)]
pub use pane_grid::PaneGrid;
#[doc(no_inline)]
pub use pick_list::PickList;
#[doc(no_inline)]
pub use pin::Pin;
#[doc(no_inline)]
pub use progress_bar::ProgressBar;
#[doc(no_inline)]
pub use radio::Radio;
#[doc(no_inline)]
pub use responsive::Responsive;
#[doc(no_inline)]
pub use row::Row;
#[doc(no_inline)]
pub use rule::Rule;
#[doc(no_inline)]
pub use scrollable::Scrollable;
#[doc(no_inline)]
pub use sensor::Sensor;
#[doc(no_inline)]
pub use slider::Slider;
#[doc(no_inline)]
pub use space::Space;
#[doc(no_inline)]
pub use stack::Stack;
#[doc(no_inline)]
pub use text::Text;
#[doc(no_inline)]
pub use text_editor::TextEditor;
#[doc(no_inline)]
pub use text_input::TextInput;
#[doc(no_inline)]
pub use themer::Themer;
#[doc(no_inline)]
pub use toggler::Toggler;
#[doc(no_inline)]
pub use tooltip::Tooltip;
#[doc(no_inline)]
pub use vertical_slider::VerticalSlider;

#[cfg(feature = "wgpu")]
pub mod shader;

#[cfg(feature = "wgpu")]
#[doc(no_inline)]
pub use shader::Shader;

#[cfg(feature = "svg")]
pub mod svg;

#[cfg(feature = "svg")]
#[doc(no_inline)]
pub use svg::Svg;

#[cfg(feature = "image")]
pub mod image;

#[cfg(feature = "image")]
#[doc(no_inline)]
pub use image::Image;

#[cfg(feature = "canvas")]
pub mod canvas;

#[cfg(feature = "canvas")]
#[doc(no_inline)]
pub use canvas::Canvas;

#[cfg(feature = "qr_code")]
pub mod qr_code;

#[cfg(feature = "qr_code")]
#[doc(no_inline)]
pub use qr_code::QRCode;

#[cfg(feature = "markdown")]
pub mod markdown;

pub use crate::core::theme::{self, Theme};
pub use action::Action;
pub use renderer::Renderer;

#[cfg(test)]
mod test {
    use crate::core::Length::{Fill, Shrink};
    use crate::core::layout;
    use crate::core::widget;
    use crate::core::{Element, Never, Pixels, Size, Theme};
    use crate::helpers::{container, scrollable, space};
    use crate::{column, row};

    const DEFAULT_LIMITS: layout::Limits = layout::Limits::new(
        Size::ZERO,
        Size {
            width: 1024.0,
            height: 768.0,
        },
    );

    #[test]
    fn layout_fill_max() {
        assert_layout_eq(
            column![
                space().height(30).width(Fill.max(300)),
                space().width(Fill.max(400))
            ],
            node(
                (0, 0),
                (400, 30),
                [node((0, 0), (300, 30), []), node((0, 30), (400, 0), [])],
            ),
        );
    }

    #[test]
    fn layout_fill_max_combined() {
        assert_layout_eq(
            row![
                space().height(10).width(Fill),
                space().height(20).width(Fill.max(300)),
                space().height(30).width(50),
            ],
            node(
                (0, 0),
                (1024, 30),
                [
                    node((0, 0), (1024 - 300 - 50, 10), []),
                    node((1024 - 300 - 50, 0), (300, 20), []),
                    node((1024 - 50, 0), (50, 30), []),
                ],
            ),
        );
    }

    #[test]
    fn layout_fill_max_nested() {
        assert_layout_eq(
            row![
                space().height(10).width(Fill),
                row![
                    space().height(20).width(Fill.max(300)),
                    space().height(30).width(50),
                ]
            ],
            node(
                (0, 0),
                (1024, 30),
                [
                    node((0, 0), (1024 - 300 - 50, 10), []),
                    node(
                        (1024 - 300 - 50, 0),
                        (300 + 50, 30),
                        [node((0, 0), (300, 20), []), node((300, 0), (50, 30), [])],
                    ),
                ],
            ),
        );
    }

    // Regression: a Shrink-cross column of all Fill-cross children must hand
    // the parent's cross max down rather than collapse them to 0.
    #[test]
    fn shrink_cross_column_with_fill_children_uses_parent_max() {
        assert_layout_eq(
            column![
                space().width(Fill).height(20),
                space().width(Fill).height(20)
            ]
            .width(Shrink),
            node(
                (0, 0),
                (1024, 40),
                [node((0, 0), (1024, 20), []), node((0, 20), (1024, 20), [])],
            ),
        );
    }

    // A fixed-cross sibling establishes the cross extent; Fill-cross siblings
    // conform to it instead of the parent's full max.
    #[test]
    fn fill_cross_child_conforms_to_fixed_cross_sibling() {
        assert_layout_eq(
            column![space().width(50).height(20), space().width(Fill).height(20)].width(Shrink),
            node(
                (0, 0),
                (50, 40),
                [node((0, 0), (50, 20), []), node((0, 20), (50, 20), [])],
            ),
        );
    }

    // A Shrink-cross column with only fixed-cross children sizes to the widest
    // child, not to the parent's cross max.
    #[test]
    fn shrink_cross_column_with_fixed_children_shrinks_to_widest() {
        assert_layout_eq(
            column![space().width(40).height(20), space().width(90).height(20)].width(Shrink),
            node(
                (0, 0),
                (90, 40),
                [node((0, 0), (40, 20), []), node((0, 20), (90, 20), [])],
            ),
        );
    }

    // A vertical scrollable stretches Fill content to the viewport width while
    // letting the scroll axis size to content.
    #[test]
    fn vertical_scrollable_stretches_fill_content_to_viewport_width() {
        assert_layout_eq(
            scrollable(
                column![
                    space().width(Fill).height(20),
                    space().width(Fill).height(20)
                ]
                .width(Fill),
            ),
            node(
                (0, 0),
                (1024, 40),
                [node(
                    (0, 0),
                    (1024, 40),
                    [node((0, 0), (1024, 20), []), node((0, 20), (1024, 20), [])],
                )],
            ),
        );
    }

    // A fixed child keeps its size inside a larger fixed parent; it is not
    // floored up to the parent's bounds.
    #[test]
    fn fixed_child_keeps_size_in_fixed_parent() {
        assert_layout_eq(
            container(space().width(60).height(10))
                .width(200)
                .height(50),
            node((0, 0), (200, 50), [node((0, 0), (60, 10), [])]),
        );
    }

    // `Shrink.min(N)` floors content smaller than `N` up to `N`.
    #[test]
    fn min_floors_shrink_content() {
        assert_layout_eq(
            container(space().width(24).height(10)).width(Shrink.min(200)),
            node((0, 0), (200, 10), [node((0, 0), (24, 10), [])]),
        );
    }

    // `Shrink.min(lo).max(hi)` clamps content into `[lo, hi]`.
    #[test]
    fn bounded_clamps_shrink_content() {
        let bounded = |intrinsic: u32| {
            container(space().width(intrinsic).height(10)).width(Shrink.min(100).max(300))
        };

        assert_layout_eq(
            bounded(40),
            node((0, 0), (100, 10), [node((0, 0), (40, 10), [])]),
        );
        assert_layout_eq(
            bounded(180),
            node((0, 0), (180, 10), [node((0, 0), (180, 10), [])]),
        );
        assert_layout_eq(
            bounded(500),
            node((0, 0), (300, 10), [node((0, 0), (300, 10), [])]),
        );
    }

    // A max bound caps Fill content on a scrollable's cross axis.
    #[test]
    fn scrollable_caps_fill_max_on_cross_axis() {
        assert_layout_eq(
            scrollable(container(space().width(800).height(20)).width(Fill.max(300))),
            node(
                (0, 0),
                (300, 20),
                [node((0, 0), (300, 20), [node((0, 0), (300, 20), [])])],
            ),
        );
    }

    // A max bound caps content on the scroll axis even though that axis is
    // otherwise unbounded.
    #[test]
    fn scrollable_caps_shrink_max_on_scroll_axis() {
        assert_layout_eq(
            scrollable(container(space().width(50).height(500)).height(Shrink.max(300))),
            node(
                (0, 0),
                (50, 300),
                [node((0, 0), (50, 300), [node((0, 0), (50, 300), [])])],
            ),
        );
    }

    // A min bound floors content on the scroll axis; the shorter child keeps
    // its size inside the floored parent.
    #[test]
    fn scrollable_floors_shrink_min_on_scroll_axis() {
        assert_layout_eq(
            scrollable(container(space().width(50).height(100)).height(Shrink.min(400))),
            node(
                (0, 0),
                (50, 400),
                [node((0, 0), (50, 400), [node((0, 0), (50, 100), [])])],
            ),
        );
    }

    fn assert_layout_eq<'a>(
        element: impl Into<Element<'a, Never, Theme, ()>>,
        expect: layout::Node,
    ) {
        let mut element = element.into();

        let mut tree = widget::Tree::new(&element);
        element.as_widget_mut().diff(&mut tree);

        let layout = element
            .as_widget_mut()
            .layout(&mut tree, &(), &DEFAULT_LIMITS);

        assert_eq!(layout, expect);
    }

    fn node(
        (x, y): (impl Into<Pixels>, impl Into<Pixels>),
        (width, height): (impl Into<Pixels>, impl Into<Pixels>),
        children: impl IntoIterator<Item = layout::Node>,
    ) -> layout::Node {
        let x = x.into().0;
        let y = y.into().0;
        let width = width.into().0;
        let height = height.into().0;

        layout::Node::with_children(Size { width, height }, children.into_iter().collect())
            .move_to((x, y))
    }
}
