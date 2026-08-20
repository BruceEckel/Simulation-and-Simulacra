//! The catalogue, and the arithmetic that puts it on screen.
//!
//! The catalogue is generated from the other packages' manifests, so the thing worth checking
//! is that the generation actually caught all of them: a viewer that quietly missed a
//! simulation would be worse than no viewer, because it would look complete.

use _viewer::{
    CATALOGUE, FOOTER_LINES, GAP_LINES, HEADING_LINES, Metrics, exe_name, layout, overlay, row_at,
    step_selection, survey,
};
use std::path::Path;

/// The line separator `overlay` carries a row down with, named rather than written inline so
/// that it reads as what it is.
const NEWLINE: char = '\n';

/// Every directory under `fulcrum/` that is a simulation — which is all of them except the
/// viewer itself, whose name begins with an underscore so that it sorts first.
fn family() -> Vec<String> {
    let fulcrum = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("fulcrum/_viewer has a parent");
    let mut names: Vec<String> = std::fs::read_dir(fulcrum)
        .expect("fulcrum/ is readable")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('_'))
        .collect();
    names.sort();
    names
}

#[test]
fn the_catalogue_holds_every_simulation_and_nothing_else() {
    let listed: Vec<String> = CATALOGUE
        .iter()
        .map(|piece| piece.name.to_string())
        .collect();
    assert_eq!(
        listed,
        family(),
        "the catalogue and fulcrum/ have drifted apart"
    );
}

#[test]
fn the_catalogue_does_not_list_the_viewer() {
    assert!(
        !CATALOGUE.iter().any(|piece| piece.name.starts_with('_')),
        "the front door should not be one of the rooms"
    );
}

#[test]
fn every_piece_says_what_it_is() {
    for piece in CATALOGUE {
        assert!(!piece.name.is_empty(), "a piece with no name");
        assert!(
            piece.about.len() > 20,
            "{} says almost nothing about itself: {:?}",
            piece.name,
            piece.about
        );
        assert!(
            !piece.about.contains('"'),
            "{}'s description would not survive being quoted",
            piece.name
        );
    }
}

#[test]
fn it_is_in_the_order_a_directory_listing_shows() {
    let names: Vec<&str> = CATALOGUE.iter().map(|piece| piece.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

// ---------------------------------------------------------------------------------------
// finding the executables
// ---------------------------------------------------------------------------------------

#[test]
fn a_piece_that_is_here_is_runnable_and_one_that_is_not_is_still_listed() {
    let root = std::env::temp_dir().join("simulacra-viewer-survey");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("a temp directory");
    // Exactly one of them is present.
    let present = CATALOGUE[0].name;
    std::fs::write(root.join(exe_name(present)), b"not really an executable").expect("write");

    let shelf = survey(Some(&root));
    assert_eq!(
        shelf.len(),
        CATALOGUE.len(),
        "everything is listed whether it is here or not"
    );
    assert!(
        shelf[0].runnable(),
        "{present} is here and should be runnable"
    );
    assert!(
        shelf[1..].iter().all(|listing| !listing.runnable()),
        "nothing else was put there"
    );

    // And with nowhere to look, everything is listed and nothing is runnable.
    let nowhere = survey(None);
    assert_eq!(nowhere.len(), CATALOGUE.len());
    assert!(nowhere.iter().all(|listing| !listing.runnable()));
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------------------
// where the list goes
// ---------------------------------------------------------------------------------------

/// Roughly what the built-in font measures at: very nearly square. Assuming the proportions of
/// an ordinary typeface here is exactly the mistake these tests exist to catch.
fn metrics() -> Metrics {
    Metrics {
        line: 1.03,
        advance: 1.04,
    }
}

#[test]
fn everything_fits_in_a_reasonable_window() {
    let rows = CATALOGUE.len();
    let columns = 150;
    for window in [(1600, 900), (1920, 1080), (2560, 1440), (1280, 800)] {
        let here = layout(window, rows, columns, metrics());
        let lines = HEADING_LINES + GAP_LINES + rows as f32 + GAP_LINES + FOOTER_LINES;
        assert!(
            here.row * lines <= window.1 as f32,
            "{lines} lines do not fit down a {window:?} window"
        );
        assert!(
            here.size * columns as f32 * metrics().advance <= window.0 as f32,
            "the longest row does not fit across a {window:?} window"
        );
        // The heading has room above the list for its two lines, so it cannot run off the top.
        assert!(
            here.top >= here.row * (HEADING_LINES + GAP_LINES),
            "the heading has nowhere to go in a {window:?} window"
        );
        assert!(here.size >= 6.0, "unreadable in a {window:?} window");
    }
}

#[test]
fn a_wider_window_does_not_make_a_smaller_list() {
    let rows = CATALOGUE.len();
    let narrow = layout((1000, 900), rows, 150, metrics());
    let wide = layout((2000, 900), rows, 150, metrics());
    assert!(
        wide.size >= narrow.size,
        "more room should not mean smaller text"
    );
}

#[test]
fn the_pointer_and_the_rows_agree() {
    // What the pointer reads must be the row the text is on, for every row: this is the
    // arithmetic that decides which program a click starts.
    let rows = CATALOGUE.len();
    let here = layout((1600, 900), rows, 150, metrics());
    for row in 0..rows {
        let middle = here.top + here.row * (row as f32 + 0.5);
        assert_eq!(
            row_at(&here, middle, rows),
            Some(row),
            "the middle of row {row} does not read back as row {row}"
        );
        // And the baseline the text is actually drawn at is inside that row's own band.
        let baseline = here.baseline_of(row as f32);
        assert!(
            baseline > here.top + here.row * row as f32
                && baseline <= here.top + here.row * (row + 1) as f32,
            "row {row} is drawn outside the band the pointer reads it in"
        );
    }
    assert_eq!(row_at(&here, here.top - 1.0, rows), None, "above the list");
    assert_eq!(
        row_at(&here, here.top + here.row * rows as f32 + 1.0, rows),
        None,
        "below the list"
    );
}

#[test]
fn the_heading_sits_above_the_list_and_the_notes_below_it() {
    let rows = CATALOGUE.len();
    let here = layout((1600, 900), rows, 150, metrics());
    assert!(
        here.baseline_of(-(HEADING_LINES + GAP_LINES)) > 0.0,
        "the heading is off the top of the window"
    );
    assert!(
        here.baseline_of(-(HEADING_LINES + GAP_LINES)) < here.baseline_of(0.0),
        "the heading is not above the list"
    );
    assert!(
        here.baseline_of(rows as f32 + GAP_LINES) > here.baseline_of((rows - 1) as f32),
        "the notes are not below the last row"
    );
    assert!(
        here.baseline_of(rows as f32 + GAP_LINES) + here.row * FOOTER_LINES <= 900.0,
        "the notes run off the bottom"
    );
}

#[test]
fn the_chosen_row_is_carried_to_its_own_row_and_no_other() {
    // The whole selection rests on this. `overlay` puts the chosen row's text at the list's own
    // anchor with blank lines above it, so the engine lays it out with the same arithmetic it
    // laid the list out with. One newline too few or too many is a viewer that lights up one
    // program and launches another, which is what the first version of this did.
    for row in 0..CATALOGUE.len() {
        let marked = overlay(row, "> chosen");
        assert_eq!(
            marked.matches(NEWLINE).count(),
            row,
            "row {row} is carried down the wrong number of lines"
        );
        let lines: Vec<&str> = marked.split(NEWLINE).collect();
        assert_eq!(lines.len(), row + 1);
        assert_eq!(lines[row], "> chosen", "the text is not on row {row}");
        assert!(
            lines[..row].iter().all(|line| line.is_empty()),
            "something was drawn on a row above {row}"
        );
    }
}

#[test]
fn the_selection_stops_at_the_ends() {
    let rows = CATALOGUE.len();
    assert_eq!(step_selection(0, -1, rows), 0, "up from the top stays");
    assert_eq!(
        step_selection(rows - 1, 1, rows),
        rows - 1,
        "down from the bottom stays"
    );
    assert_eq!(step_selection(3, 1, rows), 4);
    assert_eq!(step_selection(3, -1, rows), 2);
    assert_eq!(
        step_selection(0, 0, 0),
        0,
        "an empty list has nowhere to go"
    );
}
