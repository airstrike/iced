//! The paste shortcuts of the web, fulfilled by the `paste` events the browser
//! fires for them.
//!
//! The browser's `paste` event carries the clipboard without asking the user,
//! while `navigator.clipboard.readText` asks for permission in most browsers.
//! A read made for a paste shortcut therefore waits for the shortcut's `paste`
//! event. Safari delivers that event after a round trip to its UI process,
//! once timers have run, so the wait ends with the event itself, the next key
//! or pointer press, or a timeout; only the last two fall back to `readText`.
use crate::core::clipboard::{Content, Error};

use std::mem;

/// A pending clipboard read.
pub type Read = Box<dyn FnOnce(Result<Content, Error>)>;

/// How long a paste shortcut waits for its `paste` event before its reads
/// fall back to `readText`, in milliseconds. Only browsers that never fire the
/// event reach it; a late event is still taken (see [`Paste::time_out`]).
#[cfg(target_arch = "wasm32")]
pub const TIMEOUT: i32 = 500;

/// The last paste shortcut, until the next key or pointer press.
#[derive(Default)]
pub struct Paste {
    /// Counts the paste shortcuts, so a timeout only ends its own.
    shortcut: u64,
    /// A paste shortcut was pressed and its `paste` event has not fired yet.
    is_expected: bool,
    /// The clipboard was read for the paste shortcut.
    is_read: bool,
    /// The text of a `paste` event that no read has taken yet.
    text: Option<String>,
    /// The reads waiting for the `paste` event of the shortcut.
    reads: Vec<Read>,
}

impl Paste {
    /// A key or pointer press ends the last paste shortcut. Returns the reads
    /// still waiting for its `paste` event, which will not come anymore.
    pub fn interrupt(&mut self) -> Vec<Read> {
        self.is_expected = false;
        self.is_read = false;
        self.text = None;

        mem::take(&mut self.reads)
    }

    /// A paste shortcut was pressed, after [`Paste::interrupt`] ended the last
    /// one. Returns its identifier for [`Paste::time_out`].
    pub fn press(&mut self) -> u64 {
        self.shortcut += 1;
        self.is_expected = true;
        self.shortcut
    }

    /// Takes the text of a `paste` event that no read has taken yet.
    pub fn take_text(&mut self) -> Option<String> {
        self.text.take()
    }

    /// Makes `read` wait for the `paste` event of the paste shortcut. Returns
    /// it when no event is expected and it has to go through `readText`.
    pub fn wait(&mut self, read: Read) -> Option<Read> {
        if !self.is_expected {
            return Some(read);
        }

        self.is_read = true;
        self.reads.push(read);
        None
    }

    /// The paste shortcut `shortcut` waited too long for its `paste` event.
    /// Returns the reads that fall back to `readText`.
    ///
    /// The shortcut stays read, so its `paste` event, if it comes after all, is
    /// taken and dropped instead of pasting a second time.
    pub fn time_out(&mut self, shortcut: u64) -> Vec<Read> {
        if !self.is_expected || shortcut != self.shortcut {
            return Vec::new();
        }

        self.is_expected = false;
        mem::take(&mut self.reads)
    }

    /// A `paste` event fired, with `text`, into the canvas or another element.
    ///
    /// Returns `None` when the event belongs to that element, since the
    /// clipboard was not read for it. Otherwise the event is taken (its
    /// default must be prevented), and the reads it fulfills are returned.
    pub fn paste(&mut self, text: Option<&str>, is_canvas: bool) -> Option<Vec<Read>> {
        if !is_canvas && !self.is_read {
            return None;
        }

        let has_timed_out = self.is_read && !self.is_expected;
        self.is_expected = false;

        let reads = mem::take(&mut self.reads);

        if reads.is_empty() && !has_timed_out {
            self.text = text.map(str::to_owned);
        }

        Some(reads)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::rc::Rc;

    type Results = Rc<RefCell<Vec<Result<Content, Error>>>>;

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

        let reads = paste.paste(Some("test"), false).expect("taken");
        fulfill(reads, "test");

        assert!(paste.time_out(shortcut).is_empty());
        assert!(paste.interrupt().is_empty());
        assert_eq!(texts(&results), ["test"]);
    }

    #[test]
    fn a_late_event_still_fulfills_the_shortcut() {
        // Safari: timers run between the key press and its `paste` event.
        let results = Results::default();
        let mut paste = Paste::default();

        let shortcut = paste.press();
        assert!(!read(&mut paste, &results));
        assert!(paste.time_out(shortcut - 1).is_empty());

        let reads = paste.paste(Some("outside"), false).expect("taken");
        assert_eq!(reads.len(), 1);
        fulfill(reads, "outside");

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
        let reads = paste.paste(Some("test"), false).expect("taken");
        assert!(reads.is_empty());

        assert!(read(&mut paste, &results));
    }

    #[test]
    fn the_next_press_ends_a_shortcut_without_event() {
        let results = Results::default();
        let mut paste = Paste::default();

        let _ = paste.press();
        assert!(!read(&mut paste, &results));

        assert_eq!(paste.interrupt().len(), 1);
        assert!(paste.paste(Some("test"), false).is_none());
    }

    #[test]
    fn a_paste_into_another_element_belongs_to_it() {
        let mut paste = Paste::default();

        let _ = paste.press();
        assert!(paste.paste(Some("test"), false).is_none());
    }

    #[test]
    fn a_read_after_the_event_takes_its_text() {
        let results = Results::default();
        let mut paste = Paste::default();

        let _ = paste.press();
        let reads = paste.paste(Some("test"), true).expect("taken");
        assert!(reads.is_empty());

        assert!(!read(&mut paste, &results));
        assert_eq!(texts(&results), ["test"]);

        assert!(read(&mut paste, &results));
    }

    #[test]
    fn a_read_without_shortcut_reads_the_clipboard() {
        // A "Paste" menu item has no `paste` event.
        let results = Results::default();
        let mut paste = Paste::default();

        assert!(read(&mut paste, &results));
    }
}
