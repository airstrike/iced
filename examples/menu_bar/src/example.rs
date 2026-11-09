use std::collections::BTreeMap;

use iced::keyboard::Modifiers;
use iced::widget::{button, center, column, text};
use iced::{Function, Point, Size, Subscription, Task, Widget, menu, window};

use crate::counter;

#[derive(Default)]
pub(super) struct Example {
    windows: BTreeMap<window::Id, Window>,
    next_counter: usize,
}

enum Window {
    Counter(counter::State),
    About,
}

#[derive(Debug, Clone)]
pub(super) enum Message {
    NewCounter,
    Counter(window::Id, counter::Message),
    About,
    Close(window::Id),
    Closed(window::Id),
}

impl Example {
    pub(super) fn new() -> (Self, Task<Message>) {
        let mut example = Self::default();
        let first = example.open_counter();
        let second = example.open_counter();

        (example, first.chain(second))
    }

    fn open_counter(&mut self) -> Task<Message> {
        self.next_counter += 1;

        let (id, open) = window::open(window::Settings {
            size: Size::new(440.0, 520.0),
            position: window::Position::Specific(Point::new(
                70.0 + ((self.next_counter - 1) % 2) as f32 * 470.0,
                100.0,
            )),
            ..window::Settings::default()
        });

        let _ = self
            .windows
            .insert(id, Window::Counter(counter::State::new(self.next_counter)));

        open.discard()
    }

    pub(super) fn title(&self, id: window::Id) -> String {
        match self.windows.get(&id) {
            Some(Window::Counter(counter)) => counter.name.clone(),
            Some(Window::About) => "About iced".to_owned(),
            None => "Native menus".to_owned(),
        }
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewCounter => self.open_counter(),
            Message::Counter(id, message) => {
                if let Some(Window::Counter(counter)) = self.windows.get_mut(&id) {
                    counter.update(message);
                }

                Task::none()
            }
            Message::About => {
                if let Some((&id, _)) = self
                    .windows
                    .iter()
                    .find(|(_, window)| matches!(window, Window::About))
                {
                    return window::gain_focus(id);
                }

                let (id, open) = window::open(window::Settings {
                    size: Size::new(380.0, 230.0),
                    position: window::Position::Centered,
                    ..window::Settings::default()
                });

                let _ = self.windows.insert(id, Window::About);

                open.discard()
            }
            Message::Close(id) => window::close(id),
            Message::Closed(id) => {
                let _ = self.windows.remove(&id);

                Task::none()
            }
        }
    }

    pub(super) fn subscription(&self) -> Subscription<Message> {
        window::close_events().map(Message::Closed)
    }

    pub(super) fn menu(&self, id: Option<window::Id>) -> menu::Bar<Message> {
        let file = menu![
            "File",
            menu::item("New Counter")
                .on_press(Message::NewCounter)
                .shortcut(Modifiers::COMMAND, 'n'),
        ]
        .extend(id.map(|id| {
            menu::item("Close Window")
                .on_press(Message::Close(id))
                .shortcut(Modifiers::COMMAND, 'w')
        }));
        let windows = menu!["Windows"].role(menu::Role::Window);
        let help = menu![
            "Support",
            menu::item("About iced…").on_press(Message::About)
        ]
        .role(menu::Role::Help);

        if let Some(id) = id
            && let Some(Window::Counter(counter)) = self.windows.get(&id)
        {
            [
                file,
                menu::edit(),
                counter.menu().map(Message::Counter.with(id)),
                windows,
                help,
            ]
            .into()
        } else {
            [file, windows, help].into()
        }
    }

    pub(super) fn view(&self, id: window::Id) -> impl Widget<Message> {
        match self.windows.get(&id) {
            Some(Window::Counter(counter)) => counter.view().map(Message::Counter.with(id)).boxed(),
            Some(Window::About) => center(
                column![
                    text("iced").size(40),
                    text("A cross-platform GUI library for Rust."),
                    text("https://iced.rs"),
                    button("Close").on_press(Message::Close(id)),
                ]
                .spacing(12)
                .align_x(iced::Center),
            )
            .boxed(),
            None => center(text("Opening…")).boxed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activate(bar: &menu::Bar<Message>, title: &str) -> Message {
        bar.menus()
            .iter()
            .flat_map(menu::Menu::entries)
            .find_map(|entry| match entry {
                menu::Entry::Item(item) if item.title() == title && item.is_enabled() => {
                    match item.action()? {
                        menu::Action::Message(factory) => Some(factory()),
                        menu::Action::Edit(_) => None,
                    }
                }
                _ => None,
            })
            .expect("the example declares this enabled application command")
    }

    #[test]
    fn identical_menu_labels_route_to_their_own_windows() {
        let first = window::Id::unique();
        let second = window::Id::unique();
        let mut example = Example {
            windows: BTreeMap::from([
                (first, Window::Counter(counter::State::new(1))),
                (second, Window::Counter(counter::State::new(1))),
            ]),
            next_counter: 2,
        };

        let first_menu = example.menu(Some(first));
        let second_menu = example.menu(Some(second));
        let message = activate(&first_menu, "Increase by 1");

        assert!(matches!(message, Message::Counter(id, _) if id == first));
        assert!(matches!(
            activate(&second_menu, "Increase by 1"),
            Message::Counter(id, _) if id == second
        ));

        let _ = example.update(message);

        assert!(iced_test::simulator(example.view(first)).find("1").is_ok());
        assert!(iced_test::simulator(example.view(second)).find("0").is_ok());

        let _ = example.update(Message::Closed(first));
        let _ = example.update(activate(&first_menu, "Increase by 1"));

        assert!(iced_test::simulator(example.view(second)).find("0").is_ok());
    }

    #[test]
    fn zero_windows_keep_new_counter_available() {
        let example = Example::default();
        let menu = example.menu(None);

        assert!(matches!(
            activate(&menu, "New Counter"),
            Message::NewCounter
        ));
        assert!(menu.menus().iter().all(|menu| menu.title() != "Edit"));
    }
}
