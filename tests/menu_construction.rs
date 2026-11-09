//! Consumer-facing construction, conversion, and macro hygiene checks.
use std::cell::Cell;

use iced::menu;
use iced::widget::{self, Widget};
use iced::{Program, window};

mod facade {
    pub use iced::menu;
}

struct Message(Cell<u32>);

struct Custom;

impl From<Custom> for menu::Entry<Message> {
    fn from(_: Custom) -> Self {
        menu::item("Custom")
            .on_press_with(|| Message(Cell::new(7)))
            .into()
    }
}

fn update(state: &mut u32, message: Message) {
    *state = message.0.get();
}

fn view(_: &u32) -> impl Widget<Message> {
    widget::space()
}

fn daemon_view(state: &u32, _: window::Id) -> impl Widget<Message> {
    view(state)
}

#[test]
fn macro_converts_mixed_entries_once_and_preserves_lazy_nonclone_messages() {
    let mut evaluations = Vec::new();
    let menu = facade::menu![
        {
            evaluations.push("title");
            "Root"
        },
        {
            evaluations.push("item");
            menu::item("Disabled")
                .enabled(false)
                .checked(true)
                .on_press_with(|| Message(Cell::new(1)))
        },
        menu::separator(),
        facade::menu!["Nested", Custom],
    ];
    assert_eq!(evaluations, ["title", "item"]);

    let mapped = Cell::new(0);
    let menu = menu.map(move |message| {
        mapped.set(mapped.get() + 1);
        (message.0.get(), mapped.get())
    });
    let [
        menu::Entry::Item(item),
        menu::Entry::Separator,
        menu::Entry::Submenu(nested),
    ] = menu.entries()
    else {
        panic!("the macro preserves mixed entry order");
    };
    assert!(!item.is_enabled());
    assert!(item.is_checked());
    let menu::Entry::Item(item) = &nested.entries()[0] else {
        panic!("the custom conversion produces an item");
    };
    let Some(menu::Action::Message(produce)) = item.action() else {
        panic!("the item retains its message factory");
    };
    assert_eq!(produce(), (7, 1));
    assert_eq!(produce(), (7, 2));
}

#[test]
fn function_and_macro_share_an_import_and_accept_generated_or_empty_entries() {
    use facade::menu;

    let generated = menu("Generated", (0..3).map(|_| Custom));
    assert_eq!(generated.entries().len(), 3);
    let empty: menu::Menu<Message> = menu!["Empty",];
    assert!(empty.entries().is_empty());
    let empty: menu::Menu<Message> = menu!["Empty"];
    assert!(empty.entries().is_empty());
    let separator_only: menu::Menu<Message> = menu!["Group", menu::separator()];
    assert!(matches!(separator_only.entries(), [menu::Entry::Separator]));
}

#[test]
fn application_callbacks_accept_single_menus_arrays_and_vectors() {
    let single = iced::application(|| 0, update, view).menu(|_| menu!["File", Custom]);
    let (mut state, _) = single.boot();
    let bar = Program::menu(&single, &state, None);
    let menu::Entry::Item(item) = &bar.menus()[0].entries()[0] else {
        panic!("the single menu is converted into a bar");
    };
    let Some(menu::Action::Message(produce)) = item.action() else {
        panic!("the converted menu retains its application command");
    };
    let _ = single.update(&mut state, produce());
    assert_eq!(state, 7);

    let array =
        iced::application(|| 0, update, view).menu(|_| [menu!["File", Custom], menu::edit()]);
    let bar = Program::menu(&array, &state, None);
    assert_eq!(bar.menus()[0].title(), "File");
    assert_eq!(bar.menus()[1].title(), "Edit");

    let vector =
        iced::application(|| 0, update, view).menu(|_| vec![menu!["File", Custom], menu::edit()]);
    assert_eq!(Program::menu(&vector, &state, None).menus().len(), 2);

    let empty = iced::application(|| 0, update, view).menu(|_| []);
    assert!(Program::menu(&empty, &state, None).is_empty());
}

#[test]
fn daemon_conversion_preserves_the_focused_window_and_fallback() {
    let program = iced::daemon(|| 0, update, daemon_view)
        .menu(|_, window| [menu![format!("{window:?}"), Custom], menu::edit()]);
    let (state, _) = program.boot();
    let window = window::Id::unique();
    let bar = Program::menu(&program, &state, Some(window));
    assert_eq!(bar.menus()[0].title(), format!("{:?}", Some(window)));
    assert_eq!(
        Program::menu(&program, &state, None).menus()[0].title(),
        "None"
    );
}
