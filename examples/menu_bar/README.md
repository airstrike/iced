# Native menu bar

A macOS system menu bar shared by several iced windows. Run it with:

```sh
cargo run -p menu_bar
```

Two Counter windows start with different values and step sizes. Switch windows
to change the native menu. Each menu uses the same `window::Id` that selects its
view and routes its messages:

```rust
iced::daemon(Example::new, Example::update, Example::view)
    .menu(Example::menu)
    .title(Example::title)
    .subscription(Example::subscription)
    .run()
```

Use `menu!` for a list of items, separators, and nested menus. It converts each
entry automatically, just as `row!` and `column!` compose widgets:

```rust
use iced::menu;

let counter = menu![
    "Counter",
    menu::item("Increase").on_press(Message::Increment),
    menu::item("Decrease").on_press(Message::Decrement),
    menu::separator(),
    menu::item("Reset").on_press(Message::Reset),
];
```

The matching `menu` function accepts an iterator. The example uses it for the
Step submenu:

```rust
let steps = [Step::One, Step::Five, Step::Ten].into_iter().map(|step| {
    menu::item(step.value().to_string())
        .checked(self.step == step)
        .on_press(Message::Step(step))
});
let step_menu = menu("Step", steps);
```

A `.menu` callback can return a single menu, an array or vector of menus, or a
`menu::Bar`. For a single-window application, it takes `&State` alone:

```rust
iced::application(Counter::default, Counter::update, Counter::view)
    .menu(|_state| menu!["Counter", menu::item("Reset").on_press(Message::Reset)])
    .run()
```

Daemon callbacks also receive `Some(id)` for the focused window or `None` for
the fallback. In [src/example.rs](src/example.rs), the callback selects the
same child state as `view` and maps its menu with
`counter.menu().map(Message::Counter.with(id))`.

Use the Counter menu to increase, decrease, reset, choose a step, or allow
negative values. The buttons and checkbox update the same state, including the
menu's checks, labels, and disabled items. Command-I, Command-D, and Command-R
activate the corresponding native menu items.

The Edit menu operates on the focused window's name field or notes editor.
Cut, Copy, Paste, and Select All preserve the widgets' normal input and editing
messages. An edit item with no eligible focused control has no effect. Applications
can also request these actions with `iced::edit` tasks, such as
`iced::edit::copy(window_id)`.

Help → About iced opens a separate information window with a different menu.
File → New Counter opens another counter. Closing every window keeps the daemon
running: File → New Counter remains available. Quit from the normal macOS
application menu.

Descriptions own their actions and move into the runtime; native handles stay in
the shell. `on_press`
accepts a cloneable message; `on_press_with` creates a fresh message on activation
without requiring `Message: Clone`.

Native menu bars are currently implemented only on macOS. Requesting a nonempty
menu on another platform reaches an explicit `todo!()`. Applications without
custom menus, including the original `counter` example, retain their existing
behavior.
