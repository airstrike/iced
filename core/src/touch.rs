//! Build touch events.
use crate::Point;
use crate::time::Duration;

/// A touch interaction.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub enum Event {
    /// A touch interaction was started.
    FingerPressed {
        id: Finger,
        position: Point,
        /// Original event time in a platform-defined monotonic clock domain.
        /// Compare within this contact; the epoch is not shared with `Instant`.
        /// `None` means the backend does not expose the original event time.
        timestamp: Option<Duration>,
    },

    /// An on-going touch interaction was moved.
    FingerMoved {
        id: Finger,
        position: Point,
        /// Original event time in a platform-defined monotonic clock domain.
        /// Compare within this contact; the epoch is not shared with `Instant`.
        /// `None` means the backend does not expose the original event time.
        timestamp: Option<Duration>,
    },

    /// A touch interaction was ended.
    FingerLifted {
        id: Finger,
        position: Point,
        /// Original event time in a platform-defined monotonic clock domain.
        /// Compare within this contact; the epoch is not shared with `Instant`.
        /// `None` means the backend does not expose the original event time.
        timestamp: Option<Duration>,
    },

    /// A touch interaction was canceled.
    FingerLost {
        id: Finger,
        position: Point,
        /// Original event time in a platform-defined monotonic clock domain.
        /// Compare within this contact; the epoch is not shared with `Instant`.
        /// `None` means the backend does not expose the original event time.
        timestamp: Option<Duration>,
    },
}

/// A unique identifier representing a finger on a touch interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Finger(pub u64);
