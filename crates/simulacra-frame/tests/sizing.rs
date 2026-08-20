//! The sizing policy, which is the whole of the fix.
//!
//! A texture a sprite is drawing cannot be replaced behind its handle — the engine's bind-group
//! cache is keyed by the handle's id and built once, so a replacement is never noticed and the
//! picture freezes. So a new size means a new handle, and a new handle means a texture that is
//! never freed. Everything therefore rests on one property: **a resize must almost never need a
//! new size.** That is what is checked here.

use simulacra_frame::frame_size;

#[test]
fn the_first_allocation_takes_the_largest_display() {
    // So that going fullscreen later is not an allocation at all.
    assert_eq!(frame_size((0, 0), (1600, 1000), (2560, 1440)), (2560, 1440));
}

#[test]
fn it_is_never_smaller_than_the_window() {
    // Including on a machine that reports no monitors, which is what a headless run looks like.
    assert_eq!(frame_size((0, 0), (1600, 1000), (0, 0)), (1600, 1000));
    // And never zero, which would not be a legal texture.
    let empty = frame_size((0, 0), (0, 0), (0, 0));
    assert!(
        empty.0 >= 1 && empty.1 >= 1,
        "a zero-sized texture: {empty:?}"
    );
}

#[test]
fn a_drag_never_reallocates() {
    // The case that matters. Dragging an edge produces a new window size every frame; if each
    // one allocated, a few seconds of dragging would leak hundreds of full-screen textures.
    let display = (2560, 1440);
    let first = frame_size((0, 0), (1600, 1000), display);
    let mut size = first;
    for width in (320..=2560).step_by(3) {
        for height in [240, 700, 1080, 1440] {
            size = frame_size(size, (width, height), display);
            assert_eq!(
                size, first,
                "a window of {width}x{height} reallocated the frame"
            );
        }
    }
}

#[test]
fn going_fullscreen_and_back_never_reallocates() {
    let display = (1920, 1080);
    let first = frame_size((0, 0), (1280, 800), display);
    let mut size = first;
    for window in [(1920, 1080), (1280, 800), (1920, 1080), (640, 480)] {
        size = frame_size(size, window, display);
        assert_eq!(size, first, "{window:?} reallocated the frame");
    }
}

#[test]
fn it_grows_for_a_window_bigger_than_any_display_and_only_on_the_axis_that_needed_it() {
    // A window spanning two monitors, or a scale factor the monitor list did not account for.
    let display = (2560, 1440);
    let size = frame_size((0, 0), (1600, 1000), display);
    let grown = frame_size(size, (3000, 1200), display);
    assert_eq!(grown, (3000, 1440));
    assert_eq!(
        frame_size(grown, (800, 600), display),
        grown,
        "it shrank back again"
    );
}

#[test]
fn it_always_covers_the_window_it_was_asked_about() {
    let display = (1920, 1080);
    let mut size = frame_size((0, 0), (800, 600), display);
    for window in [
        (800, 600),
        (1920, 1080),
        (2560, 1440),
        (640, 480),
        (3840, 2160),
        (100, 100),
        (7680, 1080),
    ] {
        size = frame_size(size, window, display);
        assert!(
            size.0 >= window.0 && size.1 >= window.1,
            "a {window:?} window does not fit in a {size:?} frame"
        );
    }
}
