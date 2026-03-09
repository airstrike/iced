use iced::font;
use iced::widget::{Space, column, container, responsive, row, scrollable, slider, text};
use iced::{Element, Fill, Font, Task, color, padding};

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .window_size((1200.0, 700.0))
        .centered()
        .settings(iced::Settings {
            default_font: Font::with_name(FONT_NAME),
            default_text_size: 16.into(),
            ..Default::default()
        })
        .run()
}

const FONT_NAME: &str = "Roboto Flex";

#[derive(Debug, Clone)]
enum Message {
    FontLoaded,
    Wght(f32),
    Wdth(f32),
    Opsz(f32),
    Slnt(f32),
    Grad(f32),
    Xtra(f32),
    Xopq(f32),
    Yopq(f32),
    Ytlc(f32),
    Ytuc(f32),
    Ytas(f32),
    Ytde(f32),
    Ytfi(f32),
}

struct App {
    loaded: bool,
    wght: f32,
    wdth: f32,
    opsz: f32,
    slnt: f32,
    grad: f32,
    xtra: f32,
    xopq: f32,
    yopq: f32,
    ytlc: f32,
    ytuc: f32,
    ytas: f32,
    ytde: f32,
    ytfi: f32,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                loaded: false,
                wght: 400.0,
                wdth: 100.0,
                opsz: 149.0,
                slnt: 0.0,
                grad: 0.0,
                xtra: 468.0,
                xopq: 96.0,
                yopq: 79.0,
                ytlc: 514.0,
                ytuc: 712.0,
                ytas: 750.0,
                ytde: -203.0,
                ytfi: 738.0,
            },
            fetch_font(),
        )
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::custom(
            "Typography".to_string(),
            iced::theme::Palette {
                background: color!(0x1a1a1a),
                text: color!(0xe0e0e0),
                primary: color!(0x6db3f2),
                success: color!(0x22c55e),
                danger: color!(0xef4444),
                warning: color!(0xf59e0b),
            },
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FontLoaded => self.loaded = true,
            Message::Wght(v) => self.wght = v,
            Message::Wdth(v) => self.wdth = v,
            Message::Opsz(v) => {
                self.opsz = if v < 8.0 {
                    3.0
                } else if v > 144.0 {
                    149.0
                } else {
                    v
                };
            }
            Message::Slnt(v) => self.slnt = v,
            Message::Grad(v) => self.grad = v,
            Message::Xtra(v) => self.xtra = v,
            Message::Xopq(v) => self.xopq = v,
            Message::Yopq(v) => self.yopq = v,
            Message::Ytlc(v) => self.ytlc = v,
            Message::Ytuc(v) => self.ytuc = v,
            Message::Ytas(v) => self.ytas = v,
            Message::Ytde(v) => self.ytde = v,
            Message::Ytfi(v) => self.ytfi = v,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        if !self.loaded {
            return container(text("Loading Roboto Flex...").size(20))
                .center(Fill)
                .into();
        }

        let weight = to_weight(self.wght);
        let opsz = to_optical_size(self.opsz);
        let wght = self.wght;
        let vars = self.variations();

        row![
            container(scrollable(self.sidebar()).height(Fill))
                .width(240)
                .padding(padding::all(14).top(18))
                .style(theme::container::sidebar),
            scrollable(
                column![
                    hero(wght, opsz, &vars),
                    alphabet(wght, opsz, &vars),
                    weights(opsz, &vars),
                    big_words(opsz, &vars),
                    size_ramp(weight, opsz, &vars),
                    prose(opsz, &vars),
                ]
                .width(Fill),
            )
            .width(Fill)
            .height(Fill),
        ]
        .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        column![
            text("Variable Axes")
                .size(14)
                .font(roboto(font::Weight::Semibold, to_optical_size(self.opsz))),
            column![
                text("Registered").size(10).style(theme::text::dim),
                axis_slider("Weight", "wght", self.wght, 100.0..=1000.0, 1.0, Message::Wght),
                axis_slider("Width", "wdth", self.wdth, 25.0..=151.0, 0.1, Message::Wdth),
                opsz_slider(self.opsz),
                axis_slider("Slant", "slnt", self.slnt, -10.0..=0.0, 0.1, Message::Slnt),
            ]
            .spacing(6),
            column![
                text("Parametric").size(10).style(theme::text::dim),
                axis_slider("Grade", "GRAD", self.grad, -200.0..=150.0, 1.0, Message::Grad),
                axis_slider("Counter width", "XTRA", self.xtra, 323.0..=603.0, 1.0, Message::Xtra),
                axis_slider("Thick strokes", "XOPQ", self.xopq, 27.0..=175.0, 1.0, Message::Xopq),
                axis_slider("Thin strokes", "YOPQ", self.yopq, 25.0..=135.0, 1.0, Message::Yopq),
                axis_slider("Lowercase height", "YTLC", self.ytlc, 416.0..=570.0, 1.0, Message::Ytlc),
                axis_slider("Uppercase height", "YTUC", self.ytuc, 528.0..=760.0, 1.0, Message::Ytuc),
                axis_slider("Ascender height", "YTAS", self.ytas, 649.0..=854.0, 1.0, Message::Ytas),
                axis_slider("Descender depth", "YTDE", self.ytde, -305.0..=-98.0, 1.0, Message::Ytde),
                axis_slider("Figure height", "YTFI", self.ytfi, 560.0..=788.0, 1.0, Message::Ytfi),
            ]
            .spacing(6),
        ]
        .spacing(16)
        .into()
    }

    fn variations(&self) -> Vec<font::Variation> {
        vec![
            font::Variation::new(font::Tag::new(b"wdth"), self.wdth),
            font::Variation::new(font::Tag::new(b"slnt"), self.slnt),
            font::Variation::new(font::Tag::new(b"GRAD"), self.grad),
            font::Variation::new(font::Tag::new(b"XTRA"), self.xtra),
            font::Variation::new(font::Tag::new(b"XOPQ"), self.xopq),
            font::Variation::new(font::Tag::new(b"YOPQ"), self.yopq),
            font::Variation::new(font::Tag::new(b"YTLC"), self.ytlc),
            font::Variation::new(font::Tag::new(b"YTUC"), self.ytuc),
            font::Variation::new(font::Tag::new(b"YTAS"), self.ytas),
            font::Variation::new(font::Tag::new(b"YTDE"), self.ytde),
            font::Variation::new(font::Tag::new(b"YTFI"), self.ytfi),
        ]
    }
}

// ------ Helpers ------

fn roboto(weight: font::Weight, opsz: font::OpticalSize) -> Font {
    Font {
        weight,
        optical_size: opsz,
        ..Font::with_name(FONT_NAME)
    }
}

/// Map slider value to `OpticalSize`:
/// - 7 → disabled (`OpticalSize::None`)
/// - 8–144 → fixed (`OpticalSize::fixed(v)`)
/// - 145 → auto (`OpticalSize::Auto`)
fn to_optical_size(v: f32) -> font::OpticalSize {
    if v < 8.0 {
        font::OpticalSize::None
    } else if v > 144.0 {
        font::OpticalSize::Auto
    } else {
        font::OpticalSize::fixed(v)
    }
}

fn to_weight(v: f32) -> font::Weight {
    match v as u16 {
        0..=150 => font::Weight::Thin,
        151..=250 => font::Weight::ExtraLight,
        251..=350 => font::Weight::Light,
        351..=450 => font::Weight::Normal,
        451..=550 => font::Weight::Medium,
        551..=650 => font::Weight::Semibold,
        651..=750 => font::Weight::Bold,
        751..=850 => font::Weight::ExtraBold,
        _ => font::Weight::Black,
    }
}

fn axis_slider<'a>(
    name: &'a str,
    tag: &'a str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    on_change: fn(f32) -> Message,
) -> Element<'a, Message> {
    let fmt = if step < 1.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    };

    column![
        row![
            text(name).size(11),
            text(tag).size(9).style(theme::text::dim),
            Space::new().width(Fill),
            text(fmt).size(11).style(theme::text::dim),
        ]
        .spacing(4),
        slider(range, value, on_change).step(step),
    ]
    .spacing(2)
    .into()
}

fn opsz_slider(value: f32) -> Element<'static, Message> {
    let label = if value < 8.0 {
        "none".to_string()
    } else if value > 144.0 {
        "auto".to_string()
    } else {
        format!("{value:.1}")
    };

    column![
        row![
            text("opsz").size(11),
            Space::new().width(Fill),
            text(label).size(11).style(theme::text::dim),
        ],
        slider(3.0..=149.0, value, Message::Opsz).step(0.5),
    ]
    .spacing(2)
    .into()
}

// ------ Sections ------

fn hero(wght: f32, opsz: font::OpticalSize, vars: &[font::Variation]) -> Element<'static, Message> {
    let vars = vars.to_vec();
    let title_weight = to_weight(wght);
    let caption_weight = to_weight((wght - 200.0).max(100.0));
    container(responsive(move |size| {
        let title_size = (size.width * 0.14).max(48.0);

        column![
            text("Roboto\nFlex")
                .size(title_size)
                .line_height(1.0)
                .font(roboto(title_weight, opsz))
                .font_variations(vars.clone()),
            Space::new().height(16),
            text(
                "A variable font with 13 axes of customization. \
                 Weight, width, optical size, slant, grade, \
                 and seven parametric axes for fine-grained control.",
            )
            .size(18)
            .font(roboto(caption_weight, opsz))
            .font_variations(vars.clone())
            .style(theme::text::muted),
        ]
        .into()
    }))
    .padding(padding::all(48).top(40).bottom(24))
    .width(Fill)
    .into()
}

fn alphabet(
    wght: f32,
    opsz: font::OpticalSize,
    vars: &[font::Variation],
) -> Element<'static, Message> {
    let weight = to_weight(wght);
    let bold_weight = to_weight((wght + 200.0).min(900.0));
    let light_weight = to_weight((wght - 100.0).max(100.0));
    let vars = vars.to_vec();

    container(
        column![
            text("ABCDEFGHIJKLMNOPQRSTUVWXYZ")
                .size(36)
                .font(roboto(bold_weight, opsz))
                .font_variations(vars.clone()),
            text("abcdefghijklmnopqrstuvwxyz")
                .size(36)
                .font(roboto(weight, opsz))
                .font_variations(vars.clone()),
            text(
                "0123456789 !@#$%^&*() [{<>}] :;\u{2018}\u{2019}\u{201C}\u{201D} \u{00BF}\u{00A1}"
            )
            .size(28)
            .font(roboto(light_weight, opsz))
            .font_variations(vars)
            .style(theme::text::muted),
        ]
        .spacing(4),
    )
    .padding(padding::horizontal(48).bottom(48))
    .width(Fill)
    .into()
}

fn weights(opsz: font::OpticalSize, vars: &[font::Variation]) -> Element<'static, Message> {
    // Each weight gets a unique phrase, rendered large and responsive
    const RAMP: &[(font::Weight, &str, &str)] = &[
        (font::Weight::Thin, "Thin", "Crystalline morning light"),
        (
            font::Weight::ExtraLight,
            "ExtraLight",
            "Atmospheric phenomena",
        ),
        (font::Weight::Light, "Light", "Delicate instruments of time"),
        (font::Weight::Normal, "Regular", "The quick brown fox jumps"),
        (font::Weight::Medium, "Medium", "Architectural blueprints"),
        (font::Weight::Semibold, "SemiBold", "Precision engineering"),
        (font::Weight::Bold, "Bold", "New Zealand never floods"),
        (font::Weight::ExtraBold, "ExtraBold", "Horseshoe hamburger"),
        (font::Weight::Black, "Black", "MAXIMUM IMPACT"),
    ];

    let vars = vars.to_vec();
    container(responsive(move |size| {
        let sample_size = (size.width * 0.065).max(28.0);

        column(RAMP.iter().map(|(weight, label, sample)| {
            column![
                text(format!("{label}")).size(11).style(theme::text::dim),
                text(*sample)
                    .size(sample_size)
                    .font(roboto(*weight, opsz))
                    .font_variations(vars.clone()),
            ]
            .spacing(2)
            .into()
        }))
        .spacing(12)
        .into()
    }))
    .padding(padding::horizontal(48).bottom(48))
    .width(Fill)
    .into()
}

fn big_words(opsz: font::OpticalSize, vars: &[font::Variation]) -> Element<'static, Message> {
    let vars = vars.to_vec();
    container(responsive(move |size| {
        let huge = (size.width * 0.18).max(60.0);
        let large = (size.width * 0.12).max(48.0);

        column![
            text("Gravity")
                .size(huge)
                .line_height(1.0)
                .font(roboto(font::Weight::Thin, opsz))
                .font_variations(vars.clone()),
            text("Telescope")
                .size(large)
                .line_height(1.0)
                .font(roboto(font::Weight::Bold, opsz))
                .font_variations(vars.clone()),
            text("FJORD")
                .size(huge)
                .line_height(1.0)
                .font(roboto(font::Weight::Black, opsz))
                .font_variations(vars.clone()),
            text("equilibrium")
                .size(large)
                .line_height(1.0)
                .font(roboto(font::Weight::ExtraLight, opsz))
                .font_variations(vars.clone()),
            text("Quantum")
                .size(huge)
                .line_height(1.0)
                .font(roboto(font::Weight::Medium, opsz))
                .font_variations(vars.clone()),
        ]
        .spacing(8)
        .into()
    }))
    .padding(padding::horizontal(48).bottom(48))
    .width(Fill)
    .into()
}

fn size_ramp(
    weight: font::Weight,
    opsz: font::OpticalSize,
    vars: &[font::Variation],
) -> Element<'static, Message> {
    let f = roboto(weight, opsz);
    let v = vars.to_vec();

    container(
        column![
            text("Size Ramp").size(11).style(theme::text::dim),
            text("Horseshoe Hamburgertype New Zealand Never Floods")
                .size(8)
                .font(f)
                .font_variations(v.clone()),
            text("Horseshoe Hamburgertype New Zealand Never Floods")
                .size(10)
                .font(f)
                .font_variations(v.clone()),
            text("Horseshoe Hamburgertype New Zealand Never Floods")
                .size(12)
                .font(f)
                .font_variations(v.clone()),
            text("Horseshoe Hamburgertype New Zealand Never Floods")
                .size(14)
                .font(f)
                .font_variations(v.clone()),
            text("Horseshoe Hamburgertype New Zealand")
                .size(18)
                .font(f)
                .font_variations(v.clone()),
            text("Horseshoe Hamburgertype")
                .size(24)
                .font(f)
                .font_variations(v.clone()),
            text("Horseshoe Hamburgertype")
                .size(32)
                .font(f)
                .font_variations(v.clone()),
            text("Hamburgertype").size(48).font(f).font_variations(v),
        ]
        .spacing(4),
    )
    .padding(padding::horizontal(48).bottom(48))
    .width(Fill)
    .into()
}

fn prose(opsz: font::OpticalSize, vars: &[font::Variation]) -> Element<'static, Message> {
    let regular = roboto(font::Weight::Normal, opsz);
    let bold = roboto(font::Weight::Bold, opsz);
    let light = roboto(font::Weight::Light, opsz);
    let v = vars.to_vec();

    container(
        column![
            text("On the Phenomena of Variable Typography",)
                .size(28)
                .font(roboto(font::Weight::Semibold, opsz))
                .font_variations(v.clone()),
            row![
                column![
                    text(
                        "The history of typography is inextricable from the history of \
                         technology. Each advance in manufacturing and reproduction \
                         has brought new possibilities for the arrangement of text on \
                         a surface. From Gutenberg\u{2019}s movable type to phototypesetting, \
                         from the Macintosh to the modern web browser, the tools we use \
                         shape the letterforms we produce.",
                    )
                    .size(15)
                    .font(regular)
                    .font_variations(v.clone()),
                    text(
                        "Variable fonts represent the most significant \
                         evolution in digital typography since the introduction \
                         of OpenType.",
                    )
                    .size(15)
                    .font(bold)
                    .font_variations(v.clone()),
                    text(
                        "Rather than shipping separate font files for each weight, \
                         width, or optical size, a single variable font file contains \
                         the complete design space. The browser or application \
                         interpolates between designer-specified masters to produce \
                         any intermediate value along each axis of variation. This is \
                         not merely a technical convenience; it fundamentally changes \
                         the relationship between typographer and typeface.",
                    )
                    .size(15)
                    .font(regular)
                    .font_variations(v.clone()),
                ]
                .spacing(16)
                .width(Fill),
                column![
                    text(
                        "Consider the implications for responsive design. A heading \
                         set in weight 640 at one breakpoint might become weight 720 \
                         at another, maintaining optimal contrast against its background \
                         as the layout shifts. Optical size can track the rendered size \
                         of the text, ensuring that fine details are preserved at small \
                         sizes while clean, elegant forms emerge at display sizes.",
                    )
                    .size(15)
                    .font(regular)
                    .font_variations(v.clone()),
                    text(
                        "Roboto Flex takes this further than most variable fonts. Its \
                         thirteen axes of variation include not only the familiar \
                         weight, width, and optical size, but also parametric axes \
                         that control the thickness of vertical and horizontal strokes \
                         independently, the height of ascenders and descenders, and \
                         the proportions of uppercase and lowercase letters. These \
                         parametric axes give designers unprecedented control over \
                         the micro-typography of their text.",
                    )
                    .size(15)
                    .font(regular)
                    .font_variations(v.clone()),
                    text(
                        "The font was commissioned by Google and designed by Font \
                         Bureau as a demonstration of what is possible when a type \
                         family is conceived from the start as a variable font, \
                         rather than retrofitted from static sources.",
                    )
                    .size(15)
                    .font(light)
                    .font_variations(v)
                    .style(theme::text::muted),
                ]
                .spacing(16)
                .width(Fill),
            ]
            .spacing(32),
        ]
        .spacing(20),
    )
    .padding(padding::all(48).bottom(80))
    .width(Fill)
    .into()
}

// ------ Networking ------

fn fetch_font() -> Task<Message> {
    let url = "https://github.com/google/fonts/raw/main/ofl/robotoflex/RobotoFlex%5BGRAD%2CXOPQ%2CXTRA%2CYOPQ%2CYTAS%2CYTDE%2CYTFI%2CYTLC%2CYTUC%2Copsz%2Cslnt%2Cwdth%2Cwght%5D.ttf";

    Task::future(fetch_bytes(url)).then(|result| match result {
        Ok(bytes) => iced::font::load(bytes).map(|_| Message::FontLoaded),
        Err(e) => {
            eprintln!("Font download FAILED: {e}");
            Task::none()
        }
    })
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::get(url).await.map_err(|e| format!("{e}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let bytes = response.bytes().await.map_err(|e| format!("{e}"))?;
    Ok(bytes.to_vec())
}

// ------ Theme ------

mod theme {
    pub mod text {
        use iced::{color, widget};

        pub fn muted(_theme: &iced::Theme) -> widget::text::Style {
            widget::text::Style {
                color: Some(color!(0x888888)),
            }
        }

        pub fn dim(_theme: &iced::Theme) -> widget::text::Style {
            widget::text::Style {
                color: Some(color!(0x666666)),
            }
        }
    }

    pub mod container {
        use iced::{color, widget};

        pub fn sidebar(_theme: &iced::Theme) -> widget::container::Style {
            widget::container::Style {
                background: Some(color!(0x111111).into()),
                ..Default::default()
            }
        }
    }
}
