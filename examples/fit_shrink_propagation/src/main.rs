//! `Fit` absorbs `Fill`; `Shrink` does not.
//!
//! A `.width(Fit)` container or flex computes its own width as
//! `Fit.stack/cross(child)`, and `Fill` wins that merge -- so a `Fit` box with a
//! `Fill` child stops being `Fit` and *becomes* `Fill`: it fills its slot. A
//! `Shrink` box keeps its own width and compresses the `Fill` child to its
//! content instead.
//!
//! Each panel wraps Alice, Bo and Cassidy (deliberately different lengths)
//! several ways. A `Fit` box hugs when every child is `Fit`, but fills the
//! moment any child is `Fill` -- absorbed, because `Fill` wins the merge. A
//! `Shrink` box never absorbs: it compresses `Fill` children to content -- to a
//! `Fit` sibling's width if one is present, otherwise to the widest child's own
//! content (so all-`Fill` siblings settle on the widest, not the offered max).
//!
//! The left panel lays the box out as a `row` (width is the main axis); the
//! right as a `column` (width is the cross axis). It holds on both. Border
//! color: green = `Fit`, gray = `Fill`, red = `Shrink`. No fixed widths
//! anywhere: the window is the only space.
use iced::widget::{Column, Row, center, container, scrollable, text};
use iced::{Center, Element, Fill, Fit, Length, Shrink};

pub fn main() -> iced::Result {
    iced::application(|| (), update, view)
        .theme(theme::custom)
        .run()
}

pub type Message = ();

fn update(_state: &mut (), _message: Message) {}

fn view(_state: &()) -> Element<'_, Message> {
    let panels = iced::widget::row![
        panel("Row (main axis)", Axis::Row),
        panel("Column (cross axis)", Axis::Column),
    ]
    .spacing(40)
    .width(Fill)
    .height(Fill);

    container(
        scrollable(
            center(
                iced::widget::column![panels, legend()]
                    .spacing(20)
                    .width(Fill)
                    .height(Fill),
            )
            .padding(20),
        )
        .spacing(0)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::default().scroller_width(2).width(1),
        )),
    )
    .padding(2)
    .into()
}

/// Which axis the box lays its children along.
#[derive(Clone, Copy)]
enum Axis {
    Row,
    Column,
}

/// One panel: the same three boxes wrapped three ways, on the given axis.
fn panel(title: &str, axis: Axis) -> Element<'_, Message> {
    iced::widget::column![
        text(title).size(18),
        case(
            "Fit box, fit children",
            flex_box(axis, Fit, [Fit, Fit, Fit])
        ),
        case(
            "Fit box, fill children",
            flex_box(axis, Fit, [Fill, Fill, Fill])
        ),
        case("Fit box, Fill Alice", flex_box(axis, Fit, [Fill, Fit, Fit])),
        case(
            "Shrink box, Fill Alice",
            flex_box(axis, Shrink, [Fill, Fit, Fit])
        ),
        case(
            "Shrink box, fill children",
            flex_box(axis, Shrink, [Fill, Fill, Fill])
        ),
    ]
    .spacing(14)
    .width(Fill)
    .into()
}

/// A caption above its subject.
fn case<'a>(caption: &'a str, subject: Element<'a, Message>) -> Element<'a, Message> {
    iced::widget::column![text(caption).size(12), subject]
        .spacing(4)
        .width(Fill)
        .into()
}

/// A `box_width` flex on the given `axis`, holding Alice, Bo and Cassidy with
/// the given per-child widths. The flex resolves its own width as
/// `box_width.stack/cross(children)`: a `Fit` box yields to any `Fill` child
/// (absorbed), a `Shrink` box overrides them.
fn flex_box(axis: Axis, box_width: Length, children: [Length; 3]) -> Element<'static, Message> {
    let [alice, bo, cassidy] = children;
    let kids = vec![
        leaf("Alice", alice),
        leaf("Bo", bo),
        leaf("Cassidy", cassidy),
    ];

    let inner: Element<'static, Message> = match axis {
        Axis::Row => Row::with_children(kids).width(box_width).spacing(8).into(),
        Axis::Column => Column::with_children(kids)
            .width(box_width)
            .spacing(8)
            .into(),
    };

    bordered(inner, box_width)
}

/// A named leaf box. Its width strategy is shown only by the border color, so
/// the text is free to be a plain identity (`Alice`, `Bo`, `Cassidy`).
fn leaf(name: &'static str, width: Length) -> Element<'static, Message> {
    bordered(iced::widget::row![text(name)].width(width), width)
}

/// A styling-only container: paints the border for `width`'s strategy and pads.
/// Its own size is left at the `Fit` default, so it hugs (absorbs) its child.
fn bordered<'a>(content: impl Into<Element<'a, Message>>, width: Length) -> Element<'a, Message> {
    container(content)
        .padding(8)
        .style(theme::layer(theme::sizing(width)))
        .into()
}

/// Maps each border color to the width strategy it stands for.
fn legend() -> Element<'static, Message> {
    iced::widget::row![
        text("legend:"),
        swatch("Fit", Fit),
        swatch("Fill", Fill),
        swatch("Shrink", Shrink),
    ]
    .spacing(10)
    .align_y(Center)
    .into()
}

fn swatch(label: &'static str, width: Length) -> Element<'static, Message> {
    bordered(text(label), width)
}

mod theme {
    use iced::theme::palette::Seed;
    use iced::widget::container;
    use iced::{Border, Length, Theme};

    /// A bold Bauhaus seed (near-black ground, primary triad). Drives every
    /// color the `layer` styles read, so they come out solid and distinct
    /// rather than a wash of stacked red.
    pub fn custom(_: &()) -> iced::Theme {
        iced::Theme::custom(
            "Bauhaus",
            Seed {
                background: iced::color!(0x1a1a1d),
                text: iced::color!(0xececE4),
                primary: iced::color!(0x2457a4),
                success: iced::color!(0x2f8f70),
                warning: iced::color!(0xedc22b),
                danger: iced::color!(0xc4302b),
            },
        )
    }

    /// A layer's width strategy, encoded as the border stroke.
    #[derive(Clone, Copy)]
    pub enum Sizing {
        Fill,
        Fit,
        Shrink,
    }

    /// Buckets a width [`Length`] into a [`Sizing`].
    pub fn sizing(width: Length) -> Sizing {
        match width {
            Length::Fill | Length::FillPortion(_) => Sizing::Fill,
            Length::Fit => Sizing::Fit,
            _ => Sizing::Shrink,
        }
    }

    /// Border stroke = width strategy. All colors from the theme palette.
    pub fn layer(sizing: Sizing) -> impl Fn(&Theme) -> container::Style {
        move |theme| {
            let palette = theme.palette();

            let border = match sizing {
                Sizing::Fill => palette.background.strong.color,
                Sizing::Fit => palette.success.strong.color,
                Sizing::Shrink => palette.danger.strong.color,
            };

            container::Style {
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            }
        }
    }
}
