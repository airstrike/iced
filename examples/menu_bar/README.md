# Native menu bar

A macOS system menu bar shared by several iced windows. Run it with:

```sh
cargo run -p menu_bar
```

Two Counter windows start with different values and step sizes. Switch windows
to change the native menu. Each menu is described using the same `window::Id`
that selects its view and routes its messages:

```rust
iced::daemon(Example::new, Example::update, Example::view)
    .menu(Example::menu)
    .title(Example::title)
    .subscription(Example::subscription)
    .run()
```

The menu callback selects the same child state as `view`, then maps its messages
with the same window ID:

```rust
fn menu(&self, id: Option<window::Id>) -> menu::Bar<Message> {
    let file = menu::submenu("File")
        .push(menu::item("New Counter").on_press(Message::NewCounter));
    let mut bar = menu::bar([file]);

    if let Some(id) = id
        && let Some(Window::Counter(counter)) = self.windows.get(&id)
    {
        bar = bar
            .push(menu::edit())
            .push(counter.menu().map(Message::Counter.with(id)));
    }

    bar
}
```

`Some(id)` identifies the focused window; `None` requests the fallback menu.
The complete example also adds Close Window and About commands.

Use the Counter menu to increase, decrease, reset, choose a step, or allow
negative values. The buttons and checkbox update the same state, including the
menu's checks, labels, and disabled items. Command-I, Command-D, and Command-R
activate the corresponding native menu items.

The Edit menu operates on the focused window's name field or notes editor.
Cut, Copy, Paste, and Select All preserve the widgets' normal input and editing
messages. An edit item with no eligible focused control has no effect. Applications
can also request these actions with `iced::edit` tasks, such as
`iced::edit::copy(window_id)`.

Help → About iced opens a separate information window, which has a different
menu. File → New Counter opens another counter. Closing every window keeps the
daemon running: File → New Counter remains available. Quit from the normal
macOS application menu.

For a single-window application, `.menu` accepts a function of `&State` alone.
Descriptions are owned values; native handles and menu IDs stay in the shell.
`on_press` accepts a cloneable message; `on_press_with` creates a fresh message
on activation without requiring `Message: Clone`.

Native menu bars are currently implemented only on macOS. Requesting a nonempty
menu on another platform reaches an explicit `todo!()`. Applications without
custom menus, including the original `counter` example, retain their existing
behavior.
