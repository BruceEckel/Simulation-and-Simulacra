//! The front door, windowed: the whole set on one screen, and a way into any of it.
//!
//! `cargo run -p _viewer --release`
//!
//! - `up` / `down` or the mouse chooses
//! - `Enter` or a click runs it
//! - `Esc` closes this
//!
//! It is called `_viewer` so that it sorts to the top of a directory listing, which is the only
//! reason: somebody who has unpacked a release into a folder of twenty-two executables should
//! find the one that explains the other twenty-one without having to look for it.
//!
//! It looks for the simulations **beside itself**, which is the right answer both ways round: a
//! release is one directory of executables side by side, and a development build puts them all
//! in `target/release` side by side again. Anything it cannot find is still listed, with what
//! it is and a note that it is not here — a catalogue that hid what you had not built yet would
//! be a worse catalogue.

use _viewer::{
    AIM_COLUMNS, CATALOGUE, GAP_LINES, HEADING_LINES, Layout, Listing, Metrics, beside_executable,
    clip, columns_that_fit, layout, overlay, row_at, step_selection, survey,
};
use fulcrum::prelude::*;
use fulcrum_render::{DefaultFont, GlyphCache};
use simulacra_assets::assets;

/// How wide the name column is. The longest name in the set is `thunderhead`, at eleven.
const NAME_WIDTH: usize = 13;
/// What the chosen row is marked with, in the column before the name.
const MARKER: &str = "> ";

/// Window size at startup.
///
/// Wide rather than tall on purpose: the list is twenty-one short lines, so what decides how big
/// the text can be is how many characters fit across, not how many lines fit down.
const DEFAULT_WINDOW: (u32, u32) = (1800, 740);

/// The list, which is most of the screen and should not shout.
const LISTED: Color = Color::rgb(0.62, 0.66, 0.72);
/// The chosen one, drawn over the top of its own row in the list.
const CHOSEN: Color = Color::rgb(1.0, 0.86, 0.42);
/// The heading and the notes underneath.
const QUIET: Color = Color::rgb(0.45, 0.50, 0.58);

/// The size the font is measured at. Large, so that rounding to whole pixels in the rasteriser
/// is a small fraction of the answer.
const PROBE: f32 = 64.0;

/// What is here, and what is only listed.
#[derive(Resource)]
struct Shelf(Vec<Listing>);

/// What is chosen, and what happened last time something was started.
#[derive(Resource, Default)]
struct Choice {
    /// Which row.
    row: usize,
    /// What to say under the list: the result of the last attempt to start something.
    said: Option<String>,
}

/// What the font actually does, once there has been a font to ask.
#[derive(Resource, Default)]
struct Measured(Option<Metrics>);

/// Which of the four blocks of text an entity is.
///
/// One component rather than four marker types, so that laying them out is one query over four
/// things instead of four queries that have to be told they cannot overlap.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Block {
    /// The title, above the list.
    Heading,
    /// The list itself.
    Rows,
    /// The chosen row, drawn again in its own colour exactly over the top of itself.
    Chosen,
    /// What the chosen one is, where it is, and the keys.
    Detail,
}

/// Put the four blocks on screen, and find out what is actually here.
fn setup(mut commands: Commands) {
    let here = beside_executable();
    commands.insert_resource(Shelf(survey(here.as_deref())));
    for block in [Block::Heading, Block::Rows, Block::Chosen, Block::Detail] {
        let colour = match block {
            Block::Heading | Block::Detail => QUIET,
            Block::Rows => LISTED,
            Block::Chosen => CHOSEN,
        };
        let z = if block == Block::Chosen { 2.0 } else { 1.0 };
        commands.spawn((
            block,
            Text::new("").with_color(colour).with_z(z),
            Transform2D::default(),
        ));
    }
}

/// Ask the font what it actually does, once, on the first frame there is one.
///
/// Everything is laid out against this rather than against an assumption. The assumption was
/// wrong in both axes at once — the built-in font turns out to be *exactly* square, one pixel of
/// advance and one of line per pixel of size, where an ordinary typeface is nothing like — and
/// being wrong about it silently ran the wrong program.
fn measure(
    fonts: Res<Assets<Font>>,
    default_font: Option<Res<DefaultFont>>,
    mut measured: ResMut<Measured>,
) {
    if measured.0.is_some() {
        return;
    }
    let Some(font) = default_font.and_then(|handle| fonts.get(handle.0)) else {
        return;
    };
    // Two lines of one character: the height gives the line advance, the width gives the
    // character advance.
    let two = GlyphCache::measure(font, "M\nM", PROBE);
    if two.y <= 0.0 || two.x <= 0.0 {
        return;
    }
    measured.0 = Some(Metrics {
        line: two.y / 2.0 / PROBE,
        advance: two.x / PROBE,
    });
}

/// A point given in physical pixels from the top-left of the window, in world units.
///
/// World units are physical pixels in this engine and the camera sits on the middle of the
/// window, so this is the whole of the conversion.
fn at(window: Vec2, x: f32, y: f32) -> Vec2 {
    vec2(x - window.x / 2.0, window.y / 2.0 - y)
}

/// How wide the layout aims to be: the longest row, but not past what is worth shrinking
/// everything else for. Whatever still does not fit is clipped, and the notes under the list
/// always show the chosen one in full.
fn columns() -> usize {
    CATALOGUE
        .iter()
        .map(|piece| MARKER.len() + NAME_WIDTH + piece.about.chars().count())
        .max()
        .unwrap_or(80)
        .min(AIM_COLUMNS)
}

/// Where everything is this frame.
fn plan(window: &WindowInfo, rows: usize, measured: &Measured) -> Layout {
    layout(
        (window.width, window.height),
        rows,
        columns(),
        measured.0.unwrap_or_default(),
    )
}

/// One row of the list, marked or not, clipped to what fits across the window.
fn row_text(listing: &Listing, chosen: bool, fits: usize) -> String {
    clip(
        &format!(
            "{}{:<NAME_WIDTH$}{}",
            if chosen { MARKER } else { "  " },
            listing.piece.name,
            listing.piece.about
        ),
        fits,
    )
}

/// The keys and the mouse: choose something, or start it.
///
/// In `FixedUpdate` rather than with the drawing, so that `just_pressed` means what it says —
/// a frame system can see the same tick's edge twice.
#[expect(
    clippy::too_many_arguments,
    reason = "the keys, the mouse, and where the rows are"
)]
fn steer(
    input: Res<Input>,
    window: Res<WindowInfo>,
    shelf: Res<Shelf>,
    measured: Res<Measured>,
    mut choice: ResMut<Choice>,
    mut held: Local<f32>,
    mut was: Local<Option<Vec2>>,
    time: Res<Time>,
) {
    let rows = shelf.0.len();
    if rows == 0 {
        return;
    }
    let here = plan(&window, rows, &measured);

    // The pointer chooses whatever it is over, so the thing under your hand is the thing that
    // will run when you click it — but only while it is actually moving. A still pointer left
    // lying over the list must not drag the choice back every tick and make the arrow keys
    // useless.
    let pointer = input.mouse_screen();
    let over = row_at(&here, pointer.y, rows);
    let moved = was.is_none_or(|last| last != pointer);
    *was = Some(pointer);
    if moved && let Some(row) = over {
        choice.row = row;
    }

    // Up and down, repeating after a moment the way a held key should.
    let mut step = 0isize;
    let down = input.pressed(Key::Down) as i32 - input.pressed(Key::Up) as i32;
    if down == 0 {
        *held = 0.0;
    } else {
        let was_held = *held;
        *held += time.fixed_delta;
        if was_held == 0.0
            || (was_held < 0.35 && *held >= 0.35)
            || (was_held >= 0.35 && (*held * 14.0).floor() > (was_held * 14.0).floor())
        {
            step = down as isize;
        }
    }
    if input.just_pressed(Key::Down) {
        step = 1;
    }
    if input.just_pressed(Key::Up) {
        step = -1;
    }
    if step != 0 {
        choice.row = step_selection(choice.row, step, rows);
    }

    // A click only counts over the list itself: the heading and the notes underneath are not
    // buttons, and clicking one of them should not start whatever happened to be chosen.
    let clicked = input.mouse_just_pressed(MouseButton::Left) && over.is_some();
    if input.just_pressed(Key::Enter) || clicked {
        choice.said = Some(run(&shelf.0[choice.row]));
    }

    // Nothing here is worth saving and there is no simulation to wind down, so closing is
    // closing. The engine has no way for a game to ask for it, so this is it.
    if input.just_pressed(Key::Escape) {
        std::process::exit(0);
    }
}

/// Start one, and say what happened.
fn run(listing: &Listing) -> String {
    let Some(exe) = &listing.exe else {
        return format!(
            "{} is not in this directory: build the set with `cargo build --workspace --release`",
            listing.piece.name
        );
    };
    match std::process::Command::new(exe).spawn() {
        Ok(_) => format!("started {}", listing.piece.name),
        Err(error) => format!("could not start {}: {error}", listing.piece.name),
    }
}

/// Lay the four blocks out for whatever shape the window has ended up.
///
/// The chosen row is not a bar drawn behind the list. It is the row's own text, written again in
/// its own colour at the same anchor the list has, with the blank lines above it spelled out.
/// The engine then lays it out with exactly the same arithmetic it laid the list out with, so
/// what is lit up is always what will run. Nothing here has to know where in a line the baseline
/// sits, which is precisely what the first version of this got wrong: it drew a bar one row
/// below the choice, so clicking `boids` ran `avalanche`.
fn arrange(
    window: Res<WindowInfo>,
    shelf: Res<Shelf>,
    choice: Res<Choice>,
    measured: Res<Measured>,
    mut blocks: Query<(&Block, &mut Text, &mut Transform2D)>,
) {
    if window.width == 0 || window.height == 0 {
        return; // minimized
    }
    let extent = vec2(window.width as f32, window.height as f32);
    let count = shelf.0.len();
    if count == 0 {
        return;
    }
    let here = plan(&window, count, &measured);
    let row = choice.row.min(count - 1);
    let chosen = &shelf.0[row];
    let fits = columns_that_fit(
        &here,
        (window.width, window.height),
        measured.0.unwrap_or_default(),
    );

    for (block, mut text, mut transform) in &mut blocks {
        text.size = here.size;
        let (value, baseline) = match block {
            Block::Heading => (
                format!(
                    "SIMULATION AND SIMULACRA\n{count} programs, each one a rule and what the rule does."
                ),
                here.baseline_of(-(HEADING_LINES + GAP_LINES)),
            ),
            Block::Rows => (
                shelf
                    .0
                    .iter()
                    .map(|listing| row_text(listing, false, fits))
                    .collect::<Vec<_>>()
                    .join("\n"),
                here.baseline_of(0.0),
            ),
            // The blank lines carry it down to its own row, so it cannot land anywhere else.
            Block::Chosen => (
                overlay(row, &row_text(chosen, true, fits)),
                here.baseline_of(0.0),
            ),
            Block::Detail => {
                let where_it_is = match &chosen.exe {
                    Some(path) => path.display().to_string(),
                    None => "not in this directory: build the set with `cargo build --workspace --release`"
                        .to_string(),
                };
                (
                    format!(
                        "{}   {}\n{}\n{}\nup/down or the mouse to choose    Enter or click to run it    Esc to close",
                        chosen.piece.name,
                        chosen.piece.about,
                        where_it_is,
                        choice.said.as_deref().unwrap_or(""),
                    ),
                    here.baseline_of(count as f32 + GAP_LINES),
                )
            }
        };
        text.value = value;
        transform.translation = at(extent, here.left, baseline);
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Simulation and Simulacra".into(),
        window_size: DEFAULT_WINDOW,
        clear_color: Color::rgb(0.05, 0.06, 0.09),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .insert_resource(Choice::default())
    .insert_resource(Measured::default())
    .add_startup(setup)
    .add_system(steer)
    // Chained: the font is measured before anything is laid out against it.
    .add_frame_system((measure, arrange).chain())
    .run();
}
