//! Tests for the `anchored` widget: showing, dismissing and hover intent.
use iced_test::Simulator;
use iced_test::simulator;
use iced_widget::anchored::Side;
use iced_widget::core::keyboard::key;
use iced_widget::core::time::Duration;
use iced_widget::core::{self, Point, mouse};
use iced_widget::{
    Anchored, Button, Renderer, Space, Theme, Widget, anchored, button, column, row, space,
};

#[derive(Debug, Clone, PartialEq)]
enum Message {
    Toggle(bool),
    Anchor,
    Inside,
    Below,
}

/// A 100x30 anchor at the origin and an 80x40 content block 4px under it,
/// centered: the content spans (10, 34) to (90, 74).
const ANCHOR: Point = Point::new(50.0, 15.0);
const INSIDE: Point = Point::new(50.0, 50.0);
const OUTSIDE: Point = Point::new(500.0, 500.0);

fn block<'a>(width: f32, height: f32, message: Message) -> Block<'a> {
    button(space())
        .width(width)
        .height(height)
        .on_press(message)
}

type Block<'a> = Button<'a, Message, Space, Theme>;

fn popover<'a>() -> Anchored<'a, Block<'a>, Block<'a>, Message, Theme> {
    anchored(
        block(100.0, 30.0, Message::Anchor),
        block(80.0, 40.0, Message::Inside),
    )
    .side(Side::Bottom)
    .offset(4)
    .on_toggle(Message::Toggle)
}

fn page<'a>(
    anchored: impl Widget<Message, Theme, Renderer> + 'a,
) -> impl Widget<Message, Theme, Renderer> + 'a {
    // A button under the content, to see whether presses reach the page.
    column![
        row![anchored],
        space().height(20),
        block(200.0, 200.0, Message::Below)
    ]
}

fn press_at(ui: &mut Simulator<'_, Message, Theme, Renderer>, at: Point) -> Vec<Message> {
    ui.point_at(at);
    let _ = ui.simulate([core::Event::Mouse(mouse::Event::CursorMoved {
        position: at,
    })]);
    let _ = ui.simulate(simulator::click());

    ui.drain().collect()
}

fn move_to(ui: &mut Simulator<'_, Message, Theme, Renderer>, at: Point) -> Vec<Message> {
    ui.point_at(at);
    let _ = ui.simulate([core::Event::Mouse(mouse::Event::CursorMoved {
        position: at,
    })]);

    ui.drain().collect()
}

#[test]
fn a_press_outside_asks_to_close() {
    let mut ui = Simulator::new(page(popover().open(true)));

    assert_eq!(press_at(&mut ui, OUTSIDE), vec![Message::Toggle(false)]);
}

#[test]
fn a_press_inside_reaches_the_content_and_keeps_it_open() {
    let mut ui = Simulator::new(page(popover().open(true)));

    assert_eq!(press_at(&mut ui, INSIDE), vec![Message::Inside]);
}

#[test]
fn the_content_covers_what_lies_under_it() {
    // The content's corner overlaps the page's lower button, which starts at
    // y = 50 (30 + 20): a press there belongs to the content alone.
    let mut ui = Simulator::new(page(
        anchored(
            block(100.0, 30.0, Message::Anchor),
            space().width(80).height(40),
        )
        .offset(4)
        .on_toggle(Message::Toggle),
    ));

    assert_eq!(press_at(&mut ui, Point::new(50.0, 60.0)), vec![]);
}

#[test]
fn a_press_on_the_anchor_is_left_to_the_anchor() {
    let mut ui = Simulator::new(page(popover().open(true)));

    assert_eq!(press_at(&mut ui, ANCHOR), vec![Message::Anchor]);
}

#[test]
fn a_modal_treats_the_anchor_as_outside() {
    let mut ui = Simulator::new(page(popover().open(true).modal(true)));

    assert_eq!(press_at(&mut ui, ANCHOR), vec![Message::Toggle(false)]);
}

#[test]
fn a_modal_blocks_the_page() {
    let mut ui = Simulator::new(page(popover().open(true).modal(true)));

    assert_eq!(
        press_at(&mut ui, Point::new(150.0, 200.0)),
        vec![Message::Toggle(false)]
    );
}

#[test]
fn a_press_outside_a_non_modal_reaches_the_page() {
    let mut ui = Simulator::new(page(popover().open(true)));

    assert_eq!(
        press_at(&mut ui, Point::new(150.0, 200.0)),
        vec![Message::Toggle(false), Message::Below]
    );
}

#[test]
fn escape_asks_to_close() {
    let mut ui = Simulator::new(page(popover().open(true)));

    let _ = ui.tap_key(key::Named::Escape);

    assert_eq!(ui.drain().collect::<Vec<_>>(), vec![Message::Toggle(false)]);
}

#[test]
fn closed_content_is_not_there() {
    let mut ui = Simulator::new(page(popover().open(false)));

    assert_eq!(
        press_at(&mut ui, INSIDE),
        vec![Message::Below],
        "the page is uncovered"
    );
    assert_eq!(press_at(&mut ui, OUTSIDE), vec![]);

    let _ = ui.tap_key(key::Named::Escape);
    assert_eq!(ui.drain().collect::<Vec<_>>(), vec![]);
}

#[test]
fn hover_opens_and_survives_the_gap_into_the_content() {
    let mut ui = Simulator::new(page(popover().hover(Duration::ZERO, Duration::ZERO)));

    assert_eq!(move_to(&mut ui, ANCHOR), vec![Message::Toggle(true)]);

    // Across the 4px gap and into the content.
    assert_eq!(move_to(&mut ui, Point::new(50.0, 32.0)), vec![]);
    assert_eq!(move_to(&mut ui, INSIDE), vec![]);
    assert_eq!(press_at(&mut ui, INSIDE), vec![Message::Inside]);

    assert_eq!(move_to(&mut ui, OUTSIDE), vec![Message::Toggle(false)]);
}

#[test]
fn hover_waits_for_its_open_delay() {
    let mut ui = Simulator::new(page(
        popover().hover(Duration::from_secs(60), Duration::ZERO),
    ));

    assert_eq!(move_to(&mut ui, ANCHOR), vec![]);
    assert_eq!(
        press_at(&mut ui, INSIDE),
        vec![Message::Below],
        "nothing is open yet"
    );
}

#[test]
fn escape_closes_hover_content_until_the_pointer_leaves() {
    let mut ui = Simulator::new(page(popover().hover(Duration::ZERO, Duration::ZERO)));

    assert_eq!(move_to(&mut ui, ANCHOR), vec![Message::Toggle(true)]);

    let _ = ui.tap_key(key::Named::Escape);
    assert_eq!(ui.drain().collect::<Vec<_>>(), vec![Message::Toggle(false)]);

    // Still on the anchor: stays closed.
    assert_eq!(move_to(&mut ui, Point::new(52.0, 15.0)), vec![]);

    // Leave and come back: opens again.
    assert_eq!(move_to(&mut ui, OUTSIDE), vec![]);
    assert_eq!(move_to(&mut ui, ANCHOR), vec![Message::Toggle(true)]);
}

#[test]
fn a_point_anchor_places_the_content_at_the_point() {
    // Anchored at (60, 10) inside the anchor, start-aligned below it: the
    // content spans (60, 10) to (140, 50).
    let mut ui = Simulator::new(page(
        popover()
            .open(true)
            .at(Point::new(60.0, 10.0))
            .offset(0)
            .align(iced_widget::anchored::Align::Start),
    ));

    assert_eq!(
        press_at(&mut ui, Point::new(130.0, 45.0)),
        vec![Message::Inside]
    );
}

/// Content whose `Fill` button wraps a 40px block, under the 100px anchor.
fn fill_content<'a>(
    width: iced_widget::anchored::Width,
) -> Anchored<'a, Block<'a>, Block<'a>, Message, Theme> {
    anchored(
        block(100.0, 30.0, Message::Anchor),
        button(space().width(40).height(40))
            .width(iced_widget::core::Length::Fill)
            .on_press(Message::Inside),
    )
    .width(width)
    .offset(4)
    .open(true)
    .on_toggle(Message::Toggle)
}

#[test]
fn content_hugs_its_fill_children_by_default() {
    // Centered under the anchor: x 30..70 (plus the button's padding).
    let mut ui = Simulator::new(page(fill_content(iced_widget::anchored::Width::Content)));

    assert_eq!(
        press_at(&mut ui, Point::new(97.0, 50.0)),
        vec![Message::Toggle(false), Message::Below],
        "the press misses the content and reaches the page"
    );
}

#[test]
fn at_least_anchor_stretches_fill_children_to_the_anchor() {
    let mut ui = Simulator::new(page(fill_content(
        iced_widget::anchored::Width::AtLeastAnchor,
    )));

    assert_eq!(
        press_at(&mut ui, Point::new(97.0, 50.0)),
        vec![Message::Inside]
    );
}

#[test]
fn min_width_widens_the_content() {
    let mut ui = Simulator::new(page(
        fill_content(iced_widget::anchored::Width::Content).min_width(100),
    ));

    assert_eq!(
        press_at(&mut ui, Point::new(97.0, 50.0)),
        vec![Message::Inside]
    );
}
