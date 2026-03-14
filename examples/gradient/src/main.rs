use iced::gradient;
use iced::theme;
use iced::widget::{checkbox, column, container, row, slider, space, text, toggler};
use iced::{Center, Color, Element, Fill, Radians, Theme, color};

pub fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application(Gradient::default, Gradient::update, Gradient::view)
        .style(Gradient::style)
        .transparent(true)
        .run()
}

#[derive(Debug, Clone, Copy)]
struct Gradient {
    start: Color,
    end: Color,
    angle: Radians,
    radius: f32,
    radial: bool,
    transparent: bool,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    StartChanged(Color),
    EndChanged(Color),
    AngleChanged(Radians),
    RadiusChanged(f32),
    RadialToggled(bool),
    TransparentToggled(bool),
}

impl Gradient {
    fn new() -> Self {
        Self {
            start: Color::WHITE,
            end: color!(0x0000ff),
            angle: Radians(0.0),
            radius: 1.0,
            radial: false,
            transparent: false,
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::StartChanged(color) => self.start = color,
            Message::EndChanged(color) => self.end = color,
            Message::AngleChanged(angle) => self.angle = angle,
            Message::RadiusChanged(radius) => self.radius = radius,
            Message::RadialToggled(radial) => self.radial = radial,
            Message::TransparentToggled(transparent) => {
                self.transparent = transparent;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let Self {
            start,
            end,
            angle,
            radius,
            radial,
            transparent,
        } = *self;

        let gradient_box = container(space())
            .style(move |_theme| {
                if radial {
                    let mut g = gradient::Radial::new();
                    g.radius = radius;

                    g.add_stop(0.0, start).add_stop(1.0, end).into()
                } else {
                    gradient::Linear::new(angle)
                        .add_stop(0.0, start)
                        .add_stop(1.0, end)
                        .into()
                }
            })
            .width(Fill)
            .height(Fill);

        let gradient_control = if self.radial {
            row![
                text("Radius").width(64),
                slider(0.01..=2.0, self.radius, Message::RadiusChanged).step(0.01)
            ]
        } else {
            row![
                text("Angle").width(64),
                slider(Radians::RANGE, self.angle, Message::AngleChanged).step(0.01)
            ]
        }
        .spacing(8)
        .padding(8)
        .align_y(Center);

        let radial_toggle = container(
            toggler(self.radial)
                .label("Radial")
                .on_toggle(Message::RadialToggled),
        )
        .padding(8);

        let transparency_toggle = container(
            checkbox(transparent)
                .label("Transparent window")
                .on_toggle(Message::TransparentToggled),
        )
        .padding(8);

        column![
            color_picker("Start", self.start).map(Message::StartChanged),
            color_picker("End", self.end).map(Message::EndChanged),
            gradient_control,
            radial_toggle,
            transparency_toggle,
            gradient_box,
        ]
        .into()
    }

    fn style(&self, theme: &Theme) -> theme::Style {
        if self.transparent {
            theme::Style {
                background_color: Color::TRANSPARENT,
                text_color: theme.seed().text,
            }
        } else {
            theme::default(theme)
        }
    }
}

impl Default for Gradient {
    fn default() -> Self {
        Self::new()
    }
}

fn color_picker(label: &str, color: Color) -> Element<'_, Color> {
    row![
        text(label).width(64),
        slider(0.0..=1.0, color.r, move |r| { Color { r, ..color } }).step(0.01),
        slider(0.0..=1.0, color.g, move |g| { Color { g, ..color } }).step(0.01),
        slider(0.0..=1.0, color.b, move |b| { Color { b, ..color } }).step(0.01),
        slider(0.0..=1.0, color.a, move |a| { Color { a, ..color } }).step(0.01),
    ]
    .spacing(8)
    .padding(8)
    .align_y(Center)
    .into()
}
