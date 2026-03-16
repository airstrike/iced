use iced::gradient;
use iced::theme;
use iced::widget::{checkbox, column, container, pick_list, row, slider, space, text};
use iced::{Center, Color, Element, Fill, Point, Radians, Theme, color};

pub fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application(Gradient::default, Gradient::update, Gradient::view)
        .style(Gradient::style)
        .transparent(true)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GradientType {
    Linear,
    Radial,
    Conic,
}

impl GradientType {
    const ALL: &'static [Self] = &[Self::Linear, Self::Radial, Self::Conic];
}

impl std::fmt::Display for GradientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GradientType::Linear => write!(f, "Linear"),
            GradientType::Radial => write!(f, "Radial"),
            GradientType::Conic => write!(f, "Conic"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Gradient {
    start: Color,
    end: Color,
    angle: Radians,
    radius: f32,
    center: Point,
    gradient_type: GradientType,
    transparent: bool,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    StartChanged(Color),
    EndChanged(Color),
    AngleChanged(Radians),
    RadiusChanged(f32),
    CenterXChanged(f32),
    CenterYChanged(f32),
    GradientTypeChanged(GradientType),
    TransparentToggled(bool),
}

impl Gradient {
    fn new() -> Self {
        Self {
            start: Color::WHITE,
            end: color!(0x0000ff),
            angle: Radians(0.0),
            radius: 1.0,
            center: Point::new(0.5, 0.5),
            gradient_type: GradientType::Linear,
            transparent: false,
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::StartChanged(color) => self.start = color,
            Message::EndChanged(color) => self.end = color,
            Message::AngleChanged(angle) => self.angle = angle,
            Message::RadiusChanged(radius) => self.radius = radius,
            Message::CenterXChanged(x) => self.center.x = x,
            Message::CenterYChanged(y) => self.center.y = y,
            Message::GradientTypeChanged(gradient_type) => self.gradient_type = gradient_type,
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
            center,
            gradient_type,
            transparent,
        } = *self;

        let gradient_box = container(space())
            .style(move |_theme| match gradient_type {
                GradientType::Linear => gradient::Linear::new(angle)
                    .add_stop(0.0, start)
                    .add_stop(1.0, end)
                    .into(),
                GradientType::Radial => {
                    let mut g = gradient::Radial::new();
                    g.center = center;
                    g.radius = radius;

                    g.add_stop(0.0, start).add_stop(1.0, end).into()
                }
                GradientType::Conic => {
                    let mut g = gradient::Conic::new();
                    g.center = center;
                    g.angle = angle;

                    g.add_stop(0.0, start).add_stop(1.0, end).into()
                }
            })
            .width(Fill)
            .height(Fill);

        let gradient_controls = match self.gradient_type {
            GradientType::Linear => column![
                row![
                    text("Angle").width(64),
                    slider(Radians::RANGE, self.angle, Message::AngleChanged).step(0.01)
                ]
                .spacing(8)
                .padding(8)
                .align_y(Center)
            ],
            GradientType::Radial => column![
                row![
                    text("Radius").width(64),
                    slider(0.01..=2.0, self.radius, Message::RadiusChanged).step(0.01)
                ]
                .spacing(8)
                .padding(8)
                .align_y(Center),
                row![
                    text("Center").width(64),
                    slider(-0.5..=1.5, self.center.x, Message::CenterXChanged).step(0.01),
                    slider(-0.5..=1.5, self.center.y, Message::CenterYChanged).step(0.01),
                ]
                .spacing(8)
                .padding(8)
                .align_y(Center),
            ],
            GradientType::Conic => column![
                row![
                    text("Angle").width(64),
                    slider(Radians::RANGE, self.angle, Message::AngleChanged).step(0.01)
                ]
                .spacing(8)
                .padding(8)
                .align_y(Center),
                row![
                    text("Center").width(64),
                    slider(-0.5..=1.5, self.center.x, Message::CenterXChanged).step(0.01),
                    slider(-0.5..=1.5, self.center.y, Message::CenterYChanged).step(0.01),
                ]
                .spacing(8)
                .padding(8)
                .align_y(Center),
            ],
        };

        let bottom_row = row![
            pick_list(
                Some(self.gradient_type),
                GradientType::ALL,
                GradientType::to_string,
            )
            .on_select(Message::GradientTypeChanged),
            space().width(Fill),
            checkbox(transparent)
                .label("Transparent window")
                .on_toggle(Message::TransparentToggled),
        ]
        .spacing(8)
        .padding(8)
        .align_y(Center);

        column![
            color_picker("Start", self.start).map(Message::StartChanged),
            color_picker("End", self.end).map(Message::EndChanged),
            gradient_controls,
            bottom_row,
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
