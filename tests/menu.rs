//! Native menu builder and decorator regression tests.
use iced::menu;
use iced::widget::{self, Widget};
use iced::{Program, Subscription, Task, Theme};

use std::cell::Cell;
use std::rc::Rc;

struct Message(Cell<u32>);

fn view(_state: &u32) -> impl Widget<Message> {
    widget::space()
}

fn daemon_view(state: &u32, _window: iced::window::Id) -> impl Widget<Message> {
    view(state)
}

fn update(state: &mut u32, message: Message) {
    *state = message.0.get();
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "The application menu callback borrows its state."
)]
fn menus(state: &u32) -> menu::Bar<Message> {
    let next = *state + 1;
    menu::bar([menu::Menu::new(state.to_string())
        .push(menu::item("Next").on_press_with(move || Message(Cell::new(next))))])
}

fn activate<P: Program<State = u32, Message = Message>>(program: &P, state: &mut u32) {
    let bar = program.menu(state, None);
    let (_, dispatch) = bar.into_parts();
    let Some(menu::Output::Message(message)) = dispatch.activate(&[0, 0]) else {
        panic!("factory")
    };
    let _ = program.update(state, message);
}

#[test]
fn application_menu_survives_every_builder_wrapper_in_either_order() {
    let program = iced::application(|| 1, update, view)
        .presets([iced::Preset::new("Sample", || (7, Task::none()))])
        .antialiasing(true)
        .menu(menus)
        .title("Menu test")
        .subscription(|_| Subscription::none())
        .theme(Theme::Dark)
        .style(|_, theme| iced::theme::Base::base(theme))
        .scale_factor(|_| 1.5)
        .executor::<iced::executor::Default>();
    let (mut state, _) = program.boot();
    activate(&program, &mut state);
    assert_eq!(state, 2);
    assert!(Program::settings(&program).antialiasing);
    assert_eq!(Program::presets(&program)[0].name(), "Sample");
    assert_eq!(
        Program::scale_factor(&program, &state, iced::window::Id::unique()),
        1.5
    );

    let program = iced::application(|| 1, update, view)
        .title("Menu test")
        .subscription(|_| Subscription::none())
        .theme(Theme::Dark)
        .style(|_, theme| iced::theme::Base::base(theme))
        .scale_factor(|_| 1.5)
        .executor::<iced::executor::Default>()
        .menu(menus);
    activate(&program, &mut state);
    assert_eq!(state, 3);
}

#[test]
fn daemon_menu_receives_key_window_and_none_after_all_wrappers() {
    let seen = Rc::new(Cell::new(None));
    let program = iced::daemon(|| 1, update, daemon_view)
        .menu({
            let seen = Rc::clone(&seen);
            move |state, window| {
                seen.set(window);
                menus(state)
            }
        })
        .presets([iced::Preset::new("Sample", || (7, Task::none()))])
        .title("Daemon test")
        .subscription(|_| Subscription::none())
        .theme(Theme::Dark)
        .style(|_, theme| iced::theme::Base::base(theme))
        .scale_factor(|_, _| 2.0)
        .executor::<iced::executor::Default>();
    let window = iced::window::Id::unique();
    let _ = Program::menu(&program, &1, Some(window));
    assert_eq!(seen.get(), Some(window));
    let mut state = 1;
    activate(&program, &mut state);
    assert_eq!(seen.get(), None);
    assert_eq!(state, 2);
    assert!(Program::window(&program).is_none());
    assert_eq!(Program::presets(&program)[0].name(), "Sample");
}

#[cfg(all(feature = "debug", feature = "time-travel"))]
#[test]
fn devtools_disable_semantic_edits_and_discard_messages_while_rewinding() {
    fn view(_: &u32) -> impl Widget<u32> {
        widget::space()
    }

    let program = iced::application(
        || 1_u32,
        |state: &mut u32, message: u32| *state = message,
        view,
    )
    .menu(|_| {
        menu::bar([
            menu::Menu::new("File").push(menu::item("Change").on_press(7)),
            menu::edit(),
        ])
    });
    let program = iced_devtools::attach(program);
    let (mut state, _) = program.boot();
    let live_bar = program.menu(&state, None);
    let (_, live_dispatch) = live_bar.into_parts();
    let Some(menu::Output::Message(queued)) = live_dispatch.activate(&[0, 0]) else {
        panic!("factory")
    };

    let _ = program.update(
        &mut state,
        iced_devtools::Event::Command(iced_debug::Command::RewindTo { message: 0 }),
    );
    let _ = program.update(&mut state, queued);
    assert_eq!(*state.state(), 1);
    let bar = program.menu(&state, None);
    assert!(bar.menus().iter().all(|menu| !menu.is_enabled()));
    let (menus, dispatch) = bar.into_parts();
    assert!(matches!(
        dispatch.activate(&[0, 0]),
        Some(menu::Output::Message(iced_devtools::Event::Discard))
    ));
    let menu::description::Entry::Item(item) = &menus[1].entries()[0] else {
        panic!("edit item")
    };
    assert!(matches!(
        item.action(),
        Some(menu::description::Action::Edit(iced::edit::Command::Cut))
    ));
    let _ = program.update(
        &mut state,
        iced_devtools::Event::Command(iced_debug::Command::GoLive),
    );
    assert_eq!(
        *state.state(),
        1,
        "the late menu command must not change present state"
    );
    assert!(
        program
            .menu(&state, None)
            .menus()
            .iter()
            .all(menu::description::Menu::is_enabled)
    );
}

#[cfg(feature = "tester")]
#[test]
fn tester_forwards_the_program_menu_without_requiring_clone_or_sync_messages() {
    let program = iced_tester::attach(iced::application(|| 42, update, view).menu(menus));
    let (state, _) = program.boot();
    let bar = program.menu(&state, None);
    assert_eq!(bar.menus()[0].title(), "42");
    let (_, dispatch) = bar.into_parts();
    assert!(matches!(
        dispatch.activate(&[0, 0]),
        Some(menu::Output::Message(_))
    ));
}
