use super::Clipboard;
use crate::core::clipboard::{Content, Error, Kind};

use std::sync::mpsc;

fn read(clipboard: &Clipboard, kind: Kind) -> Result<Content, Error> {
    let (sender, receiver) = mpsc::channel();
    clipboard.read(kind, move |result| {
        let _ = sender.send(result);
    });
    receiver.recv().expect("read calls its callback once")
}

fn write(clipboard: &mut Clipboard, content: Content) -> Result<(), Error> {
    let (sender, receiver) = mpsc::channel();
    clipboard.write(content, move |result| {
        let _ = sender.send(result);
    });
    receiver.recv().expect("write calls its callback once")
}

/// Writes the system pasteboard, so it runs only on request:
/// `cargo test -p iced_winit -- --ignored rich_content`.
#[test]
#[ignore = "writes the system pasteboard"]
fn rich_content_reads_back_as_html_and_as_text() {
    let mut clipboard = Clipboard::new();
    let before = read(&clipboard, Kind::Text);

    let html = "<table><tr><td>a &amp; b</td></tr></table>";
    let text = "a & b\n";
    write(
        &mut clipboard,
        Content::Rich {
            html: html.to_owned(),
            text: text.to_owned(),
        },
    )
    .unwrap();

    let Ok(Content::Html(read_html)) = read(&clipboard, Kind::Html) else {
        panic!("public.html is on the pasteboard");
    };
    let read_text = read(&clipboard, Kind::Text);

    if let Ok(Content::Text(before)) = before {
        let _ = write(&mut clipboard, Content::Text(before));
    }

    assert!(read_html.contains(html), "{read_html}");
    assert_eq!(read_text, Ok(Content::Text(text.to_owned())));
}
