//! Semantic editing exercises the same widgets and clipboard effects as keyboard editing.
use iced_runtime::UserInterface;
use iced_runtime::core::clipboard;
use iced_runtime::core::edit;
use iced_runtime::core::event;
use iced_runtime::core::keyboard;
use iced_runtime::core::mouse;
use iced_runtime::core::renderer;
use iced_runtime::core::shell;
use iced_runtime::core::widget;
use iced_runtime::core::widget::operation;
use iced_runtime::core::window;
use iced_runtime::core::{Clipboard, Event, Size, Theme, Widget};
use iced_runtime::user_interface;
use iced_widget::Renderer;
use iced_widget::text_editor;

use std::sync::Arc;

const INPUT: widget::Id = widget::Id::new("input");

struct Harness<'a, Message> {
    interface: UserInterface<'a, Message, Theme, Renderer>,
    renderer: Renderer,
    messages: shell::Bus<Message>,
}

impl<'a, Message: 'a> Harness<'a, Message> {
    fn new(widget: impl Widget<Message, Theme, Renderer> + 'a) -> Self {
        let mut renderer = iced_runtime::futures::futures::executor::block_on(
            <Renderer as renderer::Headless>::new(renderer::Settings::default(), Some("tiny-skia")),
        )
        .expect("create headless renderer");
        let interface = UserInterface::build(
            widget,
            Size::new(600.0, 400.0),
            user_interface::Cache::default(),
            &mut renderer,
        );

        Self {
            interface,
            renderer,
            messages: shell::Bus::new(),
        }
    }

    fn focus(&mut self) {
        self.interface
            .operate(&self.renderer, &mut operation::focusable::focus(INPUT));
    }

    fn send(&mut self, event: Event) -> (Clipboard, event::Status) {
        let (state, statuses) = self.interface.update(
            &window::Headless,
            &shell::Waker::noop(),
            &[event],
            mouse::Cursor::Unavailable,
            &mut self.renderer,
            &mut self.messages,
        );
        let user_interface::State::Updated { clipboard, .. } = state else {
            panic!("editing should not replace the widget tree");
        };

        (clipboard, statuses[0])
    }

    fn edit(&mut self, command: edit::Command) -> Clipboard {
        let (clipboard, status) = self.send(Event::Edit(command));
        assert_eq!(status, event::Status::Captured);
        clipboard
    }

    fn drain(&mut self) -> Vec<Message> {
        self.messages.drain().map(|(message, _)| message).collect()
    }
}

fn reply(value: &str) -> Event {
    Event::Clipboard(clipboard::Event::Read(Ok(Arc::new(
        clipboard::Content::Text(value.to_owned()),
    ))))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Message {
    Input(String),
    Paste(String),
}

#[test]
fn input_commands_preserve_messages_clipboard_and_single_line_filtering() {
    let mut harness = Harness::new(
        iced_widget::text_input("", "original")
            .id(INPUT)
            .on_input(Message::Input)
            .on_paste(Message::Paste),
    );
    harness.focus();

    let _ = harness.edit(edit::Command::SelectAll);
    assert!(harness.drain().is_empty());
    assert_eq!(
        harness.edit(edit::Command::Copy).write,
        Some(clipboard::Content::Text("original".into()))
    );
    assert!(harness.drain().is_empty());
    assert_eq!(
        harness.edit(edit::Command::Cut).write,
        Some(clipboard::Content::Text("original".into()))
    );
    assert_eq!(harness.drain(), [Message::Input(String::new())]);
    assert_eq!(
        harness.edit(edit::Command::Paste).reads,
        [clipboard::Kind::Text]
    );
    assert!(harness.drain().is_empty());

    let _ = harness.send(reply("first\r\nsecond"));
    assert_eq!(harness.drain(), [Message::Paste("firstsecond".into())]);
}

#[test]
fn input_paste_uses_on_input_when_no_on_paste_is_set() {
    let mut harness = Harness::new(
        iced_widget::text_input("", "")
            .id(INPUT)
            .on_input(Message::Input),
    );
    harness.focus();
    let _ = harness.edit(edit::Command::Paste);
    let _ = harness.send(reply("pasted"));
    assert_eq!(harness.drain(), [Message::Input("pasted".into())]);
}

#[test]
fn secure_input_copies_only_masked_text_and_publishes_normal_edits() {
    let mut harness = Harness::new(
        iced_widget::text_input("", "secret")
            .id(INPUT)
            .secure(true)
            .on_input(Message::Input),
    );
    harness.focus();
    // Secure input moves its visible selection without producing a message.
    let _ = harness.send(Event::Edit(edit::Command::SelectAll));
    let masked = clipboard::Content::Text("•".repeat(6));
    assert_eq!(
        harness.edit(edit::Command::Copy).write,
        Some(masked.clone())
    );
    assert_eq!(harness.edit(edit::Command::Cut).write, Some(masked));
    assert_eq!(harness.drain(), [Message::Input(String::new())]);
    let _ = harness.edit(edit::Command::Paste);
    let _ = harness.send(reply("new"));
    assert_eq!(harness.drain(), [Message::Input("new".into())]);
    let _ = harness.send(Event::Edit(edit::Command::SelectAll));
    assert_eq!(
        harness.edit(edit::Command::Copy).write,
        Some(clipboard::Content::Text("•".repeat(3)))
    );
}

#[test]
fn editor_commands_publish_actions_without_mutating_application_content() {
    let mut content = text_editor::Content::with_text("first\nsecond");
    content.perform(text_editor::Action::SelectAll);
    let actions = {
        let mut harness = Harness::new(
            iced_widget::text_editor(&content)
                .id(INPUT)
                .on_action(std::convert::identity)
                .key_binding(|_| None),
        );
        harness.focus();
        let _ = harness.edit(edit::Command::SelectAll);
        assert_eq!(
            harness.edit(edit::Command::Copy).write,
            Some(clipboard::Content::Text("first\nsecond".into()))
        );
        assert_eq!(
            harness.edit(edit::Command::Cut).write,
            Some(clipboard::Content::Text("first\nsecond".into()))
        );
        assert_eq!(
            harness.edit(edit::Command::Paste).reads,
            [clipboard::Kind::Text]
        );
        let _ = harness.send(reply("new\nlines"));
        assert_eq!(content.text(), "first\nsecond");
        harness.drain()
    };

    assert_eq!(
        actions,
        [
            text_editor::Action::SelectAll,
            text_editor::Action::Edit(text_editor::Edit::Backspace),
            text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new("new\nlines".into()))),
        ]
    );
    for action in actions {
        content.perform(action);
    }
    assert_eq!(content.text(), "new\nlines");
}

fn assert_ignored<'a, Message: 'a>(harness: &mut Harness<'a, Message>) {
    for command in [
        edit::Command::Copy,
        edit::Command::Cut,
        edit::Command::Paste,
        edit::Command::SelectAll,
    ] {
        let (clipboard, status) = harness.send(Event::Edit(command));
        assert_eq!(status, event::Status::Ignored);
        assert!(clipboard.reads.is_empty());
        assert_eq!(clipboard.write, None);
        assert!(harness.drain().is_empty());
    }
}

#[test]
fn disabled_widgets_ignore_commands_even_with_retained_focus_and_selection() {
    let mut input = Harness::<Message>::new(iced_widget::text_input("", "original").id(INPUT));
    input.focus();
    input.interface.operate(
        &input.renderer,
        &mut operation::text_input::select_all(INPUT),
    );
    assert_ignored(&mut input);
    let _ = input.send(reply("ignored"));
    assert!(input.drain().is_empty());

    let mut content = text_editor::Content::with_text("original");
    content.perform(text_editor::Action::SelectAll);
    let mut editor =
        Harness::<text_editor::Action>::new(iced_widget::text_editor(&content).id(INPUT));
    editor.focus();
    assert_ignored(&mut editor);
    let _ = editor.send(reply("ignored"));
    assert!(editor.drain().is_empty());
    assert_eq!(content.text(), "original");
}

#[test]
fn input_requires_widget_and_window_focus_and_discards_late_paste_reply() {
    let mut harness = Harness::new(
        iced_widget::text_input("", "original")
            .id(INPUT)
            .on_input(Message::Input),
    );
    let _ = harness.send(Event::Window(window::Event::Focused));
    assert_ignored(&mut harness);
    harness.focus();
    let _ = harness.edit(edit::Command::SelectAll);
    let _ = harness.edit(edit::Command::Paste);
    let _ = harness.send(Event::Window(window::Event::Unfocused));
    assert_ignored(&mut harness);
    let _ = harness.send(reply("ignored"));
    assert!(harness.drain().is_empty());
    let _ = harness.send(Event::Window(window::Event::Focused));
    assert_eq!(
        harness.edit(edit::Command::Copy).write,
        Some(clipboard::Content::Text("original".into()))
    );
}

#[test]
fn editor_requires_widget_and_window_focus_and_discards_late_paste_reply() {
    let mut content = text_editor::Content::with_text("original");
    content.perform(text_editor::Action::SelectAll);
    let mut harness = Harness::new(
        iced_widget::text_editor(&content)
            .id(INPUT)
            .on_action(std::convert::identity),
    );
    let _ = harness.send(Event::Window(window::Event::Focused));
    assert_ignored(&mut harness);
    harness.focus();
    let _ = harness.edit(edit::Command::Paste);
    let _ = harness.send(Event::Window(window::Event::Unfocused));
    assert_ignored(&mut harness);
    let _ = harness.send(reply("ignored"));
    assert!(harness.drain().is_empty());
    let _ = harness.send(Event::Window(window::Event::Focused));
    assert_eq!(
        harness.edit(edit::Command::Copy).write,
        Some(clipboard::Content::Text("original".into()))
    );
}

#[test]
fn semantic_copy_uses_selectable_text_fallback_only_when_widget_did_not_capture_it() {
    let mut harness = Harness::new(iced_widget::column![
        iced_widget::text_input("", "input value")
            .id(INPUT)
            .on_input(Message::Input),
        iced_widget::text("display text").selectable(true),
    ]);
    harness.focus();
    harness
        .interface
        .operate(&harness.renderer, &mut operation::text::select_all());

    let (clipboard, status) = harness.send(Event::Edit(edit::Command::Copy));
    assert_eq!(status, event::Status::Ignored);
    assert_eq!(
        clipboard.write,
        Some(clipboard::Content::Text("display text".into()))
    );

    let mut key = iced_test::simulator::press_key(keyboard::Key::Character("c".into()), None);
    if let Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. }) = &mut key {
        *modifiers = keyboard::Modifiers::COMMAND;
    }
    assert_eq!(harness.send(key).0.write, clipboard.write);

    let _ = harness.edit(edit::Command::SelectAll);
    assert_eq!(
        harness.edit(edit::Command::Copy).write,
        Some(clipboard::Content::Text("input value".into()))
    );
}
