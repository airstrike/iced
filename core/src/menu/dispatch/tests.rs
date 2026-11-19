use std::cell::Cell;
use std::rc::Rc;

use crate::edit;
use crate::menu::{self, Bar, Menu, Output};

#[test]
fn all_leaves_share_one_owned_nonclone_mapper_and_release_it_together() {
    struct Message(u32);
    struct Mapper {
        calls: Cell<u32>,
        dropped: Rc<Cell<bool>>,
    }
    impl Mapper {
        fn map(&self, message: Message) -> (u32, u32) {
            self.calls.set(self.calls.get() + 1);
            (message.0, self.calls.get())
        }
    }
    impl Drop for Mapper {
        fn drop(&mut self) {
            self.dropped.set(true);
        }
    }

    let dropped = Rc::new(Cell::new(false));
    let mapper = Mapper {
        calls: Cell::new(0),
        dropped: Rc::clone(&dropped),
    };
    let produced = Rc::new(Cell::new(0));
    let source = menu::menu(
        "Several leaves",
        [10, 20].into_iter().map(|value| {
            let produced = Rc::clone(&produced);
            menu::item(value.to_string()).on_press_with(move || {
                produced.set(produced.get() + 1);
                Message(value)
            })
        }),
    );
    let mapped = source.map(move |message| mapper.map(message));
    assert_eq!(produced.get(), 0, "building and mapping must remain lazy");
    assert_eq!(mapped.entries().len(), 2);
    let (description, dispatch) = mapped.into_parts();
    assert_eq!(description.entries().len(), 2);
    assert_eq!(produced.get(), 0, "description inspection must remain lazy");
    for (path, expected) in [([0], (10, 1)), ([1], (20, 2)), ([0], (10, 3))] {
        let Some(Output::Message(actual)) = dispatch.activate(&path) else {
            panic!("the selected item has a factory");
        };
        assert_eq!(actual, expected);
    }
    assert_eq!(produced.get(), 3);
    assert!(!dropped.get());
    drop(dispatch);
    assert!(
        dropped.get(),
        "the adapter must release its only mapper capture"
    );
    assert_eq!(description.entries().len(), 2);
}

#[test]
fn nested_mapping_preserves_order_and_semantic_edits_bypass_every_mapper() {
    let calls = Rc::new(Cell::new(0));
    let source = Menu::new("Parent")
        .push(
            Menu::new("Child")
                .push(menu::item("Message").on_press(3_u8).map(|value| value + 1))
                .push(menu::Item::edit(edit::Command::Copy))
                .map(|value| u16::from(value) * 2),
        )
        .map(|value| u32::from(value) + 10);
    let bar = Bar::from(source).map({
        let calls = Rc::clone(&calls);
        move |value| {
            calls.set(calls.get() + 1);
            format!("{value}")
        }
    });
    let (_, dispatch) = bar.into_parts();
    assert!(matches!(
        dispatch.activate(&[0, 0, 1]),
        Some(Output::Edit(edit::Command::Copy))
    ));
    assert_eq!(calls.get(), 0);
    for expected_calls in 1..=2 {
        let Some(Output::Message(message)) = dispatch.activate(&[0, 0, 0]) else {
            panic!("mapped message");
        };
        assert_eq!(message, "18");
        assert_eq!(calls.get(), expected_calls);
    }
}

#[test]
fn appending_after_mapping_keeps_old_paths_and_maps_each_generation_once() {
    let menu = Menu::new("Root")
        .push(menu::item("Original").on_press(1_u8))
        .map(|value| u16::from(value) + 10)
        .push(menu::item("Appended").on_press(100_u16))
        .extend([menu::item("Extended").on_press(200_u16)])
        .map(|value| u32::from(value) * 2)
        .push(menu::item("Last").on_press(1_000_u32));
    let bar = Bar::from(menu)
        .map(|value| u64::from(value) + 1)
        .push(Menu::new("Later root").push(menu::item("Fresh").on_press(9_u64)))
        .map(|value| value * 3);
    let (_, dispatch) = bar.into_parts();
    for (path, expected) in [
        ([0, 0], 69),
        ([0, 1], 603),
        ([0, 2], 1_203),
        ([0, 3], 3_003),
        ([1, 0], 27),
    ] {
        let Some(Output::Message(actual)) = dispatch.activate(&path) else {
            panic!("the appended entry retains its action");
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn invalid_paths_and_non_actions_do_not_run_a_mapper() {
    let menu = crate::menu![
        "Root",
        menu::separator(),
        menu::item("No action"),
        Menu::new("Empty"),
        menu::item("Message").on_press(1_u8),
    ]
    .map(|_| -> () { panic!("invalid paths must not reach an action") });
    let (_, dispatch) = menu.into_parts();
    for path in [
        &[][..],
        &[0],
        &[1],
        &[2],
        &[2, 0],
        &[3, 0],
        &[4],
        &[usize::MAX],
    ] {
        assert!(dispatch.activate(path).is_none());
    }
}
