use iced::widget::space;
use iced::{Element, Task, window};

use std::time::Instant;

pub fn main() -> iced::Result {
    iced::daemon(EventLoop::new, EventLoop::update, EventLoop::view).run()
}

struct EventLoop {
    count: u64,
    start: Instant,
    phase: Phase,
}

#[derive(Debug, Clone, Copy)]
enum Phase {
    Done,
    Perform,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    DoneTick,
    PerformTick,
}

impl EventLoop {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                count: 0,
                start: Instant::now(),
                phase: Phase::Done,
            },
            Task::done(Message::DoneTick),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        self.count += 1;

        let elapsed = self.start.elapsed();

        if elapsed.as_secs_f64() >= 5.0 {
            let secs = elapsed.as_secs_f64();
            let rate = self.count as f64 / secs;
            let label = match message {
                Message::DoneTick => "Task::done",
                Message::PerformTick => "Task::perform",
            };

            println!(
                "\x1b[1;36m{} event loop iterations/sec\x1b[0m ({} in {:.1}s) [{}]",
                format_rate(rate),
                format_count(self.count),
                secs,
                label,
            );

            match self.phase {
                Phase::Done => {
                    self.count = 0;
                    self.start = Instant::now();
                    self.phase = Phase::Perform;
                    Task::perform(async { Message::PerformTick }, |m| m)
                }
                Phase::Perform => iced::exit(),
            }
        } else {
            match message {
                Message::DoneTick => Task::done(Message::DoneTick),
                Message::PerformTick => Task::perform(async { Message::PerformTick }, |m| m),
            }
        }
    }

    fn view(&self, _window: window::Id) -> Element<'_, Message> {
        space().into()
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
