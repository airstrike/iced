//! The `paste` events of the web: those of the paste shortcuts, and those of
//! the browser's Edit menu.
//!
//! The browser's `paste` event carries the clipboard without asking the user,
//! while `navigator.clipboard.readText` asks for permission in most browsers.
//! A read made for a paste shortcut therefore waits for the shortcut's `paste`
//! event. Safari delivers that event after a round trip to its UI process,
//! once timers have run, so the wait ends with the event itself, the next key
//! or pointer press, or a timeout; only the last two fall back to `readText`.
//!
//! A `paste` event that no shortcut awaits comes from the browser's Edit menu.
//! Its text waits for the read of the Paste key press the canvas is sent.
use super::target::Target;

use crate::core::clipboard::{Content, Error};

use std::mem;

/// A pending clipboard read.
pub type Read = Box<dyn FnOnce(Result<Content, Error>)>;

/// How long a paste shortcut waits for its `paste` event before its reads
/// fall back to `readText`, in milliseconds. Only browsers that never fire the
/// event reach it; a late event is still taken (see [`Paste::time_out`]).
#[cfg(target_arch = "wasm32")]
pub const TIMEOUT: i32 = 500;

/// The last paste shortcut, up to the next key or pointer press.
#[derive(Default)]
pub struct Paste {
    /// Counts the paste shortcuts, so a timeout only ends its own.
    count: u64,
    shortcut: Shortcut,
    /// The text of a `paste` event that no read has taken yet.
    text: Option<String>,
}

#[derive(Default)]
enum Shortcut {
    /// No paste shortcut awaits its `paste` event, so a `paste` event comes
    /// from the browser's Edit menu.
    #[default]
    Idle,
    /// A paste shortcut was pressed and its `paste` event has not fired yet.
    /// The reads made for it wait for the event.
    Expected { reads: Vec<Read> },
    /// The paste shortcut waited too long for its `paste` event, which may
    /// still come. If the clipboard was read for it, the event is dropped
    /// instead of pasting a second time.
    TimedOut { is_read: bool },
}

/// What becomes of a `paste` event.
pub enum Outcome<C> {
    /// The event belongs to the element it fired on, since iced did not read
    /// the clipboard for it.
    Element,
    /// The event of a paste shortcut. Its default must be prevented, and these
    /// reads take its text.
    Shortcut(Vec<Read>),
    /// A paste from the browser's Edit menu into this canvas. Its default must
    /// be prevented, and the canvas is sent a Paste key press, whose read
    /// takes the text.
    Menu(C),
}

impl Paste {
    /// A key or pointer press ends the last paste shortcut. Returns the reads
    /// still waiting for its `paste` event, which will not come anymore.
    pub fn interrupt(&mut self) -> Vec<Read> {
        self.text = None;

        match mem::take(&mut self.shortcut) {
            Shortcut::Expected { reads } => reads,
            Shortcut::Idle | Shortcut::TimedOut { .. } => Vec::new(),
        }
    }

    /// A paste shortcut was pressed, after [`Paste::interrupt`] ended the last
    /// one. Returns its identifier for [`Paste::time_out`].
    pub fn press(&mut self) -> u64 {
        let reads = match mem::take(&mut self.shortcut) {
            Shortcut::Expected { reads } => reads,
            Shortcut::Idle | Shortcut::TimedOut { .. } => Vec::new(),
        };

        self.count += 1;
        self.shortcut = Shortcut::Expected { reads };
        self.count
    }

    /// Takes the text of a `paste` event that no read has taken yet.
    pub fn take_text(&mut self) -> Option<String> {
        self.text.take()
    }

    /// Makes `read` wait for the `paste` event of the paste shortcut. Returns
    /// it when no event is expected and it has to go through `readText`.
    pub fn wait(&mut self, read: Read) -> Option<Read> {
        let Shortcut::Expected { reads } = &mut self.shortcut else {
            return Some(read);
        };

        reads.push(read);
        None
    }

    /// The paste shortcut `shortcut` waited too long for its `paste` event.
    /// Returns the reads that fall back to `readText`.
    ///
    /// The shortcut stays read, so its `paste` event, if it comes after all, is
    /// taken and dropped instead of pasting a second time.
    pub fn time_out(&mut self, shortcut: u64) -> Vec<Read> {
        if shortcut != self.count {
            return Vec::new();
        }

        let Shortcut::Expected { reads } = &mut self.shortcut else {
            return Vec::new();
        };

        let reads = mem::take(reads);

        self.shortcut = Shortcut::TimedOut {
            is_read: !reads.is_empty(),
        };

        reads
    }

    /// A `paste` event fired, with `text`, at `target`.
    ///
    /// Only an event that no shortcut awaits comes from the Edit menu, so the
    /// shortcuts and their timeout behave the same whatever the menu does.
    pub fn paste<C>(&mut self, text: Option<&str>, target: Target<C>) -> Outcome<C> {
        match (mem::take(&mut self.shortcut), target) {
            (Shortcut::Idle, Target::Canvas(canvas)) if text.is_some() => {
                self.text = text.map(str::to_owned);

                Outcome::Menu(canvas)
            }
            // The input method's field commits a menu paste as text of its
            // own, and an element of the page pastes into itself.
            (Shortcut::Idle, _) => Outcome::Element,
            (Shortcut::Expected { reads }, Target::InputMethod(_) | Target::Page)
                if reads.is_empty() =>
            {
                self.shortcut = Shortcut::Expected { reads };

                Outcome::Element
            }
            (Shortcut::Expected { reads }, _) => {
                if reads.is_empty() {
                    self.text = text.map(str::to_owned);
                }

                Outcome::Shortcut(reads)
            }
            (Shortcut::TimedOut { is_read: true }, _) => Outcome::Shortcut(Vec::new()),
            (Shortcut::TimedOut { is_read: false }, Target::Canvas(_)) => {
                self.text = text.map(str::to_owned);

                Outcome::Shortcut(Vec::new())
            }
            (shortcut @ Shortcut::TimedOut { is_read: false }, _) => {
                self.shortcut = shortcut;

                Outcome::Element
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::rc::Rc;

    type Results = Rc<RefCell<Vec<Result<Content, Error>>>>;

    /// What the web clipboard does with a `paste` event.
    #[derive(Debug, PartialEq, Eq)]
    enum Effect {
        /// The element pastes on its own.
        Element,
        /// The event is cancelled, and this many reads take its text.
        Shortcut(usize),
        /// The event is cancelled, and the canvas is sent a Paste key press.
        Menu,
    }

    /// Fires a `paste` event with `text` and fulfills the reads it takes.
    fn paste(paste: &mut Paste, text: &str, target: Target<()>) -> Effect {
        match paste.paste(Some(text), target) {
            Outcome::Element => Effect::Element,
            Outcome::Shortcut(reads) => {
                let count = reads.len();
                fulfill(reads, text);
                Effect::Shortcut(count)
            }
            Outcome::Menu(()) => Effect::Menu,
        }
    }

    /// Reads the clipboard like the web clipboard does. Returns whether the
    /// read goes through `readText`.
    fn read(paste: &mut Paste, results: &Results) -> bool {
        if let Some(text) = paste.take_text() {
            results.borrow_mut().push(Ok(Content::Text(text)));
            return false;
        }

        paste.wait(callback(results)).is_some()
    }

    fn callback(results: &Results) -> Read {
        let results = results.clone();

        Box::new(move |result| results.borrow_mut().push(result))
    }

    fn fulfill(reads: Vec<Read>, text: &str) {
        for pending in reads {
            pending(Ok(Content::Text(text.to_owned())));
        }
    }

    fn texts(results: &Results) -> Vec<String> {
        results
            .borrow()
            .iter()
            .map(|result| match result {
                Ok(Content::Text(text)) => text.clone(),
                result => panic!("unexpected read: {result:?}"),
            })
            .collect()
    }

    #[test]
    fn a_shortcut_pastes_once_from_its_event() {
        let results = Results::default();
        let mut paste = Paste::default();

        let shortcut = paste.press();
        assert!(!read(&mut paste, &results));

        assert_eq!(
            self::paste(&mut paste, "test", Target::InputMethod(())),
            Effect::Shortcut(1)
        );

        assert!(paste.time_out(shortcut).is_empty());
        assert!(paste.interrupt().is_empty());
        assert_eq!(texts(&results), ["test"]);
    }

    #[test]
    fn a_shortcut_on_the_canvas_pastes_once_from_its_event() {
        let results = Results::default();
        let mut paste = Paste::default();

        let _ = paste.press();
        assert!(!read(&mut paste, &results));

        assert_eq!(
            self::paste(&mut paste, "test", Target::Canvas(())),
            Effect::Shortcut(1)
        );
        assert_eq!(texts(&results), ["test"]);
        assert!(read(&mut paste, &results));
    }

    #[test]
    fn a_late_event_still_fulfills_the_shortcut() {
        // Safari: timers run between the key press and its `paste` event.
        let results = Results::default();
        let mut paste = Paste::default();

        let shortcut = paste.press();
        assert!(!read(&mut paste, &results));
        assert!(paste.time_out(shortcut - 1).is_empty());

        assert_eq!(
            self::paste(&mut paste, "outside", Target::Page),
            Effect::Shortcut(1)
        );

        assert!(paste.time_out(shortcut).is_empty());
        assert_eq!(texts(&results), ["outside"]);
    }

    #[test]
    fn an_event_after_the_timeout_is_taken_and_dropped() {
        let results = Results::default();
        let mut paste = Paste::default();

        let shortcut = paste.press();
        assert!(!read(&mut paste, &results));
        assert_eq!(paste.time_out(shortcut).len(), 1);

        // The event must not reach the input method's field, which would
        // paste a second time.
        assert_eq!(
            self::paste(&mut paste, "test", Target::InputMethod(())),
            Effect::Shortcut(0)
        );

        assert!(read(&mut paste, &results));
        assert!(results.borrow().is_empty());
    }

    #[test]
    fn an_event_after_the_timeout_on_the_canvas_is_dropped_too() {
        let results = Results::default();
        let mut paste = Paste::default();

        let shortcut = paste.press();
        assert!(!read(&mut paste, &results));
        assert_eq!(paste.time_out(shortcut).len(), 1);

        assert_eq!(
            self::paste(&mut paste, "test", Target::Canvas(())),
            Effect::Shortcut(0)
        );
        assert!(read(&mut paste, &results));
    }

    #[test]
    fn the_next_press_ends_a_shortcut_without_event() {
        let results = Results::default();
        let mut paste = Paste::default();

        let _ = paste.press();
        assert!(!read(&mut paste, &results));

        assert_eq!(paste.interrupt().len(), 1);
        assert_eq!(
            self::paste(&mut paste, "test", Target::InputMethod(())),
            Effect::Element
        );
    }

    #[test]
    fn a_paste_into_another_element_belongs_to_it() {
        let mut paste = Paste::default();

        let _ = paste.press();
        assert_eq!(
            self::paste(&mut paste, "test", Target::Page),
            Effect::Element
        );
    }

    #[test]
    fn a_read_after_the_event_takes_its_text() {
        let results = Results::default();
        let mut paste = Paste::default();

        let _ = paste.press();
        assert_eq!(
            self::paste(&mut paste, "test", Target::Canvas(())),
            Effect::Shortcut(0)
        );

        assert!(!read(&mut paste, &results));
        assert_eq!(texts(&results), ["test"]);

        assert!(read(&mut paste, &results));
    }

    #[test]
    fn a_read_without_shortcut_reads_the_clipboard() {
        // A paste command of the application has no `paste` event.
        let results = Results::default();
        let mut paste = Paste::default();

        assert!(read(&mut paste, &results));
    }

    #[test]
    fn a_menu_paste_into_the_canvas_is_read_by_its_paste_key() {
        let results = Results::default();
        let mut paste = Paste::default();

        assert_eq!(
            self::paste(&mut paste, "menu", Target::Canvas(())),
            Effect::Menu
        );

        // The Paste key press reads the clipboard during its iced turn.
        assert!(!read(&mut paste, &results));
        assert_eq!(texts(&results), ["menu"]);

        assert!(read(&mut paste, &results));
    }

    #[test]
    fn a_menu_paste_without_text_is_left_to_the_canvas() {
        let mut paste = Paste::default();

        assert!(matches!(
            paste.paste(None, Target::Canvas(())),
            Outcome::Element
        ));
    }

    #[test]
    fn a_menu_paste_into_the_input_method_inserts_once() {
        // The field commits the text itself. Taking the event as well would
        // paste a second time through the Paste key's read.
        let results = Results::default();
        let mut paste = Paste::default();

        assert_eq!(
            self::paste(&mut paste, "menu", Target::InputMethod(())),
            Effect::Element
        );

        assert!(read(&mut paste, &results));
        assert!(results.borrow().is_empty());
    }

    #[test]
    fn a_menu_paste_into_another_element_belongs_to_it() {
        let mut paste = Paste::default();

        assert_eq!(
            self::paste(&mut paste, "menu", Target::Page),
            Effect::Element
        );
    }

    #[test]
    fn a_menu_paste_after_the_event_of_an_unread_shortcut() {
        let results = Results::default();
        let mut paste = Paste::default();

        let shortcut = paste.press();
        assert_eq!(
            self::paste(&mut paste, "shortcut", Target::Canvas(())),
            Effect::Shortcut(0)
        );
        assert!(paste.time_out(shortcut).is_empty());

        // The menu is reached without a key or pointer press on the page.
        assert_eq!(
            self::paste(&mut paste, "menu", Target::Canvas(())),
            Effect::Menu
        );
        assert!(!read(&mut paste, &results));
        assert_eq!(texts(&results), ["menu"]);
    }

    #[test]
    fn a_menu_paste_after_the_event_of_a_read_shortcut() {
        let results = Results::default();
        let mut paste = Paste::default();

        let _ = paste.press();
        assert!(!read(&mut paste, &results));
        assert_eq!(
            self::paste(&mut paste, "shortcut", Target::InputMethod(())),
            Effect::Shortcut(1)
        );

        assert_eq!(
            self::paste(&mut paste, "menu", Target::Canvas(())),
            Effect::Menu
        );
        assert!(!read(&mut paste, &results));
        assert_eq!(texts(&results), ["shortcut", "menu"]);
    }

    #[test]
    fn a_late_event_of_an_unread_shortcut_is_not_a_menu_paste() {
        // Safari: an unread shortcut times out before its event comes, and
        // the event looks like a menu paste. It stays the shortcut's.
        let results = Results::default();
        let mut paste = Paste::default();

        let shortcut = paste.press();
        assert!(paste.time_out(shortcut).is_empty());

        assert_eq!(
            self::paste(&mut paste, "late", Target::Canvas(())),
            Effect::Shortcut(0)
        );

        // A widget that reads it now takes it, as before the timeout.
        assert!(!read(&mut paste, &results));
        assert_eq!(texts(&results), ["late"]);
    }

    #[test]
    fn a_pending_shortcut_on_the_canvas_is_not_a_menu_paste() {
        let mut paste = Paste::default();

        let _ = paste.press();

        assert_eq!(
            self::paste(&mut paste, "shortcut", Target::Canvas(())),
            Effect::Shortcut(0)
        );
    }
}
