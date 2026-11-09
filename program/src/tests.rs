use super::*;
use crate::core::Theme;

use std::cell::Cell;
use std::rc::Rc;

struct Message(Cell<u32>);

struct Definition {
    presets: Vec<Preset<u32, Message>>,
}

impl Program for Definition {
    type State = u32;
    type Message = Message;
    type Theme = Theme;
    type Renderer = ();
    type Executor = futures::backend::null::Executor;

    fn name() -> &'static str {
        "menus"
    }

    fn settings(&self) -> Settings {
        Settings {
            antialiasing: true,
            ..Settings::default()
        }
    }

    fn window(&self) -> Option<window::Settings> {
        None
    }

    fn boot(&self) -> (u32, Task<Message>) {
        (1, Task::none())
    }

    fn update(&self, state: &mut u32, message: Message) -> Task<Message> {
        *state = message.0.get();
        Task::none()
    }

    fn view<'a>(
        &self,
        _state: &'a u32,
        _window: window::Id,
    ) -> impl Widget<Message, Theme, ()> + 'a {
        core::widget::Void
    }

    fn presets(&self) -> &[Preset<u32, Message>] {
        &self.presets
    }
}

#[test]
fn decorators_preserve_menus_presets_settings_and_lazy_nonclone_messages() {
    let calls = Rc::new(Cell::new(0));
    let seen_window = Rc::new(Cell::new(None));
    let program = with_menu(
        Definition {
            presets: vec![Preset::new("Sample", || (9, Task::none()))],
        },
        {
            let calls = Rc::clone(&calls);
            let seen_window = Rc::clone(&seen_window);
            move |state, window| {
                seen_window.set(window);
                menu::bar([menu::Menu::new(state.to_string()).push(
                    menu::item("Next").on_press_with({
                        let calls = Rc::clone(&calls);
                        let next = *state + 1;
                        move || {
                            calls.set(calls.get() + 1);
                            Message(Cell::new(next))
                        }
                    }),
                )])
            }
        },
    );
    fn exact_types(_: &impl Program<Renderer = (), Executor = futures::backend::null::Executor>) {}
    exact_types(&program);
    let program = with_subscription(program, |_| Subscription::none());
    let program = with_style(program, |_, theme| theme::Base::base(theme));
    let program = with_executor::<_, futures::backend::null::Executor>(program);
    assert_eq!(program.presets()[0].name(), "Sample");
    assert!(program.settings().antialiasing);
    assert!(program.window().is_none());
    let (mut instance, _) = Instance::new(program);
    let window = window::Id::unique();
    let menus = instance.menu(Some(window));
    assert_eq!(seen_window.get(), Some(window));
    assert_eq!(menus.menus()[0].title(), "1");
    assert_eq!(calls.get(), 0);
    let menu::Entry::Item(item) = &menus.menus()[0].entries()[0] else {
        panic!("item")
    };
    let Some(menu::Action::Message(produce)) = item.action() else {
        panic!("factory")
    };
    let _ = instance.update(produce());
    assert_eq!(calls.get(), 1);
    assert_eq!(instance.menu(None).menus()[0].title(), "2");
    assert_eq!(seen_window.get(), None);
}

#[test]
fn programs_without_menu_logic_have_an_empty_bar() {
    let (instance, _) = Instance::new(Definition {
        presets: Vec::new(),
    });
    assert!(instance.menu(None).is_empty());
}
