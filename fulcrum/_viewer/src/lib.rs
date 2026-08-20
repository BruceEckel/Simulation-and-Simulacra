//! The front door: what is in the set, whether it is here, and where the list sits on screen.
//!
//! No window and no drawing — that is `main.rs`. This is the part worth testing on its own: the
//! catalogue, finding the executables, and the arithmetic that turns a window and a row count
//! into somewhere to put them.

use std::path::{Path, PathBuf};

/// One simulation: what it is called, and the one line it describes itself with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Piece {
    /// Its name, which is its package, its directory and its executable.
    pub name: &'static str,
    /// What it says it is: the `description` from its own manifest.
    pub about: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/catalogue.rs"));

/// A simulation, and whether it is actually here to be run.
#[derive(Clone, Debug)]
pub struct Listing {
    /// Which piece this is.
    pub piece: Piece,
    /// Its executable, if one was found beside this one.
    pub exe: Option<PathBuf>,
}

impl Listing {
    /// Can it be started?
    pub fn runnable(&self) -> bool {
        self.exe.is_some()
    }
}

/// What a simulation's executable is called on this platform.
pub fn exe_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// The directory to look for the simulations in: the one this executable is in.
///
/// That is the right answer both ways round, which is why there is no second guess. A release
/// is unpacked into one directory and every executable sits in it side by side. A development
/// build puts them all in `target/debug` or `target/release`, side by side again. The viewer
/// looks beside itself and finds them in either case.
pub fn beside_executable() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.to_path_buf())
}

/// The whole catalogue, each entry paired with its executable if `directory` holds one.
pub fn survey(directory: Option<&Path>) -> Vec<Listing> {
    CATALOGUE
        .iter()
        .map(|piece| Listing {
            piece: *piece,
            exe: directory
                .map(|dir| dir.join(exe_name(piece.name)))
                .filter(|path| path.is_file()),
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// where the list goes
// ---------------------------------------------------------------------------------------

/// What the font actually does, per unit of text size.
///
/// Measured rather than assumed. The first version of this guessed, and both guesses were
/// wrong: the built-in font is *exactly* square, so a list laid out for the proportions of an
/// ordinary typeface ran off the right-hand edge, off the top, and — worst — put the highlight
/// on a different row from the one that would be launched.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Metrics {
    /// Distance from one line to the next.
    pub line: f32,
    /// Width of one character.
    pub advance: f32,
}

impl Default for Metrics {
    /// What the built-in font measures at — exactly square, one pixel of advance and one pixel
    /// of line per pixel of size. Used for the frame or two before the real measurement is
    /// available, and it is not a guess: it is what asking the font returns.
    fn default() -> Self {
        Self {
            line: 1.0,
            advance: 1.0,
        }
    }
}

/// Lines of heading above the list.
pub const HEADING_LINES: f32 = 2.0;
/// Blank lines between the heading and the list, and between the list and the notes.
pub const GAP_LINES: f32 = 1.0;
/// Lines of notes below it: what the chosen one is, where it is, what happened, and the keys.
pub const FOOTER_LINES: f32 = 4.0;
/// Margin around the whole thing, as a fraction of the window.
const MARGIN: f32 = 0.02;
/// Where in a line the baseline sits, as a fraction of the line height. The engine anchors a
/// block of text at the baseline of its first line, so this is what turns "the top of row `n`"
/// into somewhere to put it.
pub const BASELINE: f32 = 0.8;
/// Never smaller than this, however cramped the window.
const SIZE_MIN: f32 = 6.0;
/// And never larger, however much room there is.
const SIZE_MAX: f32 = 20.0;

/// Where the list sits in the window.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layout {
    /// Text height, in physical pixels.
    pub size: f32,
    /// Distance from one row to the next, in physical pixels.
    pub row: f32,
    /// Top of the first row, in physical pixels from the top of the window.
    pub top: f32,
    /// Left edge, in physical pixels from the left of the window.
    pub left: f32,
}

impl Layout {
    /// Where to anchor a block of text whose first line is row `row` of the list.
    ///
    /// In physical pixels from the top of the window, and it is the **baseline** of that first
    /// line, because that is what the engine puts at a text entity's translation.
    pub fn baseline_of(&self, row: f32) -> f32 {
        self.top + self.row * (row + BASELINE)
    }
}

/// The width the list aims to be, in characters.
///
/// An aim rather than a requirement. Sizing the text so that the single longest description fits
/// whole makes everything small for the sake of one line — the longest is half again the length
/// of the typical one — so the layout aims for this and [`clip`] shortens whatever still does not
/// fit. Nothing is lost by that: the notes under the list always show the chosen one in full.
pub const AIM_COLUMNS: usize = 112;

/// Fit a heading, `rows` rows of about `columns` characters, and the notes into a window.
///
/// The size is whichever of the two limits binds first — the height, so that everything is on
/// screen at once, or the width, so that a row of about `columns` characters fits across.
pub fn layout(window: (u32, u32), rows: usize, columns: usize, metrics: Metrics) -> Layout {
    let width = window.0.max(1) as f32;
    let height = window.1.max(1) as f32;
    let lines = HEADING_LINES + GAP_LINES + rows as f32 + GAP_LINES + FOOTER_LINES;
    let by_height = height * (1.0 - 2.0 * MARGIN) / (lines * metrics.line.max(0.1));
    let by_width =
        width * (1.0 - 2.0 * MARGIN) / (columns.max(1) as f32 * metrics.advance.max(0.1));
    let size = by_height.min(by_width).clamp(SIZE_MIN, SIZE_MAX);
    let row = size * metrics.line;
    Layout {
        size,
        row,
        top: height * MARGIN + row * (HEADING_LINES + GAP_LINES),
        left: width * MARGIN,
    }
}

/// How many characters actually fit across the window at this layout.
pub fn columns_that_fit(layout: &Layout, window: (u32, u32), metrics: Metrics) -> usize {
    let usable = window.0.max(1) as f32 - 2.0 * layout.left;
    (usable / (layout.size * metrics.advance).max(1.0))
        .floor()
        .max(1.0) as usize
}

/// `text`, shortened to `columns` characters if it is longer, and marked where it was cut.
///
/// Marked rather than silently cut: a description that stops mid-word without saying so reads
/// as a description that was written that way.
pub fn clip(text: &str, columns: usize) -> String {
    if text.chars().count() <= columns {
        return text.to_string();
    }
    const MARK: &str = "...";
    let keep = columns.saturating_sub(MARK.len());
    text.chars().take(keep).collect::<String>() + MARK
}

/// The chosen row's own text, carried down to its row by the blank lines above it.
///
/// This is how the selection is shown, and it is worth saying why it is a string rather than a
/// rectangle. A bar drawn behind the list has to be positioned by arithmetic that agrees with
/// the engine's text layout, and the first version of this did not: it put the bar one row below
/// the choice, so what was lit up was not what would be launched, and clicking `boids` ran
/// `avalanche`. Written as text at the list's own anchor, the engine lays it out with exactly
/// the arithmetic it laid the list out with, and the two cannot disagree.
pub fn overlay(row: usize, text: &str) -> String {
    const NEWLINE: &str = "\n";
    format!("{}{text}", NEWLINE.repeat(row))
}

/// Which row a pointer at `y` physical pixels from the top of the window is over, if any.
pub fn row_at(layout: &Layout, y: f32, rows: usize) -> Option<usize> {
    if y < layout.top || layout.row <= 0.0 {
        return None;
    }
    let row = ((y - layout.top) / layout.row) as usize;
    (row < rows).then_some(row)
}

/// Move a selection by `step` rows, stopping at the ends rather than wrapping.
///
/// Stopping rather than wrapping on purpose: a list you can see all of at once has a top and a
/// bottom, and holding a key down should come to rest against them rather than cycling.
pub fn step_selection(selected: usize, step: isize, rows: usize) -> usize {
    if rows == 0 {
        return 0;
    }
    let last = rows as isize - 1;
    (selected as isize + step).clamp(0, last) as usize
}
