use crate::menu::callback::Callback;
use crate::menu::scope::Scope;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn retiring_a_target_never_reinterprets_its_factory() {
    let value = Rc::new(Cell::new(0));
    let old = Rc::new(Callback::new(
        Rc::new(Scope::capture(None, crate::window::Focus::default())),
        {
            let value = value.clone();
            move || value.set(1)
        },
    ));
    let retained = old.clone();
    old.invoke();
    assert_eq!(value.get(), 1);
    old.retire();
    let replacement = Callback::new(
        Rc::new(Scope::capture(None, crate::window::Focus::default())),
        {
            let value = value.clone();
            move || value.set(2)
        },
    );
    replacement.invoke();
    retained.invoke();
    assert_eq!(value.get(), 2);
}

#[test]
fn releasing_target_releases_its_captured_state() {
    let state = Rc::new(());
    let weak = Rc::downgrade(&state);
    let callback = Callback::new(
        Rc::new(Scope::capture(None, crate::window::Focus::default())),
        move || {
            let _ = &state;
        },
    );
    assert!(weak.upgrade().is_some());
    drop(callback);
    assert!(weak.upgrade().is_none());
}

#[test]
fn a_frozen_window_callback_rechecks_current_focus_before_running_its_factory() {
    let key = Rc::new(Cell::new(true));
    let invocations = Rc::new(Cell::new(0));
    let scope = Rc::new(Scope {
        window: Some(crate::core::window::Id::unique()),
        is_current: Box::new({
            let key = key.clone();
            move || key.get()
        }),
    });
    let callback = Callback::new(scope, {
        let invocations = invocations.clone();
        move || invocations.set(invocations.get() + 1)
    });
    callback.invoke();
    key.set(false);
    callback.invoke();
    assert_eq!(invocations.get(), 1);
}

#[test]
fn identical_presentation_does_not_cache_an_old_factory() {
    use crate::core::menu;
    use crate::runtime::Action;
    use std::cell::RefCell;

    let output = Rc::new(RefCell::new(Vec::new()));
    let send: Rc<dyn Fn(Action<usize>)> = Rc::new({
        let output = output.clone();
        move |action| {
            if let Action::Output(value) = action {
                output.borrow_mut().push(value);
            }
        }
    });
    let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
    let make = |value| {
        lower(
            menu::Menu::new("Same").push(menu::item("Same").on_press_with(move || value)),
            true,
            &scope,
            &send,
        )
    };
    let first = make(1);
    let second = make(2);
    assert_eq!(first.title, second.title);
    let (crate::menu::branch::Entry::Item(first), crate::menu::branch::Entry::Item(second)) =
        (&first.entries[0], &second.entries[0])
    else {
        panic!("the test declares item entries");
    };
    assert_eq!(first.title, second.title);
    first.callback.as_ref().unwrap().invoke();
    first.callback.as_ref().unwrap().retire();
    second.callback.as_ref().unwrap().invoke();
    first.callback.as_ref().unwrap().invoke();
    assert_eq!(&*output.borrow(), &[1, 2]);
}

#[test]
fn inherited_then_first_enabled_shortcuts_win_and_disabled_ancestors_are_inert() {
    use crate::core::{keyboard::Modifiers, menu};
    use crate::runtime::Action;
    use std::collections::HashSet;

    let key = menu::Shortcut::new(Modifiers::LOGO, 'k').unwrap();
    let inherited = menu::Shortcut::new(Modifiers::LOGO, 'q').unwrap();
    let send: Rc<dyn Fn(Action<()>)> = Rc::new(|_| panic!("disabled factories must not run"));
    let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
    let menu = menu::Menu::new("Root")
        .push(
            menu::Menu::new("Disabled").enabled(false).push(
                menu::item("Hidden accelerator")
                    .on_press(())
                    .shortcut(Modifiers::LOGO, 'k'),
            ),
        )
        .push(
            menu::item("First")
                .on_press(())
                .shortcut(Modifiers::LOGO, 'k'),
        )
        .push(
            menu::item("Duplicate")
                .on_press(())
                .shortcut(Modifiers::LOGO, 'k'),
        )
        .push(
            menu::item("Inherited")
                .on_press(())
                .shortcut(Modifiers::LOGO, 'q'),
        );
    let mut branch = lower(menu, true, &scope, &send);
    branch.claim_shortcuts(&mut HashSet::from([inherited]));
    let crate::menu::branch::Entry::Submenu(disabled) = &branch.entries[0] else {
        panic!("the first entry is a submenu");
    };
    let crate::menu::branch::Entry::Item(disabled) = &disabled.entries[0] else {
        panic!("the disabled submenu contains an item");
    };
    assert!(!disabled.enabled);
    assert!(disabled.callback.is_none());
    assert!(disabled.shortcut.is_none());
    for (entry, expected) in branch.entries[1..].iter().zip([Some(key), None, None]) {
        let crate::menu::branch::Entry::Item(item) = entry else {
            panic!("the remaining entries are items");
        };
        assert_eq!(item.shortcut, expected);
    }
}

#[test]
fn native_edit_keeps_the_captured_window_and_is_disabled_without_one() {
    use crate::core::{edit, menu, window};
    use crate::runtime::Action;
    use std::cell::RefCell;

    let output = Rc::new(RefCell::new(Vec::new()));
    let send: Rc<dyn Fn(Action<()>)> = Rc::new({
        let output = output.clone();
        move |action| {
            if let Action::Event { window, event } = action {
                output.borrow_mut().push((window, event));
            }
        }
    });
    let window = window::Id::unique();
    let menu = || menu::Menu::new("Edit").push(menu::Item::edit(edit::Command::Paste));
    let scope = Rc::new(Scope {
        window: Some(window),
        is_current: Box::new(|| true),
    });
    let branch = lower(menu(), true, &scope, &send);
    let crate::menu::branch::Entry::Item(item) = &branch.entries[0] else {
        panic!("the Edit menu contains an item");
    };
    item.callback.as_ref().unwrap().invoke();
    assert!(
        matches!(&output.borrow()[..], [(id, crate::core::Event::Edit(edit::Command::Paste))] if *id == window)
    );

    let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
    let branch = lower(menu(), true, &scope, &send);
    let crate::menu::branch::Entry::Item(item) = &branch.entries[0] else {
        panic!("the Edit menu contains an item");
    };
    assert!(!item.enabled);
    assert!(item.callback.is_none());
}

#[test]
fn lowering_owns_factories_until_the_native_snapshot_is_released() {
    use crate::core::menu;
    use crate::runtime::Action;
    use std::cell::RefCell;

    struct Message(u32);

    let captured = Rc::new(Cell::new(0));
    let weak = Rc::downgrade(&captured);
    let menu = crate::core::menu![
        "File",
        menu::item("Run").on_press_with(move || {
            captured.set(captured.get() + 1);
            Message(captured.get())
        }),
    ];
    let output = Rc::new(RefCell::new(Vec::new()));
    let send: Rc<dyn Fn(Action<Message>)> = Rc::new({
        let output = Rc::clone(&output);
        move |action| {
            if let Action::Output(Message(value)) = action {
                output.borrow_mut().push(value);
            }
        }
    });
    let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
    let branch = lower(menu, true, &scope, &send);
    assert!(output.borrow().is_empty());
    let crate::menu::branch::Entry::Item(item) = &branch.entries[0] else {
        panic!("the menu contains the factory item");
    };
    let callback = item.callback.as_ref().unwrap();
    callback.invoke();
    callback.invoke();
    assert_eq!(*output.borrow(), [1, 2]);
    assert!(weak.upgrade().is_some());
    drop(branch);
    assert!(weak.upgrade().is_none());
}

fn lower<Message: 'static>(
    menu: crate::core::menu::Menu<Message>,
    enabled: bool,
    scope: &Rc<Scope>,
    send: &Rc<dyn Fn(crate::runtime::Action<Message>)>,
) -> super::Branch {
    let (description, dispatch) = menu.into_parts();
    let binding = Rc::new(crate::menu::binding::Binding::new(
        dispatch,
        Rc::clone(scope),
        Rc::clone(send),
    ));
    super::Branch::from_menu(&description, enabled, &binding, &mut Vec::new())
}

#[test]
fn retained_native_targets_keep_one_mapper_and_their_originating_dispatch() {
    use crate::core::menu;
    use crate::runtime::Action;
    use std::cell::RefCell;

    let output = Rc::new(RefCell::new(Vec::new()));
    let send: Rc<dyn Fn(Action<(u8, u32)>)> = Rc::new({
        let output = Rc::clone(&output);
        move |action| {
            if let Action::Output(value) = action {
                output.borrow_mut().push(value);
            }
        }
    });
    let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
    let capture = Rc::new(());
    let weak = Rc::downgrade(&capture);
    let count = Cell::new(0);
    let source = crate::core::menu![
        "Root",
        menu::item("First").on_press(1_u8),
        menu::Menu::new("Nested").push(menu::item("Second").on_press(2_u8)),
    ]
    .map(move |message| {
        let _keep_alive = &capture;
        count.set(count.get() + 1);
        (message, count.get())
    });
    let old = lower(source, true, &scope, &send);
    let crate::menu::branch::Entry::Item(first) = &old.entries[0] else {
        panic!("the first entry is an item");
    };
    let crate::menu::branch::Entry::Submenu(nested) = &old.entries[1] else {
        panic!("the second entry is a submenu");
    };
    let crate::menu::branch::Entry::Item(second) = &nested.entries[0] else {
        panic!("the submenu contains an item");
    };
    let retained_first = Rc::clone(first.callback.as_ref().expect("the item is enabled"));
    let retained_second = Rc::clone(second.callback.as_ref().expect("the item is enabled"));
    retained_first.invoke();
    retained_second.invoke();
    drop(old);
    assert!(
        weak.upgrade().is_some(),
        "native targets retain the original dispatcher"
    );

    let replacement = lower(
        menu::Menu::new("Root").push(menu::item("First").on_press((99, 0))),
        true,
        &scope,
        &send,
    );
    let crate::menu::branch::Entry::Item(replacement) = &replacement.entries[0] else {
        panic!("the replacement contains an item");
    };
    replacement
        .callback
        .as_ref()
        .expect("enabled replacement")
        .invoke();
    retained_first.invoke();
    retained_second.retire();
    retained_second.invoke();
    assert_eq!(*output.borrow(), [(1, 1), (2, 2), (99, 0), (1, 3)]);
    drop(retained_first);
    assert!(weak.upgrade().is_some());
    drop(retained_second);
    assert!(
        weak.upgrade().is_none(),
        "releasing the last target releases the owned mapper"
    );
}

#[test]
fn mapped_menu_roles_reach_the_native_snapshot() {
    use crate::core::menu;
    use crate::runtime::Action;

    let menu = crate::core::menu!["Support", menu::item("About").on_press(1_u8)]
        .role(menu::Role::Help)
        .map(u16::from);
    let send: Rc<dyn Fn(Action<u16>)> = Rc::new(|_| {});
    let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
    let branch = lower(menu, true, &scope, &send);
    assert_eq!(branch.role, Some(menu::Role::Help));
    assert_eq!(branch.title, "Support");
}

#[test]
fn mapped_item_icons_reach_the_native_snapshot() {
    use crate::core::{image, menu};
    use crate::runtime::Action;

    let icon = menu::Icon::new(image::Handle::from_rgba(1, 1, vec![0, 0, 0, 255])).template(true);
    let menu = crate::core::menu!["Counter", menu::item("Increase").icon(&icon).on_press(1_u8)]
        .map(u16::from);
    let send: Rc<dyn Fn(Action<u16>)> = Rc::new(|_| {});
    let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
    let branch = lower(menu, true, &scope, &send);
    let crate::menu::branch::Entry::Item(item) = &branch.entries[0] else {
        panic!("the menu contains one item");
    };
    assert_eq!(item.icon.as_ref(), Some(&icon));
}
