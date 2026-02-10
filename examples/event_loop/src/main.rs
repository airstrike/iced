use iced::widget::text;
use iced::{window, Element, Task};

use std::time::Instant;

pub fn main() -> iced::Result {
    iced::daemon(EventLoop::new, EventLoop::update, EventLoop::view).run()
}

struct EventLoop {
    count: u64,
    start: Instant,
    done: bool,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Tick,
}

impl EventLoop {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                count: 0,
                start: Instant::now(),
                done: false,
            },
            Task::done(Message::Tick),
        )
    }

    fn update(&mut self, _message: Message) -> Task<Message> {
        if self.done {
            return Task::none();
        }

        self.count += 1;

        let elapsed = self.start.elapsed();

        if elapsed.as_secs_f64() >= 5.0 {
            self.done = true;

            let secs = elapsed.as_secs_f64();
            let rate = self.count as f64 / secs;

            println!(
                "\x1b[1m{} event loop iterations/sec\x1b[0m ({} in {:.1}s)",
                format_rate(rate),
                format_count(self.count),
                secs,
            );

            iced::exit()
        } else {
            Task::done(Message::Tick)
        }
    }

    fn view(&self, _window: window::Id) -> Element<'_, Message> {
        text("").into()
    }
}

fn format_rate(rate: f64) -> String {
    if rate >= 1_000_000.0 {
        format!("{:.2}M", rate / 1_000_000.0)
    } else if rate >= 1_000.0 {
        format!("{:.1}K", rate / 1_000.0)
    } else {
        format!("{:.0}", rate)
    }
}

fn format_count(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();

    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }

    result.chars().rev().collect()
}
