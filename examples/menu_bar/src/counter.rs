use iced::keyboard::Modifiers;
use iced::widget::{button, checkbox, column, row, text, text_editor, text_input};
use iced::{Fill, Widget, menu};

pub struct State {
    pub name: String,
    value: i64,
    step: Step,
    allow_negative: bool,
    notes: text_editor::Content,
}

#[derive(Debug, Clone)]
pub enum Message {
    Increment,
    Decrement,
    Reset,
    Step(Step),
    AllowNegative(bool),
    Name(String),
    Edit(text_editor::Action),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    One,
    Five,
    Ten,
}

impl Step {
    fn value(self) -> i64 {
        match self {
            Self::One => 1,
            Self::Five => 5,
            Self::Ten => 10,
        }
    }
}

impl State {
    pub fn new(number: usize) -> Self {
        Self {
            name: format!("Counter {number}"),
            value: if number == 1 { 0 } else { 10 },
            step: if number == 1 { Step::One } else { Step::Five },
            allow_negative: false,
            notes: text_editor::Content::with_text(
                "Select some text here, then use the native Edit menu.\nEach window has its own counter and notes.",
            ),
        }
    }

    fn can_increment(&self) -> bool {
        self.value.checked_add(self.step.value()).is_some()
    }

    fn can_decrement(&self) -> bool {
        self.value
            .checked_sub(self.step.value())
            .is_some_and(|value| self.allow_negative || value >= 0)
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Increment if self.can_increment() => self.value += self.step.value(),
            Message::Decrement if self.can_decrement() => self.value -= self.step.value(),
            Message::Increment | Message::Decrement => {}
            Message::Reset => self.value = 0,
            Message::Step(step) => self.step = step,
            Message::AllowNegative(allow) => {
                self.allow_negative = allow;

                if !allow {
                    self.value = self.value.max(0);
                }
            }
            Message::Name(name) => self.name = name,
            Message::Edit(action) => self.notes.perform(action),
        }
    }

    pub fn view(&self) -> impl Widget<Message> {
        column![
            text(&self.name).size(24),
            text(self.value).size(50),
            row![
                button(text(format!("−{}", self.step.value())))
                    .on_press_maybe(self.can_decrement().then_some(Message::Decrement)),
                button("Reset").on_press_maybe((self.value != 0).then_some(Message::Reset)),
                button(text(format!("+{}", self.step.value())))
                    .on_press_maybe(self.can_increment().then_some(Message::Increment)),
            ]
            .spacing(8),
            checkbox(self.allow_negative)
                .label("Allow negative values")
                .on_toggle(Message::AllowNegative),
            text("Window name"),
            text_input("Counter name", &self.name).on_input(Message::Name),
            text("Notes"),
            text_editor(&self.notes)
                .on_action(Message::Edit)
                .height(Fill),
            text("Switch windows to change the native menu.").size(13),
        ]
        .padding(24)
        .spacing(12)
        .height(Fill)
    }

    pub fn menu(&self) -> menu::Menu<Message> {
        let steps = [Step::One, Step::Five, Step::Ten].into_iter().map(|step| {
            menu::item(step.value().to_string())
                .checked(self.step == step)
                .on_press(Message::Step(step))
        });

        menu![
            &self.name,
            menu::item(format!("Increase by {}", self.step.value()))
                .on_press_maybe(self.can_increment().then_some(Message::Increment))
                .shortcut(Modifiers::COMMAND, 'i'),
            menu::item(format!("Decrease by {}", self.step.value()))
                .on_press_maybe(self.can_decrement().then_some(Message::Decrement))
                .shortcut(Modifiers::COMMAND, 'd'),
            menu::item("Reset")
                .on_press_maybe((self.value != 0).then_some(Message::Reset))
                .shortcut(Modifiers::COMMAND, 'r'),
            menu::separator(),
            menu::item("Allow negative values")
                .checked(self.allow_negative)
                .on_press(Message::AllowNegative(!self.allow_negative)),
            menu("Step", steps),
            menu::separator(),
            menu::item(format!("Current value: {}", self.value)),
        ]
    }
}
