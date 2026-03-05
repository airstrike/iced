use iced::font;
use iced::widget::{
    Space, column, container, responsive, rich_text, row, rule, scrollable, span, text,
};
use iced::{Center, Color, Element, Fill, Font, Task, color, padding};

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .window_size((1200.0, 800.0))
        .settings(iced::Settings {
            default_font: Font::with_name(FONT_NAME),
            default_text_size: 16.into(),
            ..Default::default()
        })
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    FontLoaded,
}

struct App {
    loaded: bool,
}

const FONT_NAME: &str = "Inter Regular";
const LETTER_SPACING: f32 = -0.02;

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self { loaded: false },
            fetch_font(
                "https://raw.githubusercontent.com/google/fonts/main/ofl/inter/Inter%5Bopsz%2Cwght%5D.ttf",
                // "https://raw.githubusercontent.com/google/fonts/main/ofl/fraunces/Fraunces%5BSOFT%2CWONK%2Copsz%2Cwght%5D.ttf",
            ),
        )
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::custom(
            FONT_NAME.to_string(),
            iced::theme::Palette {
                background: color!(0x000000),
                text: color!(0xFFFFFF),
                primary: color!(0xFFFFFF),
                success: color!(0x22c55e),
                danger: color!(0xef4444),
                warning: color!(0xf59e0b),
            },
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FontLoaded => self.loaded = true,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        if !self.loaded {
            return container(text(format!("Loading {FONT_NAME}...")).size(20))
                .center(Fill)
                .into();
        }

        scrollable(
            column![
                hero(),
                description(),
                alphabet(),
                weights(),
                paragraphs(),
                example_text(),
                features(),
                feature_listing(),
            ]
            .width(Fill),
        )
        .into()
    }
}

// ------ Helpers ------

fn inter(weight: font::Weight) -> Font {
    Font {
        weight,
        ..Font::with_name(FONT_NAME)
    }
}

/// Builds a rich_text element from sample text with `§` toggle delimiters.
///
/// Text segments alternate between normal and highlighted on each `§`.
/// In the "Off" column, highlighted segments are shown in a muted color
/// to indicate which glyphs are affected by the feature.
/// In the "On" column, all text is rendered in the same dark color.
fn sample_text(marked: &str, tag: font::Tag, on: bool) -> Element<'_, Message> {
    use iced::widget::text::Span;

    let dark = color!(0x000000);
    let muted = Color { a: 0.3, ..dark };

    let feature = if on {
        font::Feature::on(tag)
    } else {
        font::Feature::off(tag)
    };

    let spans: Vec<Span<'_>> = marked
        .split('§')
        .enumerate()
        .map(|(i, segment)| {
            let c = if !on && i % 2 == 1 { muted } else { dark };
            span(segment).color(c).font_feature(feature)
        })
        .collect();

    rich_text(spans).size(30).into()
}

// ------ Sections ------

fn hero() -> Element<'static, Message> {
    container(responsive(|size| {
        // Scale hero text with window width (~10vw equivalent)
        let font_size = (size.width * 0.1).clamp(48.0, 140.0);

        text("The Inter\ntypeface family")
            .size(font_size)
            .font(inter(font::Weight::Bold))
            .letter_spacing(LETTER_SPACING)
            .into()
    }))
    .padding(padding::all(40).top(80))
    .width(Fill)
    .into()
}

fn description() -> Element<'static, Message> {
    container(
        row![
            text("The 21st century standard")
                .size(16)
                .font(inter(font::Weight::Semibold))
                .width(240),
            text(
                "Inter is a workhorse of a typeface carefully crafted & designed \
                 for a wide range of applications, from detailed user interfaces \
                 to marketing & signage. The Inter typeface family features over \
                 2000 glyphs covering 147 languages. Weights range from a \
                 delicate thin 100 all the way up to a heavy 900.",
            )
            .size(15)
            .style(theme::text::muted)
            .width(Fill),
            text(
                "Many OpenType features are provided as well, including \
                 contextual alternates which adjusts punctuation depending \
                 on the shape of surrounding glyphs, slashed zero for when \
                 you need to disambiguate \"0\" from \"o\", tabular numbers, \
                 and much more.",
            )
            .size(15)
            .style(theme::text::muted)
            .width(Fill),
        ]
        .spacing(40),
    )
    .padding(padding::horizontal(40).bottom(80))
    .width(Fill)
    .into()
}

fn alphabet() -> Element<'static, Message> {
    container(
        column![
            text("ABCDEFGHIJKLMNOPQRSTUVWXYZ")
                .size(80)
                .font(inter(font::Weight::Bold))
                .letter_spacing(LETTER_SPACING),
            text("abcdefghijklmnopqrstuvwxyz")
                .size(80)
                .font(inter(font::Weight::Bold))
                .letter_spacing(LETTER_SPACING),
            text("0123456789 &\u{2192}!")
                .size(80)
                .font(inter(font::Weight::Bold))
                .letter_spacing(LETTER_SPACING),
        ]
        .padding(padding::all(40).bottom(80)),
    )
    .width(Fill)
    .into()
}

fn weights() -> Element<'static, Message> {
    // (name, weight, value, sample, italic_sample, letter_spacing)
    const RAMP: &[(&str, font::Weight, u16, &str, &str, f32)] = &[
        (
            "Thin",
            font::Weight::Thin,
            100,
            "Salient gazelle eyes",
            "Inorganic compound",
            -0.01,
        ),
        (
            "ExtraLight",
            font::Weight::ExtraLight,
            200,
            "Internationalization",
            "Extravaganza Lime",
            -0.01,
        ),
        (
            "Light",
            font::Weight::Light,
            300,
            "Millimeter waves",
            "Rectangular ellipse",
            0.0,
        ),
        (
            "Regular",
            font::Weight::Normal,
            400,
            "Assimilation engine",
            "3 hours till midnight",
            0.0,
        ),
        (
            "Medium",
            font::Weight::Medium,
            500,
            "Botanica Francisco",
            "Artificial Intelligence",
            0.0,
        ),
        (
            "SemiBold",
            font::Weight::Semibold,
            600,
            "Spontaneous apes",
            "Sulfur hexafluoride",
            0.0,
        ),
        (
            "Bold",
            font::Weight::Bold,
            700,
            "15 Mango Avenue",
            "Hospital helicopter",
            0.0,
        ),
        (
            "ExtraBold",
            font::Weight::ExtraBold,
            800,
            "Comedy Morning",
            "Encyclopedia Abc",
            0.0,
        ),
        (
            "Black",
            font::Weight::Black,
            900,
            "Hamburgefonstiv",
            "United Martians",
            0.0,
        ),
    ];

    container(responsive(|size| {
        // Website uses font-size: 10vw for single-line weight samples
        let font_size = (size.width * 0.10).clamp(40.0, 120.0);

        let regular_samples = column(RAMP.iter().map(|(name, weight, value, sample, _, ls)| {
            weight_sample(name, *weight, *value, sample, false, *ls, font_size)
        }))
        .spacing(24);

        let italic_samples = column(RAMP.iter().map(|(name, weight, value, _, italic, ls)| {
            weight_sample(name, *weight, *value, italic, true, *ls, font_size)
        }))
        .spacing(24);

        column![regular_samples, italic_samples].spacing(60).into()
    }))
    .padding(padding::all(40).bottom(80))
    .width(Fill)
    .into()
}

fn weight_sample<'a>(
    name: &'a str,
    weight: font::Weight,
    value: u16,
    sample: &'a str,
    italic: bool,
    letter_spacing: f32,
    font_size: f32,
) -> Element<'a, Message> {
    let style_label = if italic {
        format!("{name} Italic")
    } else {
        name.to_string()
    };

    let f = Font {
        weight,
        style: if italic {
            font::Style::Italic
        } else {
            font::Style::Normal
        },
        ..Font::with_name(FONT_NAME)
    };

    let mut sample_text = text(sample).size(font_size).font(f);
    if letter_spacing != 0.0 {
        sample_text = sample_text.letter_spacing(letter_spacing);
    }

    column![
        row![
            text(style_label).size(13).style(theme::text::muted),
            Space::new().width(Fill),
            text(value.to_string()).size(13).style(theme::text::muted),
        ],
        sample_text,
    ]
    .into()
}

fn paragraphs() -> Element<'static, Message> {
    const PARAGRAPHS: &[(font::Weight, u16, &str)] = &[
        (
            font::Weight::Thin,
            100,
            "Chemistry is a physical science under natural sciences that covers the elements that make up matter",
        ),
        (
            font::Weight::ExtraLight,
            200,
            "The aspect ratio of an image is the ratio of its width to its height, but you already knew that",
        ),
        (
            font::Weight::Light,
            300,
            "Unlike a moka express, a napoletana does not use the pressure of steam to force the water through the coffee",
        ),
        (
            font::Weight::Normal,
            400,
            "The Berlin key, also known as Schlie\u{00DF}zwangschl\u{00FC}ssel, was not designed to make people laugh",
        ),
        (
            font::Weight::Medium,
            500,
            "Stanley Kubrick was an American film director, screenwriter, and producer of many films",
        ),
        (
            font::Weight::Semibold,
            600,
            "Jet Propulsion Laboratory,\nCalifornia Institute of Technology",
        ),
        (
            font::Weight::Bold,
            700,
            "The Sicilian Defense is a chess opening that begins with 1.e4 c5",
        ),
        (
            font::Weight::ExtraBold,
            800,
            "Padr\u{00F3}n province of A Coru\u{00F1}a, Galicia, northwestern Spain",
        ),
        (
            font::Weight::Black,
            900,
            "Woven silk pyjamas exchanged for blue quartz crystals",
        ),
    ];

    container(responsive(|size| {
        // Website uses font-size: 6vw for multi-line samples
        let font_size = (size.width * 0.06).clamp(24.0, 72.0);

        column(PARAGRAPHS.iter().map(|(weight, value, sample)| {
            column![
                text(value.to_string()).size(13).style(theme::text::muted),
                text(*sample).size(font_size).font(inter(*weight)),
            ]
            .spacing(4)
            .into()
        }))
        .spacing(20)
        .into()
    }))
    .padding(padding::all(40).bottom(80))
    .width(Fill)
    .into()
}

fn example_text() -> Element<'static, Message> {
    const PASSAGE_1: &str = "\
One of the most famous lighthouses of antiquity, as I have already \
pointed out, was the pharos of Alexandria, which ancient writers \
included among the Seven Wonders of the World. It might naturally be \
supposed that the founder of so remarkable a monument of \
architectural skill would be well known; yet while Strabo and Pliny, \
Eusebius, Suidas, and Lucian ascribe its erection to Ptolem\u{00E6}us \
Philadelphus, the wisest and most benevolent of the Ptolemean kings \
of Egypt, by Tzetzes and Ammianus Marcellinus the honour is given to \
Cleopatra;";

    const PASSAGE_1_ITALIC: &str = " and other authorities even attribute it to Alexander the \
Great.";

    const PASSAGE_2: &str = " From \u{03C6}\u{03B1}\u{03AF}\u{03BD}\u{03B5}\u{03B9}\u{03BD}, \
\u{201C}to shine,\u{201D} he says, comes \u{03C6}\u{03B1}\u{03BD}\u{03B5}\u{03C1}\u{03CC}\u{03C2}, \
and from \u{03C6}\u{03B1}\u{03BD}\u{03B5}\u{03C1}\u{03CC}\u{03C2}, \
\u{03C6}\u{03AC}\u{03C1}\u{03BF}\u{03C2}.... But the island \
was called Pharos seven or eight hundred years before it possessed \
either tower or beacon-light.";

    const PASSAGE_3: &str = "\
All that can with certainty be affirmed is, that the architect was named \
Sostrates. Montfaucon, in his great work, endeavours to explain how it \
is that while we are thus informed as to the architect, we are so \
doubtful as to the founder, whom, for his part, he believes to have \
been Ptolem\u{00E6}us. Our ignorance, he says, is owing to the knavery of \
Sostrates. He wished to immortalize his name; a blameless wish, if at \
the same time he had not sought to suppress that of the founder, \
whose glory it was to have suggested the erection. For this purpose \
Sostrates devised a stratagem which proved successful; deep in the \
wall of the tower he cut the following inscription: \u{201C}Sostrates of Cnidos, \
son of Dexiphanes, to the gods who Protect those who are upon the \
Sea.\u{201D} But, mistrustful that King Ptolem\u{00E6}us would scarcely be satisfied \
with an inscription in which he was wholly ignored, he covered it with \
a light coat of cement, which he knew would not long endure the \
action of the atmosphere, and carved thereon the name of \
Ptolem\u{00E6}us. After a few years the cement and the name of the king \
disappeared, and revealed the inscription which gave all the glory to \
Sostrates.";

    const PASSAGE_6: &str = "\
At a later date we find the word applied to very different objects, \
though always retaining the signification of light or brilliancy. A pharos \
of fire\u{2014}i.e., a ball or meteor\u{2014}was seen, says Gregory of Tours, to \
issue from the church of St. Hilaire, and descend upon King Clovis. \
The same historian uses the word to describe a conflagration:\u{2014}\u{201C}They \
(the barbarians) set fire to the church of St. Hilaire, kindled a great \
pharos, and while the church was burning, pillaged the monastery.\u{201D} \
The old French historian frequently employs the word in this sense, \
which leads us to suppose that in his time an incendiary was probably \
designated \u{201C}a maker of pharoses\u{201D} (un faiseur de phares). Still later, \
the term pharos was applied to certain machines in which a number of \
lamps or tapers were placed, as in a candelabrum.";

    const PASSAGE_4: &str = "\
Much etymological erudition has been expended on the derivation of \
the word Pharos. As far as the Alexandrian light-tower is concerned, \
there can be no doubt that it was named from the islet on which it \
stood; yet Isidore asserts that the word came from \u{03C6}\u{1FF6}\u{03C2}, \u{201C}light,\u{201D} and \
\u{1F44}\u{03C1}\u{1FB6}\u{03BD}, \u{201C}to see.\u{201D} To quote again from Montfaucon:";

    const PASSAGE_4_ITALIC: &str = " That numerous \
persons, who have not read the Greek authors, should exercise their \
ingenuity to no avail in the extraction of these etymologies, is far less \
surprising than that so good a scholar as Isaac Vossius should seek \
the origin of Pharos in the Greek language.";

    const PASSAGE_5: &str = "\
The most reasonable conjecture seems to be that the word is a \
Hellenic form of Phrah, the Egyptian name of the sun, to whom the \
Alexandrian lighthouse would naturally be compared by wondering \
spectators, or dedicated by a devout prince.";

    let italic = Font {
        style: font::Style::Italic,
        ..Font::with_name(FONT_NAME)
    };

    container(responsive(move |size| {
        use iced::widget::text::Span;

        let col1_spans: Vec<Span<'_>> = vec![
            span(PASSAGE_1),
            span(PASSAGE_1_ITALIC).font(italic),
            span(PASSAGE_2),
        ];

        let col2_spans: Vec<Span<'_>> = vec![span(PASSAGE_4), span(PASSAGE_4_ITALIC).font(italic)];

        let col1: Element<'_, Message> =
            column![rich_text(col1_spans).size(15), text(PASSAGE_3).size(15),]
                .spacing(24)
                .width(Fill)
                .into();

        let col2: Element<'_, Message> = column![
            rich_text(col2_spans).size(15),
            text(PASSAGE_5).size(15),
            text(PASSAGE_6).size(15),
        ]
        .spacing(24)
        .width(Fill)
        .into();

        let body: Element<'_, Message> = if size.width > 700.0 {
            row![col1, col2].spacing(40).into()
        } else {
            column![col1, col2].spacing(24).into()
        };

        column![
            text("Example text, Regular")
                .size(13)
                .style(theme::text::muted),
            body,
        ]
        .spacing(16)
        .into()
    }))
    .padding(padding::all(40).bottom(80))
    .width(Fill)
    .into()
}

fn features() -> Element<'static, Message> {
    container(
        column![
            // Header
            row![
                column![
                    text("Features")
                        .size(32)
                        .font(inter(font::Weight::Semibold))
                        .style(theme::text::dark),
                    text(
                        "Inter comes with many OpenType features which can be \
                         used to tailor functionality and aesthetics to your \
                         specific needs. Some of these features can be combined \
                         to form a great number of alternative variations.",
                    )
                    .size(16)
                    .style(theme::text::dark)
                    .width(480),
                ]
                .spacing(16),
                Space::new().width(Fill),
                text("altG16I")
                    .size(80)
                    .font(inter(font::Weight::Normal))
                    .font_feature(font::Feature::on(font::Tag(*b"cv01")))
                    .font_feature(font::Feature::on(font::Tag(*b"cv03")))
                    .font_feature(font::Feature::on(font::Tag(*b"cv04")))
                    .font_feature(font::Feature::on(font::Tag(*b"cv08")))
                    .font_feature(font::Feature::on(font::Tag(*b"cv10")))
                    .font_feature(font::Feature::on(font::Tag(*b"cv11")))
                    .font_feature(font::Feature::on(font::Tag(*b"ss01")))
                    .font_feature(font::Feature::on(font::Tag(*b"ss02")))
                    .font_feature(font::Feature::on(font::Tag(*b"dlig"))),
            ]
            .align_y(Center),
            // Thick divider
            rule::horizontal(2).style(theme::rule::black),
            // Column headers
            row![
                text("Feature")
                    .size(16)
                    .style(theme::text::dark)
                    .width(220),
                text("Off")
                    .size(16)
                    .style(theme::text::dark)
                    .width(Fill),
                text("On")
                    .size(16)
                    .style(theme::text::dark)
                    .width(Fill),
            ]
            .spacing(20),
            // Feature rows (matching rsms.me/inter)
            // § marks toggle muted/opaque in the Off column
            feature_row(
                "calt",
                "Contextual alternates",
                "Depending on the surrounding context, different \
                 glyphs are used. Enabled by default",
                "3§*§9 12§:§34 3§\u{2013}§8 §+§8+x\n\
                 §(§SEMI§)§PER§[§M§]§ANE§{§N§}§T\n\
                 -> --> ---> => ==> <->\n\
                 S§@§N s@n §:-)§ §\u{2022}§Smile",
                font::Tag(*b"calt"),
            ),
            feature_row(
                "dlig",
                "Discretionary Ligatures",
                "Disabled by default",
                "Dif§f§icult af§f§ine §f§jord\n\
                 after affine art interface\n\
                 ff ffi fft ft fi tt tf df dt ff kf kt rf\n\
                 rt vf vt wf wt yf yt §\u{00A1}\u{00BF}§What§?!§",
                font::Tag(*b"dlig"),
            ),
            feature_row(
                "tnum",
                "Tabular numbers",
                "Fixed-width numbers are useful for \
                 tabular data",
                "0.45, 0.91. +0.08\n\
                 1.00; 9.44, \u{2212}0.13\n\
                 0:00. 1.13; ~7.12",
                font::Tag(*b"tnum"),
            ),
            feature_row(
                "frac",
                "Fractions",
                "Convert spans of numbers & forward \
                 slash into fractions",
                "1/3  5/12  0123/456789\n\
                 Approximately 6/16\"",
                font::Tag(*b"frac"),
            ),
            feature_row(
                "case",
                "Case alternates",
                "Alternate glyphs that matches capital \
                 letters and numbers",
                "§(§Hello§)§ §[§World§]§ §{§9000§}§\n\
                 A§@§B  3 §+§ 9 §\u{2248}§ 12 §*§ 1 §\u{2192}§ X",
                font::Tag(*b"case"),
            ),
            feature_row(
                "ccmp",
                "Compositions",
                "Custom-made glyphs for compositions. \
                 Enabled by default",
                "§j\u{0303}§  §\u{00EC}§  §\u{012F}\u{0301}§  §\u{0135}§  §\u{012B}§\n\
                 Figure §A\u{20DD}§ §#\u{20DE}§ §3\u{20DD}§ §\u{00D7}\u{20DE}§",
                font::Tag(*b"ccmp"),
            ),
            feature_row(
                "sups",
                "Superscript",
                "",
                "ABC§123abc (+)\u{2212}[=]§",
                font::Tag(*b"sups"),
            ),
            feature_row(
                "subs",
                "Subscript",
                "",
                "ABC§123abc (+)\u{2212}[=]§",
                font::Tag(*b"subs"),
            ),
            feature_row(
                "sinf",
                "Scientific inferiors",
                "Same as Subscript",
                "H§2§O SF§6§ H§2§SO§4§",
                font::Tag(*b"sinf"),
            ),
            feature_row(
                "dnom",
                "Denominators",
                "",
                "ABC§1234567890§",
                font::Tag(*b"dnom"),
            ),
            feature_row(
                "numr",
                "Numerators",
                "",
                "ABC§1234567890§",
                font::Tag(*b"numr"),
            ),
            feature_row(
                "zero",
                "Slashed zero",
                "Disambiguate \"0\" from \"O\"",
                "§0§123",
                font::Tag(*b"zero"),
            ),
            feature_row(
                "ss01",
                "Alternate digits",
                "",
                "12§34§5§6§78§9§0",
                font::Tag(*b"ss01"),
            ),
            feature_row(
                "ss02",
                "Disambiguation",
                "Alternate glyph set that increases visual \
                 difference between similar-looking characters.",
                "WP§0§ACO9XS§I§1§0§12O9\n\
                 §Ill§ega§l§ busine§\u{00DF}§ \u{03B2}eta",
                font::Tag(*b"ss02"),
            ),
            feature_row(
                "ss07",
                "Square punctuation",
                "",
                "Hello§,§ M§\u{00E4}§stare§.!?§",
                font::Tag(*b"ss07"),
            ),
            feature_row(
                "ss08",
                "Square quotes",
                "",
                "I§'§m not, uhm §\"§smol§\"§",
                font::Tag(*b"ss08"),
            ),
            feature_row(
                "ss03",
                "Round quotes & comma",
                "",
                "I§'§m not§,§ uhm §\"§smol§\"§",
                font::Tag(*b"ss03"),
            ),
            feature_row(
                "ss05",
                "Characters in circles",
                "",
                "ABC123+\u{2192}",
                font::Tag(*b"ss05"),
            ),
            feature_row(
                "ss06",
                "Characters in squares",
                "",
                "ABC123+\u{2192}",
                font::Tag(*b"ss06"),
            ),
        ]
        .spacing(24)
        .padding(padding::all(40).bottom(80)),
    )
    .width(Fill)
    .style(theme::container::gold)
    .into()
}

fn feature_row<'a>(
    tag_label: &'a str,
    name: &'a str,
    desc: &'a str,
    sample: &'a str,
    tag: font::Tag,
) -> Element<'a, Message> {
    let mut info = column![
        row![
            container(
                text(tag_label)
                    .size(13)
                    .font(Font::MONOSPACE)
                    .style(theme::text::dark),
            )
            .padding([2, 6])
            .style(theme::container::tag),
            text(name)
                .size(16)
                .font(inter(font::Weight::Medium))
                .style(theme::text::dark),
        ]
        .spacing(8)
        .align_y(Center),
    ]
    .spacing(4)
    .width(220);

    if !desc.is_empty() {
        info = info.push(text(desc).size(12).style(theme::text::dark));
    }

    column![
        rule::horizontal(1).style(theme::rule::gold_light),
        row![
            info,
            container(sample_text(sample, tag, false)).width(Fill),
            container(sample_text(sample, tag, true)).width(Fill),
        ]
        .spacing(20),
    ]
    .spacing(16)
    .into()
}

fn feature_listing() -> Element<'static, Message> {
    const FEATURES: &[(&str, &str)] = &[
        ("aalt", "Access All Alternates"),
        ("c2sc", "Small Capitals From Capitals"),
        ("calt", "Contextual Alternates"),
        ("case", "Case-Sensitive Forms"),
        ("ccmp", "Glyph Composition/Decomposition"),
        ("cpsp", "Capital Spacing"),
        ("cv01", "Alternate one"),
        ("cv02", "Open four"),
        ("cv03", "Open six"),
        ("cv04", "Open nine"),
        ("cv05", "Lowercase L with tail"),
        ("cv06", "Simplified u"),
        ("cv07", "Alternate German double s"),
        ("cv08", "Upper-case i with serif"),
        ("cv09", "Flat-top three"),
        ("cv10", "Capital G with spur"),
        ("cv11", "Single-story a"),
        ("cv12", "Compact f"),
        ("cv13", "Compact t"),
        ("dlig", "Discretionary Ligatures"),
        ("dnom", "Denominators"),
        ("frac", "Fractions"),
        ("locl", "Localized Forms"),
        ("numr", "Numerators"),
        ("ordn", "Ordinals"),
        ("pnum", "Proportional Figures"),
        ("salt", "Stylistic Alternates"),
        ("sinf", "Scientific Inferiors"),
        ("ss01", "Open digits"),
        ("ss02", "Disambiguation (with zero)"),
        ("ss03", "Round quotes & commas"),
        ("ss04", "Disambiguation (no zero)"),
        ("ss05", "Circled characters"),
        ("ss06", "Squared characters"),
        ("ss07", "Square punctuation"),
        ("ss08", "Square quotes"),
        ("subs", "Subscript"),
        ("sups", "Superscript"),
        ("tnum", "Tabular Figures"),
        ("zero", "Slashed Zero"),
    ];

    fn feature_entry<'a>(tag: &'a str, name: &'a str) -> Element<'a, Message> {
        row![
            container(text(tag).size(13).font(Font::MONOSPACE),)
                .padding([2, 6])
                .style(theme::container::listing_tag),
            text(name).size(14),
        ]
        .spacing(8)
        .align_y(Center)
        .into()
    }

    // Split features into 3 roughly-equal columns
    let per_col = (FEATURES.len() + 2) / 3;
    let col1 = column(FEATURES[..per_col].iter().map(|(t, n)| feature_entry(t, n)))
        .spacing(6)
        .width(Fill);
    let col2 = column(
        FEATURES[per_col..per_col * 2]
            .iter()
            .map(|(t, n)| feature_entry(t, n)),
    )
    .spacing(6)
    .width(Fill);
    let col3 = column(
        FEATURES[per_col * 2..]
            .iter()
            .map(|(t, n)| feature_entry(t, n)),
    )
    .spacing(6)
    .width(Fill);

    container(
        column![
            text("Listing of all features")
                .size(16)
                .style(theme::text::muted),
            row![col1, col2, col3].spacing(20),
        ]
        .spacing(16)
        .padding(padding::all(40).bottom(80)),
    )
    .width(Fill)
    .into()
}

// ------ Networking ------

async fn fetch_bytes(url: &'static str) -> Result<Vec<u8>, String> {
    use http_body_util::{BodyExt, Empty};
    use hyper::body::Bytes;
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;

    let request = hyper::Request::get(url)
        .body(Empty::<Bytes>::new())
        .map_err(|e| format!("{e}"))?;
    let client = Client::builder(TokioExecutor::new()).build(hyper_tls::HttpsConnector::new());
    let response = client.request(request).await.map_err(|e| format!("{e}"))?;
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("{e}"))?;
    Ok(body.to_bytes().to_vec())
}

fn fetch_font(url: &'static str) -> Task<Message> {
    Task::future(fetch_bytes(url)).then(move |result| match result {
        Ok(bytes) => iced::font::load(bytes).map(|_| Message::FontLoaded),
        Err(_) => Task::none(),
    })
}

// ------ Theme ------

mod theme {
    use iced::{color, widget};

    pub mod text {
        use super::*;

        pub fn muted(_theme: &iced::Theme) -> widget::text::Style {
            widget::text::Style {
                color: Some(color!(0x888888)),
            }
        }

        pub fn dark(_theme: &iced::Theme) -> widget::text::Style {
            widget::text::Style {
                color: Some(color!(0x000000)),
            }
        }
    }

    pub mod container {
        use super::*;

        pub fn gold(_theme: &iced::Theme) -> widget::container::Style {
            widget::container::Style {
                background: Some(color!(0xFFE310).into()),
                ..Default::default()
            }
        }

        pub fn tag(_theme: &iced::Theme) -> widget::container::Style {
            widget::container::Style {
                background: Some(color!(0x000000).scale_alpha(0.08).into()),
                border: iced::border::rounded(3.0),
                ..Default::default()
            }
        }

        pub fn listing_tag(_theme: &iced::Theme) -> widget::container::Style {
            widget::container::Style {
                background: Some(color!(0xFFFFFF).scale_alpha(0.15).into()),
                border: iced::border::rounded(3.0),
                ..Default::default()
            }
        }
    }

    pub mod rule {
        use super::*;

        pub fn black(_theme: &iced::Theme) -> widget::rule::Style {
            widget::rule::Style {
                color: color!(0x000000),
                fill_mode: widget::rule::FillMode::Full,
                radius: 0.0.into(),
                snap: true,
            }
        }

        pub fn gold_light(_theme: &iced::Theme) -> widget::rule::Style {
            widget::rule::Style {
                color: color!(0x000011).scale_alpha(0.3),
                fill_mode: widget::rule::FillMode::Full,
                radius: 0.0.into(),
                snap: true,
            }
        }
    }
}
