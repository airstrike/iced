mod counter;
mod example;

use example::Example;

fn main() -> iced::Result {
    iced::daemon(Example::new, Example::update, Example::view)
        .menu(Example::menu)
        .title(Example::title)
        .subscription(Example::subscription)
        .run()
}
