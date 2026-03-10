use iced::widget::{column, pick_list, text};
use iced::{Center, Element, Fill, Task, padding};

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .window_size((400.0, 600.0))
        .run()
}

struct App {
    families: Vec<String>,
    selected: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    FontsLoaded(Vec<String>),
    FontSelected(String),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                families: Vec::new(),
                selected: None,
            },
            iced::font::families().map(Message::FontsLoaded),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::FontsLoaded(families) => {
                self.families = families;
            }
            Message::FontSelected(name) => {
                self.selected = Some(name);
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let picker = pick_list(
            self.selected.clone(),
            self.families.as_slice(),
            String::to_string,
        )
        .on_select(Message::FontSelected)
        .placeholder("Choose a font…");

        let preview = text("iced").size(120).font(self.selected.as_deref());

        column![preview, picker]
            .spacing(20)
            .padding(padding::top(20))
            .width(Fill)
            .align_x(Center)
            .into()
    }
}
