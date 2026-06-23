//! Why a `Fit` box does not shield a `Fill` child from a `Shrink` ancestor.
//!
//! A container rewrites its own width to `width.stack(child.width)` in
//! `container::diff`, and in that merge **Fill wins over Fit** (see
//! `Length::merge_with`). So a `Fit` box wrapping a `Fill` child *becomes* a
//! `Fill` box: it forwards whatever sizing its parent dictates instead of
//! imposing `Fit`. `Shrink` and `Fixed` do not absorb, so they keep their own
//! width. These tests pin that behavior at the `Length` level and end-to-end
//! through the layout engine. They back `examples/fit_shrink_propagation`.
use iced::widget::{Column, Grid, container, text};
use iced::{Element, Fill, Fit, Length, Renderer, Shrink, Theme};
use iced_test::selector::id;
use iced_test::simulator;

/// `true` when two resolved widths are within a pixel of each other.
fn same(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0
}

/// Lays `subject` into a 400px frame and returns the width of the leaf tagged
/// `"fill"` inside it.
fn fill_width(subject: Element<'static, (), Theme, Renderer>) -> f32 {
    let view = container(subject).width(400.0).padding(5);
    let mut ui = simulator(view);
    ui.find(id("fill")).expect("fill leaf").bounds().width
}

fn fill_leaf() -> Element<'static, (), Theme, Renderer> {
    container(text("Fill"))
        .width(Fill)
        .padding(5)
        .id("fill")
        .into()
}

#[test]
fn stack_absorbs_fill_into_fit_only() {
    // The mechanism, stated directly: only Fit absorbs a Fill child.
    assert_eq!(Length::Fit.stack(Length::Fill), Length::Fill);
    assert_eq!(Length::Shrink.stack(Length::Fill), Length::Shrink);
    assert_eq!(
        Length::Fixed(200.0).stack(Length::Fill),
        Length::Fixed(200.0)
    );
    assert_eq!(Length::Fill.stack(Length::Fill), Length::Fill);
}

#[test]
fn fit_box_does_not_shield_fill_from_shrink() {
    let fit_top = fill_width(container(fill_leaf()).width(Fit).into());
    let shrink_direct = fill_width(container(fill_leaf()).width(Shrink).into());
    let shrink_fit = fill_width(
        container(container(fill_leaf()).width(Fit))
            .width(Shrink)
            .into(),
    );
    let shrink_fixed = fill_width(
        container(container(fill_leaf()).width(200.0))
            .width(Shrink)
            .into(),
    );

    // A lone Fit box fills; a lone Shrink box collapses the Fill to its content.
    assert!(fit_top > 300.0, "Fit should fill, got {fit_top}");
    assert!(
        shrink_direct < 40.0,
        "Shrink should collapse, got {shrink_direct}"
    );

    // Inserting a Fit box under the Shrink changes nothing: it absorbed to Fill.
    assert!(
        same(shrink_fit, shrink_direct),
        "Fit box must not shield: {shrink_fit} vs bare {shrink_direct}"
    );

    // A Fixed box does not absorb, so it shields: the Fill fills its pinned width.
    assert!(
        shrink_fixed > 150.0,
        "Fixed should shield, got {shrink_fixed}"
    );
}

#[test]
fn compression_reaches_the_leaf_through_a_fit_box() {
    // A Grid with no explicit width returns Size::ZERO iff it receives
    // compression == true (grid.rs). So a surviving (non-zero) grid means the
    // compression flag did not reach it.
    fn grid_survivor_width(subject: Element<'static, (), Theme, Renderer>) -> f32 {
        let view = container(subject).width(400.0).padding(5);
        let mut ui = simulator(view);
        ui.find(id("outer")).expect("outer").bounds().width
    }
    fn probe() -> Element<'static, (), Theme, Renderer> {
        Grid::with_children([container(text("XX")).padding(5).into()]).into()
    }

    let under_fit = grid_survivor_width(container(probe()).width(Fit).id("outer").into());
    let under_shrink = grid_survivor_width(container(probe()).width(Shrink).id("outer").into());
    let under_shrink_then_fit = grid_survivor_width(
        container(container(probe()).width(Fit))
            .width(Shrink)
            .id("outer")
            .into(),
    );

    assert!(
        under_fit > 100.0,
        "Fit: grid should survive, got {under_fit}"
    );
    assert!(
        under_shrink < 1.0,
        "Shrink: grid should collapse, got {under_shrink}"
    );
    // The Fit box did not reset the flag for its child: the grid still collapsed.
    assert!(
        under_shrink_then_fit < 1.0,
        "Shrink > Fit: grid still collapses, got {under_shrink_then_fit}"
    );
}

#[test]
fn shrink_column_of_all_fill_settles_on_widest_content() {
    // A compressing-cross column whose children are ALL Fill-cross has nothing
    // to set its running cross. Rather than collapse to 0 or fill the 400px slot,
    // the flex seeds the cross from the widest child's own content and stretches
    // every child to it -- so all three settle on Cassidy's width.
    let children: Vec<Element<'static, (), Theme, Renderer>> = vec![
        container(text("Alice"))
            .width(Fill)
            .padding(8)
            .id("alice")
            .into(),
        container(text("Bo")).width(Fill).padding(8).id("bo").into(),
        container(text("Cassidy"))
            .width(Fill)
            .padding(8)
            .id("cassidy")
            .into(),
    ];
    let col = Column::with_children(children).width(Shrink).spacing(8);
    let view = container(col).width(400.0).padding(5);
    let mut ui = simulator(view);
    let alice = ui.find(id("alice")).expect("alice").bounds().width;
    let bo = ui.find(id("bo")).expect("bo").bounds().width;
    let cassidy = ui.find(id("cassidy")).expect("cassidy").bounds().width;

    // All three stretch to the same width: the widest content, well short of 400.
    assert!(
        same(alice, cassidy),
        "Alice should match Cassidy: {alice} vs {cassidy}"
    );
    assert!(
        same(bo, cassidy),
        "Bo should match Cassidy: {bo} vs {cassidy}"
    );
    assert!(
        cassidy > 40.0 && cassidy < 150.0,
        "should hug the widest content, not the slot, got {cassidy}"
    );
}

#[test]
fn fit_absorbs_fill_but_shrink_does_not() {
    // A `box_width` flex (a row, then a column), holding Alice plus two Fit
    // anchors. A Fit box yields to a Fill Alice (fills its 400px slot); the same
    // Fit box hugs a Fit Alice; a Shrink box overrides Fill and collapses.
    fn box_width(row: bool, wrapper: Length, alice: Length) -> f32 {
        let children: Vec<Element<'static, (), Theme, Renderer>> = vec![
            container(text("Alice")).width(alice).padding(8).into(),
            container(text("Bo")).width(Fit).padding(8).into(),
            container(text("Cassidy")).width(Fit).padding(8).into(),
        ];
        let inner: Element<'static, (), Theme, Renderer> = if row {
            iced::widget::Row::with_children(children)
                .width(wrapper)
                .spacing(8)
                .into()
        } else {
            Column::with_children(children)
                .width(wrapper)
                .spacing(8)
                .into()
        };
        let view = container(container(inner).padding(8).id("box"))
            .width(400.0)
            .padding(5);
        let mut ui = simulator(view);
        ui.find(id("box")).expect("box").bounds().width
    }

    for row in [true, false] {
        let axis = if row { "row" } else { "column" };
        let fit_fills = box_width(row, Fit, Fill);
        let fit_hugs = box_width(row, Fit, Fit);
        let shrink_collapses = box_width(row, Shrink, Fill);

        assert!(
            fit_fills > 300.0,
            "{axis}: Fit box + Fill child fills, got {fit_fills}"
        );
        assert!(
            fit_hugs < 280.0,
            "{axis}: Fit box + Fit child hugs, got {fit_hugs}"
        );
        assert!(
            shrink_collapses < 280.0,
            "{axis}: Shrink box + Fill child collapses, got {shrink_collapses}"
        );
        assert!(
            fit_fills > shrink_collapses + 80.0,
            "{axis}: Fit absorbs but Shrink does not: {fit_fills} vs {shrink_collapses}"
        );
    }
}

#[test]
fn fill_among_anchors_in_a_row() {
    // Alice (Fill) flanked by two Fit anchors, in a row of the given outer width.
    fn alice_width(outer: Length) -> f32 {
        let children: Vec<Element<'static, (), Theme, Renderer>> = vec![
            container(text("Alice"))
                .width(Fill)
                .padding(5)
                .id("alice")
                .into(),
            container(text("Bob")).width(Fit).padding(5).into(),
            container(text("Charlie")).width(Fit).padding(5).into(),
        ];
        let r = iced::widget::Row::with_children(children)
            .width(outer)
            .spacing(8);
        let view = container(r).width(400.0).padding(5);
        let mut ui = simulator(view);
        ui.find(id("alice")).expect("alice").bounds().width
    }

    let fit = alice_width(Fit);
    let shrink = alice_width(Shrink);

    // Under Fit the row fills and Alice grabs the leftover; under Shrink she
    // collapses to her own content. The anchors make the difference legible.
    assert!(
        fit > 200.0,
        "Alice should grab the leftover under Fit, got {fit}"
    );
    assert!(
        shrink < 60.0,
        "Alice should collapse under Shrink, got {shrink}"
    );
    assert!(
        fit > shrink + 100.0,
        "Fit and Shrink must visibly differ: {fit} vs {shrink}"
    );
}

#[test]
fn flex_sizing_matches_container_sizing() {
    use iced::widget::row;

    // Sizing on the container itself (current) vs sizing on a single-child row,
    // with the container left at its Fit default so it only paints/pads.
    let by_container = |w| container(fill_leaf()).width(w).padding(5);
    let by_flex = |w| container(row![fill_leaf()].width(w)).padding(5);

    for w in [Fit, Shrink, Fill] {
        let bare_c = fill_width(by_container(w).into());
        let bare_f = fill_width(by_flex(w).into());
        let nested_c = fill_width(container(by_container(w)).width(Shrink).into());
        let nested_f = fill_width(container(by_flex(w)).width(Shrink).into());

        assert!(
            same(bare_c, bare_f),
            "{w:?} bare: container={bare_c} flex={bare_f}"
        );
        assert!(
            same(nested_c, nested_f),
            "{w:?} under Shrink: container={nested_c} flex={nested_f}"
        );
    }
}

#[test]
fn column_child_is_cross_axis() {
    // Mirror the example's column rows: a Fill leaf as a column's child sizes on
    // the cross axis, so under Shrink it collapses to zero with no sibling, and
    // hugs the sibling's width when one is present.
    fn boxed(
        content: Element<'static, (), Theme, Renderer>,
        w: Length,
    ) -> Element<'static, (), Theme, Renderer> {
        container(content).width(w).padding(5).into()
    }
    fn col(
        children: Vec<Element<'static, (), Theme, Renderer>>,
    ) -> Element<'static, (), Theme, Renderer> {
        container(
            Column::with_children(children)
                .width(Fill)
                .spacing(8)
                .padding(5),
        )
        .width(Fill)
        .into()
    }
    fn row_width(subject: Element<'static, (), Theme, Renderer>) -> f32 {
        let labeled = Column::with_children([text("name").into(), subject]).width(Fill);
        let view = container(labeled).width(400.0).padding(5);
        let mut ui = simulator(view);
        ui.find(id("fill")).expect("fill leaf").bounds().width
    }

    let lone = row_width(boxed(col(vec![fill_leaf()]), Shrink));
    let with_sibling = row_width(boxed(
        col(vec![
            container(text("SIBLING")).padding(5).into(),
            fill_leaf(),
        ]),
        Shrink,
    ));

    // A Fill-cross child settles on the widest content in its column: its own
    // when alone, or a wider sibling's when one is present.
    assert!(
        lone > 20.0 && lone < 60.0,
        "lone Fill hugs its own content, got {lone}"
    );
    assert!(
        with_sibling > lone,
        "with a wider sibling the Fill stretches to it, got {with_sibling} vs lone {lone}"
    );
}
