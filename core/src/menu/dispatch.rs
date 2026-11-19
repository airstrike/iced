#[expect(
    clippy::module_inception,
    reason = "The private leaf holds the owned dispatch and its behavior."
)]
mod dispatch;
mod entry;
mod prefix;

pub use dispatch::Dispatch;
pub(super) use entry::Entry;

#[cfg(test)]
mod tests;
