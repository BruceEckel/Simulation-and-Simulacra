//! Giraffe, windowed: the board, lit. The board and its terms live in `game.rs` and stay
//! renderer-free; everything here is colour, light, and the keys that change nothing about
//! the game.
//!
//! `cargo run -p giraffe --release`
//!
//! - press to plant giraffes, right-press to plant jackals; hold for a bigger patch
//! - scroll turns the dial: how often needs truly conflict
//! - `up` / `down` pace, `left` / `right` temptation, `space` pause, `r` a new board
//! - `c` is the next palette, `h` puts the hint away, `F11` goes fullscreen and back
//!
//! It opens in an ordinary window. `F11` takes the whole display with no border, and `F11`
//! again gives the window back.

use fulcrum::prelude::*;
use fulcrum_render::WindowHandle;
use giraffe::game::{
    self, ACROSS, ARENA, CELL, DOWN, Field, GamePlugin, Pace, Strategy, Terms, centre_of,
};
use giraffe::look::{LOOKS, Palette, blend, colour, luminance, palette, tone};
use simulacra_assets::assets;
use std::f32::consts::TAU;

/// Text height in world units. The built-in font is sharpest at multiples of 8.
const HINT_SIZE: f32 = 8.0;
/// Gap between the lowest line of the hint and the bottom of the board.
const HINT_MARGIN: f32 = 20.0;
/// How far apart the four legend entries sit, in world units.
const LEGEND_PITCH: f32 = 300.0;
/// Seconds the hint stays up before it goes away and leaves you alone.
const HINT_LIFE: f32 = 18.0;
/// How bright the hint is while it is up.
const HINT_ALPHA: f32 = 0.6;
/// The least luminance a legend entry is drawn at: the darkest tones are lifted to this so
/// that their names can be read against the veil.
const LEGEND_FLOOR: f32 = 0.12;
/// Seconds a palette takes to cross into the next one.
const PALETTE_CROSSFADE: f32 = 3.0;

/// The gap between tiles, in world units. What is left of a cell is the tile.
const TILE_GAP: f32 = 1.0;
/// Seconds a tile takes to settle most of the way into a new colour. A change of strategy
/// blooms rather than flips.
const SETTLE: f32 = 0.22;
/// How much brighter a tile is the moment it changes strategy, fading over the generation.
const FRESH: f32 = 0.4;
/// How dim a tile is when it scored nothing, and how much brighter the best score
/// on the board makes it.
const DIM: f32 = 0.78;
const EARNED: f32 = 0.22;
/// How far the lamp reaches, in world units, and how much brighter it makes a tile under it:
/// a floor for every tile, and more for the ones that are scoring, so that under the lamp you
/// can see who is doing well.
const LAMP_REACH: f32 = 110.0;
const LAMP_LIFT: f32 = 0.18;
const LAMP_REVEAL: f32 = 0.5;
/// How wide the lamp is drawn, and how bright, and how long a breath is. Eleven seconds is
/// five and a half breaths a minute, which is the pace slow-breathing practice settles on.
const LAMP_SIZE: f32 = 320.0;
const LAMP_ALPHA: f32 = 0.16;
const LAMP_SWELL: f32 = 0.3;
const BREATH_PERIOD: f32 = 11.0;

/// What the hint says while it is up: the mouse, then the keys. The readout is the third line.
const HINT_MOUSE: &str = "press  plant giraffes      right press  plant jackals      hold  a bigger patch      scroll  how often needs truly conflict";
const HINT_KEYS: &str = "up / down  pace     left / right  temptation     space  pause     c  palette     r  new board     h  hint     F11  window";
/// Distance from one line of the hint to the next. The built-in font is square, so lines set
/// at their own height touch; half again gives them air.
const LINE: f32 = 12.0;
/// The band of night drawn behind the hint so it can be read over a busy board: how tall, in
/// world units, and how much of the board it lets through.
const VEIL_HEIGHT: f32 = 72.0;
const VEIL_ALPHA: f32 = 0.72;

/// The legend, in [`Strategy::index`] order.
const LEGEND: [&str; 4] = [
    "silent, shares",
    "silent, takes",
    "speaks, shares",
    "speaks, takes",
];
/// The order the legend is read in: the ones who speak first.
const LEGEND_ORDER: [usize; 4] = [2, 3, 0, 1];

/// Everything that is a matter of taste rather than of the board.
#[derive(Resource)]
struct Painter {
    /// Which of [`LOOKS`] is showing, and which it is coming from.
    palette: usize,
    previous: usize,
    /// How far across the crossfade is, `0..1`.
    blend: f32,
    /// Seconds since the start.
    elapsed: f32,
    /// Seconds the hint has been up.
    shown: f32,
    /// Whether the hint is wanted.
    hint: bool,
    /// Whether the pointer has been hidden yet.
    hidden: bool,
    /// The colour every tile is showing right now, in linear light, on its way to the colour
    /// it should be showing.
    glaze: Vec<[f32; 3]>,
}

impl Default for Painter {
    fn default() -> Self {
        Self {
            palette: 0,
            previous: 0,
            blend: 1.0,
            elapsed: 0.0,
            shown: 0.0,
            hint: true,
            hidden: false,
            glaze: vec![[0.0; 3]; (ACROSS * DOWN) as usize],
        }
    }
}

/// One cell's tile.
#[derive(Component)]
struct Tile(usize);

/// The ground behind the tiles.
#[derive(Component)]
struct Night;

/// The light that follows the pointer.
#[derive(Component)]
struct Lamp;

/// One line of the hint: the mouse, the keys, and the readout.
#[derive(Component)]
struct HintLine(usize);

/// The band of night behind the hint.
#[derive(Component)]
struct Veil;

/// One entry of the legend, by strategy index.
#[derive(Component)]
struct Legend(usize);

/// Aim the camera, lay the tiles, and put up the lamp, the hint and the legend.
fn setup(mut commands: Commands, mut assets: AssetLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: ARENA.x,
        height: ARENA.y,
    };
    let tile = assets.load("tile.png");
    let white = assets.load("white.png");
    let glow = assets.load("glow.png");

    commands.spawn((
        Night,
        Sprite::new(white).with_size(ARENA).with_z(-1.0),
        Transform2D::IDENTITY,
    ));
    for y in 0..DOWN {
        for x in 0..ACROSS {
            commands.spawn((
                Tile(Field::index(x as i32, y as i32)),
                Sprite::new(tile)
                    .with_size(Vec2::splat(CELL - TILE_GAP))
                    .with_z(0.0),
                Transform2D {
                    translation: centre_of(x, y),
                    rotation: 0.0,
                    scale: Vec2::ONE,
                },
            ));
        }
    }
    commands.spawn((
        Lamp,
        Sprite::new(glow)
            .with_size(Vec2::splat(LAMP_SIZE))
            .with_z(1.0),
        Transform2D::IDENTITY,
    ));
    commands.spawn((
        Veil,
        Sprite::new(white)
            .with_size(vec2(ARENA.x, VEIL_HEIGHT))
            .with_z(9.0),
        Transform2D::from_xy(0.0, -ARENA.y / 2.0 + VEIL_HEIGHT / 2.0),
    ));
    // The lines are set from the bottom up: the readout lowest, the keys above it, the mouse
    // above that, and the legend on top.
    for line in 0..3 {
        commands.spawn((
            HintLine(line),
            Text::new("")
                .with_size(HINT_SIZE)
                .with_align(HAlign::Center)
                .with_z(10.0),
            Transform2D::from_xy(0.0, -ARENA.y / 2.0 + HINT_MARGIN + LINE * (2 - line) as f32),
        ));
    }
    for (slot, &which) in LEGEND_ORDER.iter().enumerate() {
        commands.spawn((
            Legend(which),
            Text::new(LEGEND[which])
                .with_size(HINT_SIZE)
                .with_align(HAlign::Center)
                .with_z(10.0),
            Transform2D::from_xy(
                (slot as f32 - 1.5) * LEGEND_PITCH,
                -ARENA.y / 2.0 + HINT_MARGIN + LINE * 3.0 + 4.0,
            ),
        ));
    }
}

/// The keys that change nothing about the board: the palette, the hint, and whether this is
/// a window or the whole display. Debounced against the previous frame rather than using
/// `just_pressed`, since a frame system can see the same tick's edge twice.
fn look_controls(
    input: Res<Input>,
    time: Res<Time>,
    mut painter: ResMut<Painter>,
    window: Option<Res<WindowHandle>>,
    mut held: Local<[bool; 3]>,
) {
    let dt = time.frame_delta;
    painter.elapsed += dt;

    let down = [
        input.pressed(Key::C),
        input.pressed(Key::H),
        input.pressed(Key::F11),
    ];
    if down[0] && !held[0] {
        painter.previous = painter.palette;
        painter.palette = (painter.palette + 1) % LOOKS.len();
        painter.blend = 0.0;
    }
    if down[1] && !held[1] {
        painter.hint = !painter.hint;
        painter.shown = 0.0;
    }
    // Fullscreen is a window operation: the event loop keeps running, the fixed tick keeps
    // firing, and the board does not hear about it.
    if down[2]
        && !held[2]
        && let Some(window) = &window
    {
        let fullscreen = window.0.fullscreen().is_none();
        window
            .0
            .set_fullscreen(fullscreen.then_some(winit::window::Fullscreen::Borderless(None)));
    }
    *held = down;

    // The pointer is the lamp. An arrow over the board would be a second thing to look at.
    if !painter.hidden
        && let Some(window) = &window
    {
        window.0.set_cursor_visible(false);
        painter.hidden = true;
    }

    painter.blend = (painter.blend + dt / PALETTE_CROSSFADE).min(1.0);

    // Any key, a turn of the wheel or a press brings the hint back for a while.
    let touched = [
        Key::C,
        Key::H,
        Key::R,
        Key::Space,
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::F11,
    ]
    .iter()
    .any(|key| input.pressed(*key))
        || input.scroll_delta() != 0.0
        || input.mouse_pressed(MouseButton::Left)
        || input.mouse_pressed(MouseButton::Right);
    painter.shown = if touched && painter.hint {
        0.0
    } else {
        painter.shown + dt
    };
}

/// The palette as it is right now, part way through a crossfade or not.
fn current(painter: &Painter) -> Palette {
    let from = palette(&LOOKS[painter.previous % LOOKS.len()]);
    let to = palette(&LOOKS[painter.palette % LOOKS.len()]);
    blend(&from, &to, game::ease(painter.blend))
}

/// Colour every tile: its strategy's tone, brighter for what it scored, brighter again under
/// the lamp, and brightest the moment it changes. Each tile eases toward that rather than
/// jumping, so a generation blooms across the board instead of flipping.
fn paint_tiles(
    input: Res<Input>,
    time: Res<Time>,
    field: Res<Field>,
    terms: Res<Terms>,
    pace: Res<Pace>,
    mut painter: ResMut<Painter>,
    mut tiles: Query<(&Tile, &mut Sprite)>,
) {
    let palette = current(&painter);
    let pointer = input.mouse_world();
    let ceiling = (game::NEIGHBOURS.len() as f32 * terms.ceiling()).max(1e-6);
    let fresh = FRESH * (1.0 - pace.phase()).powi(2);
    let settle = 1.0 - (-time.frame_delta / SETTLE).exp();
    let reach = -0.5 / (LAMP_REACH * LAMP_REACH);
    for (tile, mut sprite) in &mut tiles {
        let i = tile.0;
        let (x, y) = Field::place(i);
        let strategy = field.cells[i];
        let earned = (field.score[i] / ceiling).clamp(0.0, 1.0);
        let lit = (centre_of(x, y).distance_squared(pointer) * reach).exp();
        let bright = DIM
            + EARNED * earned
            + lit * (LAMP_LIFT + LAMP_REVEAL * earned)
            + if field.age[i] == 0 { fresh } else { 0.0 };
        let want = tone(&palette, strategy).map(|channel| channel * bright);
        let glaze = &mut painter.glaze[i];
        for channel in 0..3 {
            glaze[channel] += (want[channel] - glaze[channel]) * settle;
        }
        sprite.color = colour(*glaze, 1.0, 1.0);
    }
}

/// The ground takes the palette's night, and the lamp goes where the pointer is and breathes.
fn dress(
    input: Res<Input>,
    painter: Res<Painter>,
    mut nights: Query<&mut Sprite, (With<Night>, Without<Lamp>)>,
    mut lamps: Query<(&mut Transform2D, &mut Sprite), With<Lamp>>,
) {
    let palette = current(&painter);
    for mut night in &mut nights {
        night.color = colour(palette.night, 1.0, 1.0);
    }
    let breath = 0.5 - 0.5 * (TAU * painter.elapsed / BREATH_PERIOD).cos();
    for (mut transform, mut sprite) in &mut lamps {
        transform.translation = input.mouse_world();
        sprite.color = colour(palette.lamp, 1.0, LAMP_ALPHA * (1.0 + LAMP_SWELL * breath));
    }
}

/// The hint and the legend go away after a while and come back whenever a key is pressed, so
/// the piece spends nearly all of its life with nothing on it but the board. The veil behind
/// them fades with them.
fn hint(
    painter: Res<Painter>,
    field: Res<Field>,
    terms: Res<Terms>,
    pace: Res<Pace>,
    mut lines: Query<(&HintLine, &mut Text), Without<Legend>>,
    mut legends: Query<(&Legend, &mut Text), Without<HintLine>>,
    mut veils: Query<&mut Sprite, With<Veil>>,
) {
    let fade = if painter.hint {
        ((HINT_LIFE - painter.shown) / 3.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let palette = current(&painter);
    let ink = tone(&palette, Strategy::GIRAFFE);
    let readout = format!(
        "{}   needs truly conflict {:.0}% of the time   temptation {:.2}   a generation every {:.1} s   speaking {:.0}%   sharing {:.0}%{}",
        LOOKS[painter.palette % LOOKS.len()].name,
        terms.conflict * 100.0,
        terms.temptation,
        pace.period,
        field.speaking() * 100.0,
        field.sharing() * 100.0,
        if pace.paused { "   paused" } else { "" },
    );
    for (line, mut text) in &mut lines {
        let value = match line.0 {
            0 => HINT_MOUSE,
            1 => HINT_KEYS,
            _ => readout.as_str(),
        };
        if text.value != value {
            text.value = value.to_string();
        }
        text.color = colour(ink, 0.9, HINT_ALPHA * fade);
    }
    for (legend, mut text) in &mut legends {
        let tone = palette.tones[legend.0];
        let lift = (LEGEND_FLOOR / luminance(tone).max(1e-4)).max(1.0);
        text.color = colour(tone, lift, HINT_ALPHA * fade);
    }
    for mut veil in &mut veils {
        veil.color = colour(palette.night, 1.0, VEIL_ALPHA * fade);
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Giraffe".into(),
        window_size: (ARENA.x as u32, ARENA.y as u32),
        clear_color: Color::rgb(0.0, 0.0, 0.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Painter::default())
    .add_startup(setup)
    .add_frame_system(look_controls)
    .add_frame_system(paint_tiles)
    .add_frame_system(dress)
    .add_frame_system(hint)
    .run();
}
