//! Window-scoped semantic editing tasks produce events without application output.
use iced_runtime::core::edit::Command;
use iced_runtime::core::{Event, window};
use iced_runtime::{Action, Task, edit, task};

#[test]
fn helpers_preserve_the_target_window_and_command() {
    use iced_runtime::futures::futures::StreamExt;
    use iced_runtime::futures::futures::executor;

    let window = window::Id::unique();
    let tasks: [(Command, Task<()>); 4] = [
        (Command::Copy, edit::copy(window)),
        (Command::Cut, edit::cut(window)),
        (Command::Paste, edit::paste(window)),
        (Command::SelectAll, edit::select_all(window)),
    ];

    for (command, task) in tasks {
        let actions = executor::block_on(
            task::into_stream(task)
                .expect("edit task contains an event")
                .collect::<Vec<_>>(),
        );
        assert_eq!(actions.len(), 1);
        let Action::Event {
            window: target,
            event: Event::Edit(actual),
        } = &actions[0]
        else {
            panic!("editing must emit a semantic event");
        };
        assert_eq!(*target, window);
        assert_eq!(*actual, command);
    }
}
