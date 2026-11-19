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
activate the corresponding native menu items. The More shortcuts submenu also
accepts F5, Command–Down Arrow, and Command–é:

```rust
use iced::keyboard::Modifiers;
use iced::keyboard::key::Named;

menu::item("Increase").on_press(Message::Increment)
    .shortcut(Modifiers::NONE, Named::F5);
menu::item("Reset").on_press(Message::Reset)
    .shortcut(Modifiers::COMMAND, 'é');
```

Shortcuts use iced's logical key types. Character keys accept a single Unicode
scalar and require a command modifier; named navigation/editing keys and F1–F35
may be unmodified. `try_shortcut` reports unsupported keys. AppKit handles matching
against the active keyboard layout; there is no synthetic keyboard event.

Items accept iced image handles for icons. The example creates its handles once,
then reuses them across menu updates: plus and minus are RGBA templates, while
Reset uses an embedded PNG in full color.

```rust
menu::item("Increase").on_press(Message::Increment)
    .icon(menu::Icon::new(&self.plus_image).template(true));
menu::item("Reset").on_press(Message::Reset).icon(&self.reset_image);
```

Templates use their alpha channel as a mask and adapt to macOS appearance.
Icons fit within 16 × 16 points while preserving aspect ratio. AppKit decodes
paths and encoded bytes; invalid images are omitted. Handles can be constructed
through `iced::widget::image` with `image-without-codecs` enabled; no Rust image
codec feature is required for native menus.

The Edit menu operates on the focused window's name field or notes editor.
Cut, Copy, Paste, and Select All preserve the widgets' normal input and editing
messages. An edit item with no eligible focused control has no effect. Applications
can also request these actions with `iced::edit` tasks, such as
`iced::edit::copy(window_id)`.

Support → About iced opens a separate information window with a different menu.
The example registers its menus explicitly, so their titles can be customized:

```rust
let windows: menu::Menu<Message> = menu!["Windows"].role(menu::Role::Window);
let help = menu!["Support", menu::item("About iced…").on_press(Message::About)]
    .role(menu::Role::Help);
```

AppKit maintains the Window menu's list of windows and the Help menu's search
field. The first top-level menu for each role is registered; roles on nested
menus have no effect. Help registration restores the previous Help menu when
removed. Window roles
follow the lifetime of the native menu; changing a role replaces that native
menu without altering inherited application menus.
File → New Counter opens another counter. Closing every window keeps the daemon
running: File → New Counter remains available. Quit from the normal macOS
application menu.

Menus own their actions and move into the runtime; native handles stay in
the shell. Mapping a menu owns one adapter around its actions, so a mapper can
capture state without implementing `Clone`. Factories and mappers only run on
activation. Low-level runtimes use `into_parts()` to separate message-free
presentation from its originating `Dispatch`; native targets retain that dispatch
along with the selected entry path. `entries()` and `menus()` inspect these
message-free descriptions; they no longer expose message factories. The consuming
`IntoIterator` implementations are replaced by `into_parts()`, because splitting
mapped children into separate owners would require sharing or cloning the mapper.

`on_press` accepts a cloneable message; `on_press_with` creates a fresh message on activation
without requiring `Message: Clone`.

Native menu bars are currently implemented only on macOS. Requesting a nonempty
menu on another platform reaches an explicit `todo!()`. Applications without
custom menus, including the original `counter` example, retain their existing
behavior.
