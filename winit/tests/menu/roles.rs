use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::MainThreadMarker;

use super::registration::{self, Registration};

pub(super) fn run() {
    let mtm = MainThreadMarker::new().expect("the harness-free test runs on the main thread");
    let application = NSApplication::sharedApplication(mtm);
    let previous_main = application.mainMenu();
    let main = NSMenu::new(mtm);
    application.setMainMenu(Some(&main));

    let original = NSMenu::new(mtm);
    let first = NSMenu::new(mtm);
    let second = NSMenu::new(mtm);
    let external = NSMenu::new(mtm);
    let before = application.helpMenu();
    let assert_help = |menu: &NSMenu| {
        let current = application
            .helpMenu()
            .expect("the test installs a Help menu");
        assert!(std::ptr::eq(&*current, menu));
    };
    application.setHelpMenu(Some(&original));
    let mut registration = Registration::new(mtm);
    registration.update(Some(&first));
    assert_help(&first);
    registration.update(Some(&second));
    assert_help(&second);
    registration.update(None);
    assert_help(&original);
    registration.update(Some(&first));
    drop(registration);
    assert_help(&original);

    let mut registration = Registration::new(mtm);
    registration.update(Some(&first));
    application.setHelpMenu(Some(&external));
    drop(registration);
    assert_help(&external);

    let mut registration = Registration::new(mtm);
    registration.update(Some(&first));
    application.setHelpMenu(Some(&original));
    registration.update(Some(&second));
    drop(registration);
    assert_help(&original);

    application.setHelpMenu(None);
    let mut registration = Registration::new(mtm);
    registration.update(Some(&first));
    drop(registration);
    assert!(application.helpMenu().is_none());
    application.setHelpMenu(before.as_deref());

    let first_item = NSMenuItem::new(mtm);
    first_item.setSubmenu(Some(&first));
    main.addItem(&first_item);
    registration::window(&first);
    let assert_window = |menu: &NSMenu| {
        let current = application
            .windowsMenu()
            .expect("the test installs a Window menu");
        assert!(std::ptr::eq(&*current, menu));
    };
    assert_window(&first);

    // A replacement root is selected even while the old marked object is retained.
    main.removeItem(&first_item);
    let second_item = NSMenuItem::new(mtm);
    second_item.setSubmenu(Some(&second));
    main.addItem(&second_item);
    registration::window(&second);
    assert_window(&second);
    main.removeItem(&second_item);
    main.addItem(&first_item);
    assert_window(&first);

    application.setMainMenu(previous_main.as_deref());
    println!("native menu roles: Window replacement and scoped Help restoration passed");
}
