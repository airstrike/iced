use iced::widget::anchored::{Align, Side};
use iced::widget::{anchored, button, center, column, container, mouse_area, row, space, text};
use iced::{Center, Element, Fill, Point, Theme, Widget, border};

use std::time::Duration;

pub fn main() -> iced::Result {
    iced::application(Anchored::default, Anchored::update, Anchored::view)
        .title("Anchored")
        .run()
}

#[derive(Default)]
struct Anchored {
    side: Side,
    align: Align,
    open: Option<Popover>,
    cursor: Point,
    menu_at: Option<Point>,
    last: Option<&'static str>,
}

/// Which popover is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Popover {
    Center,
    Corner(usize),
}

#[derive(Debug, Clone)]
enum Message {
    Side(Side),
    Align(Align),
    Toggle(Popover, bool),
    Moved(Point),
    OpenMenu,
    CloseMenu,
    Pick(&'static str),
}

impl Anchored {
    fn update(&mut self, message: Message) {
        match message {
            Message::Side(side) => self.side = side,
            Message::Align(align) => self.align = align,
            Message::Toggle(popover, open) => {
                if open {
                    self.open = Some(popover);
                } else if self.open == Some(popover) {
                    self.open = None;
                }
            }
            Message::Moved(cursor) => self.cursor = cursor,
            Message::OpenMenu => self.menu_at = Some(self.cursor),
            Message::CloseMenu => self.menu_at = None,
            Message::Pick(item) => {
                self.last = Some(item);
                self.menu_at = None;
            }
        }
    }

    fn view(&self) -> impl Widget<Message> + '_ {
        let sides = row([Side::Top, Side::Right, Side::Bottom, Side::Left]
            .map(|side| choice(format!("{side:?}"), self.side == side, Message::Side(side))))
        .spacing(4);

        let aligns = row([Align::Start, Align::Center, Align::End].map(|align| {
            choice(
                format!("{align:?}"),
                self.align == align,
                Message::Align(align),
            )
        }))
        .spacing(4);

        let popover = self.popover(
            Popover::Center,
            "Open popover",
            column![
                text("Dimensions").size(14),
                text("Set the dimensions for the layer.").size(12),
            ]
            .spacing(4),
        );

        let hover_card = anchored(
            text("@iced").size(14),
            panel(
                column![
                    text("@iced").size(14),
                    text("A cross-platform GUI library for Rust.").size(12),
                ]
                .spacing(4)
                .width(220),
            ),
        )
        .side(Side::Bottom)
        .offset(10)
        .hover(Duration::from_millis(300), Duration::from_millis(100))
        .arrow([10.0, 5.0], |theme: &Theme| {
            theme.palette().background.strong.color
        });

        let menu_area = mouse_area(
            container(text(match self.last {
                Some(item) => format!("Right click here (last pick: {item})"),
                None => "Right click here".to_owned(),
            }))
            .center(Fill)
            .height(120)
            .style(container::bordered_box),
        )
        .on_move(Message::Moved)
        .on_right_press(Message::OpenMenu);

        let menu = column(["Back", "Forward", "Reload", "Save As..."].map(|item| {
            button(text(item).size(14))
                .width(Fill)
                .style(button::text)
                .on_press(Message::Pick(item))
                .boxed()
        }))
        .width(160);

        let context_menu = anchored(menu_area, panel(menu))
            .at(self.menu_at.unwrap_or(Point::ORIGIN))
            .align(Align::Start)
            .open(self.menu_at.is_some())
            .modal(true)
            .on_toggle(|open| {
                if open {
                    Message::OpenMenu
                } else {
                    Message::CloseMenu
                }
            });

        let corners: Vec<Element<'_, Message>> = (0..4)
            .map(|corner| {
                self.popover(
                    Popover::Corner(corner),
                    "Edge",
                    text("Flips and shifts to stay in the window.").size(12),
                )
                .boxed()
            })
            .collect();
        let mut corners = corners.into_iter();

        let first = row![corners.next().expect("four corners"), space::horizontal()];
        let last = row![space::horizontal(), corners.next().expect("four corners")];
        let bottom = row![
            corners.next().expect("four corners"),
            space::horizontal(),
            corners.next().expect("four corners"),
        ];

        column![
            row![first, last],
            center(
                column![
                    sides,
                    aligns,
                    popover,
                    space().height(16),
                    row![text("Hover").size(14), hover_card].spacing(6),
                    space().height(16),
                    context_menu,
                ]
                .spacing(12)
                .align_x(Center)
                .width(480),
            ),
            bottom,
        ]
        .padding(8)
    }

    fn popover<'a>(
        &'a self,
        popover: Popover,
        label: &'a str,
        content: impl Widget<Message> + 'a,
    ) -> impl Widget<Message> + 'a {
        let is_open = self.open == Some(popover);

        anchored(
            button(text(label).size(14)).on_press(Message::Toggle(popover, !is_open)),
            panel(content),
        )
        .side(self.side)
        .align(self.align)
        .offset(4)
        .padding(8)
        .open(is_open)
        .on_toggle(move |open| Message::Toggle(popover, open))
    }
}

fn choice<'a>(label: String, selected: bool, message: Message) -> Element<'a, Message> {
    button(text(label).size(12))
        .style(if selected {
            button::primary
        } else {
            button::secondary
        })
        .on_press(message)
        .boxed()
}

fn panel<'a>(content: impl Widget<Message> + 'a) -> impl Widget<Message> + 'a {
    container(content).padding(10).style(|theme: &Theme| {
        let palette = theme.palette();

        container::Style {
            background: Some(palette.background.base.color.into()),
            text_color: Some(palette.background.base.text),
            border: border::rounded(8)
                .width(1)
                .color(palette.background.strong.color),
            shadow: iced::Shadow {
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.15),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..container::Style::default()
        }
    })
}
