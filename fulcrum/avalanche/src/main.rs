//! Avalanche, windowed: the table, the readouts, and the histogram that is the whole point. The
//! rule itself lives in `game.rs` and does not know what any of this looks like.
//!
//! `cargo run -p avalanche --release`
//!
//! - hold the pointer to pour, right-click for a handful
//! - `f` loads the table again, `r` sweeps it clean, `x` forgets the measurements
//! - `t` stops the table feeding itself, `c` changes the colours, `h` hides the histogram
//! - `space` stills it, `up`/`down` change the pace, `0` restores it
//!
//! The histogram in the corner is the only thing here that is an argument rather than a picture.
//! Both axes are logarithmic, so a straight line means the avalanche sizes follow a power law:
//! there is no typical avalanche, and the pile has arranged that for itself out of a rule that
//! says nothing about it.

use avalanche::game::{
    self, ARENA, BINS, CELL, CELLS, GamePlugin, Ledger, Paused, Rain, Sizes, Slide, Speed, Table,
    WIDE,
};
use fulcrum::prelude::*;
use simulacra_assets::assets;

/// How many colours a cell can be: nothing, one, two, three, and about to go.
const LEVELS: usize = 5;
/// One palette: the five cell colours, the colour a toppling cell flares, and the surround.
type Look = ([[f32; 3]; LEVELS], [f32; 3], [f32; 3]);

/// How much of the flare a freshly toppled cell shows.
const GLOW_STRENGTH: f32 = 0.72;
/// Seconds a palette takes to cross into the next one.
const PALETTE_CROSSFADE: f32 = 0.8;
/// Seconds the hint stays up before it fades.
const HINT_LIFE: f32 = 22.0;

/// How tall the histogram panel is, in world units, and how wide.
const PANEL: Vec2 = Vec2::new(470.0, 96.0);
/// Where its bottom left corner sits.
const PANEL_AT: Vec2 = Vec2::new(70.0, 366.0);
/// How many octaves of height the histogram shows before it clips.
const PANEL_RANGE: f32 = 15.0;
/// A bin needs this many avalanches in it before it is allowed to affect the fitted line.
const FIT_FLOOR: u32 = 4;

/// Four palettes, in sRGB, converted to linear light once at startup.
const PALETTES: [(&str, Look); 4] = [
    (
        "sand",
        (
            [
                [0.121, 0.098, 0.078],
                [0.420, 0.243, 0.118],
                [0.722, 0.471, 0.180],
                [0.949, 0.776, 0.361],
                [1.000, 0.973, 0.851],
            ],
            [1.000, 0.949, 0.749],
            [0.067, 0.059, 0.055],
        ),
    ),
    (
        "ember",
        (
            [
                [0.078, 0.051, 0.086],
                [0.349, 0.059, 0.180],
                [0.749, 0.180, 0.149],
                [0.980, 0.549, 0.121],
                [1.000, 0.929, 0.600],
            ],
            [1.000, 0.898, 0.600],
            [0.047, 0.031, 0.051],
        ),
    ),
    (
        "ice",
        (
            [
                [0.047, 0.071, 0.118],
                [0.098, 0.251, 0.451],
                [0.200, 0.522, 0.749],
                [0.549, 0.851, 0.949],
                [0.949, 1.000, 1.000],
            ],
            [0.851, 0.980, 1.000],
            [0.031, 0.047, 0.078],
        ),
    ),
    (
        "ink",
        (
            [
                [0.078, 0.078, 0.086],
                [0.278, 0.278, 0.298],
                [0.522, 0.522, 0.549],
                [0.780, 0.780, 0.800],
                [1.000, 1.000, 1.000],
            ],
            [1.000, 1.000, 1.000],
            [0.051, 0.051, 0.055],
        ),
    ),
];

/// Texture handles, loaded once.
#[derive(Resource, Clone)]
struct Art {
    cell: Handle<Texture>,
    white: Handle<Texture>,
}

/// Sound handles, loaded once.
#[derive(Resource)]
struct Sounds {
    hiss: Handle<Sound>,
    rumble: Handle<Sound>,
    tick: Handle<Sound>,
}

/// Every palette, in linear light.
#[derive(Resource)]
struct Looks([Look; PALETTES.len()]);

/// Which palette is showing, and how far into the crossfade from the last one.
#[derive(Resource)]
struct Palette {
    current: usize,
    previous: usize,
    blend: f32,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            current: 0,
            previous: 0,
            blend: 1.0,
        }
    }
}

/// Whether the noise is turned off.
#[derive(Resource, Default)]
struct Muted(bool);

/// Whether the histogram is showing.
#[derive(Resource)]
struct Showing(bool);

impl Default for Showing {
    fn default() -> Self {
        Self(true)
    }
}

/// One cell of the table.
#[derive(Component)]
struct Cell(usize);

/// One bar of the histogram.
#[derive(Component)]
struct Bar(usize);

/// The straight line fitted through the bars.
#[derive(Component)]
struct Fit;

/// The surround: everything outside the table.
#[derive(Component)]
struct Ground;

/// The panel the histogram sits on.
#[derive(Component)]
struct Backing;

/// Marks the one number worth watching.
#[derive(Component)]
struct Headline;

/// Marks the rest of the readouts.
#[derive(Component)]
struct Readout;

/// Marks the histogram's caption.
#[derive(Component)]
struct Caption;

/// Marks the hint line.
#[derive(Component)]
struct Hint;

/// One sRGB channel in linear light.
fn linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// A colour written in sRGB, ready for the renderer.
fn paint(srgb: [f32; 3], alpha: f32) -> Color {
    Color::rgba(linear(srgb[0]), linear(srgb[1]), linear(srgb[2]), alpha)
}

/// Mix two colours.
fn mix(from: [f32; 3], to: [f32; 3], amount: f32) -> [f32; 3] {
    [
        from[0] + (to[0] - from[0]) * amount,
        from[1] + (to[1] - from[1]) * amount,
        from[2] + (to[2] - from[2]) * amount,
    ]
}

/// The palette as it currently stands, mid-crossfade or not.
fn showing(looks: &Looks, palette: &Palette) -> Look {
    let to = looks.0[palette.current];
    if palette.blend >= 1.0 {
        return to;
    }
    let from = looks.0[palette.previous];
    let ease = palette.blend * palette.blend * (3.0 - 2.0 * palette.blend);
    let mut blended = to;
    for level in 0..LEVELS {
        blended.0[level] = mix(from.0[level], to.0[level], ease);
    }
    blended.1 = mix(from.1, to.1, ease);
    blended.2 = mix(from.2, to.2, ease);
    blended
}

/// Load everything and lay the table out, one sprite a cell.
fn setup(
    mut commands: Commands,
    mut assets: AssetLoader,
    mut sounds: SoundLoader,
    mut audio: ResMut<Audio>,
    mut camera: ResMut<Camera2D>,
) {
    // The table is a simulation constant, so the window is only ever a view of it.
    camera.scaling = ScalingMode::Letterbox {
        width: ARENA.x,
        height: ARENA.y,
    };

    let art = Art {
        cell: assets.load("cell.png"),
        white: assets.load("white.png"),
    };
    commands.insert_resource(Sounds {
        hiss: sounds.load("hiss.wav"),
        rumble: sounds.load("rumble.wav"),
        tick: sounds.load("tick.wav"),
    });
    audio.set_master_volume(0.55);
    commands.insert_resource(Looks(PALETTES.map(|(_, look)| {
        let mut linear_look = look;
        for level in 0..LEVELS {
            linear_look.0[level] = look.0[level].map(linear);
        }
        linear_look.1 = look.1.map(linear);
        linear_look.2 = look.2.map(linear);
        linear_look
    })));

    // The surround, behind everything, so the table has an edge you can see.
    let mut ground = Sprite::new(art.white).with_z(-10.0);
    ground.custom_size = Some(ARENA + Vec2::splat(8.0));
    commands.spawn((Ground, ground, Transform2D::default()));

    for index in 0..CELLS {
        let column = (index % WIDE) as i32;
        let row = (index / WIDE) as i32;
        let mut sprite = Sprite::new(art.cell).with_z(0.0);
        sprite.custom_size = Some(Vec2::splat(CELL));
        commands.spawn((
            Cell(index),
            sprite,
            Transform2D::from_translation(game::cell_at(column, row)),
        ));
    }

    // The histogram: a panel, a bar per bin, and the line fitted through them.
    let mut backing = Sprite::new(art.white).with_z(20.0);
    backing.custom_size = Some(PANEL + vec2(16.0, 16.0));
    backing.anchor = Vec2::ZERO;
    commands.spawn((
        Backing,
        backing,
        Transform2D::from_translation(PANEL_AT - vec2(8.0, 8.0)),
    ));
    for bin in 0..BINS {
        let mut bar = Sprite::new(art.white).with_z(21.0);
        // Bars grow up from their bottom edge.
        bar.anchor = vec2(0.5, 0.0);
        commands.spawn((Bar(bin), bar, Transform2D::default()));
    }
    let mut line = Sprite::new(art.white).with_z(22.0);
    line.custom_size = Some(vec2(0.0, 2.0));
    commands.spawn((Fit, line, Transform2D::default()));

    commands.spawn((
        Headline,
        Text::new("")
            .with_size(14.0)
            .with_align(HAlign::Left)
            .with_z(30.0),
        Transform2D::from_translation(vec2(-ARENA.x * 0.5 + 26.0, ARENA.y * 0.5 - 24.0)),
    ));
    commands.spawn((
        Readout,
        Text::new("")
            .with_size(9.0)
            .with_align(HAlign::Left)
            .with_z(30.0),
        Transform2D::from_translation(vec2(-ARENA.x * 0.5 + 26.0, ARENA.y * 0.5 - 58.0)),
    ));
    commands.spawn((
        Caption,
        Text::new("")
            .with_size(8.0)
            .with_align(HAlign::Left)
            .with_z(30.0),
        Transform2D::from_translation(PANEL_AT + vec2(0.0, PANEL.y + 14.0)),
    ));
    commands.spawn((
        Hint,
        Text::new(HINT_LINES)
            .with_size(8.0)
            .with_align(HAlign::Center)
            .with_z(30.0),
        Transform2D::from_translation(vec2(0.0, -ARENA.y * 0.5 + 14.0)),
    ));
    commands.insert_resource(art);
}

/// C crosses to the next palette, H hides the histogram, M turns the noise off. Debounced against
/// the previous frame, since a frame system can see one tick's edge twice.
fn look_controls(
    input: Res<Input>,
    mut palette: ResMut<Palette>,
    mut showing: ResMut<Showing>,
    mut muted: ResMut<Muted>,
    mut audio: ResMut<Audio>,
    mut held: Local<(bool, bool, bool)>,
) {
    let now = (
        input.pressed(Key::C),
        input.pressed(Key::H),
        input.pressed(Key::M),
    );
    if now.0 && !held.0 {
        palette.previous = palette.current;
        palette.current = (palette.current + 1) % PALETTES.len();
        palette.blend = 0.0;
    }
    if now.1 && !held.1 {
        showing.0 = !showing.0;
    }
    if now.2 && !held.2 {
        muted.0 = !muted.0;
        audio.set_master_volume(if muted.0 { 0.0 } else { 0.55 });
    }
    *held = now;
}

/// Cross the palette over, once a change has been asked for.
fn advance_palette(mut palette: ResMut<Palette>, time: Res<Time>) {
    if palette.blend < 1.0 {
        palette.blend = (palette.blend + time.frame_delta / PALETTE_CROSSFADE).min(1.0);
    }
}

/// Colour every cell.
///
/// Two things decide a cell's colour: how many grains are in it, and how recently it toppled. The
/// second is the one that matters to watch, because it is what draws the shape of the avalanche
/// after the grains have already moved on.
fn paint_table(
    mut cells: Query<(&Cell, &mut Sprite), Without<Ground>>,
    mut ground: Query<&mut Sprite, With<Ground>>,
    table: Res<Table>,
    looks: Option<Res<Looks>>,
    palette: Res<Palette>,
) {
    let Some(looks) = looks else { return };
    let (levels, flare, surround) = showing(&looks, &palette);
    for mut sprite in &mut ground {
        sprite.color = Color::rgba(surround[0], surround[1], surround[2], 1.0);
    }
    for (cell, mut sprite) in &mut cells {
        let grains = table.grains[cell.0].min(LEVELS as u16 - 1) as usize;
        let heat = table.glow[cell.0] as f32 / 255.0;
        let colour = mix(levels[grains], flare, heat * GLOW_STRENGTH);
        sprite.color = Color::rgba(colour[0], colour[1], colour[2], 1.0);
    }
}

/// The straight line through the bars: slope, intercept, and how many bins it was fitted to.
fn fit_line(sizes: &Sizes) -> Option<(f32, f32, usize)> {
    let mut points = Vec::new();
    for (bin, &count) in sizes.bins.iter().enumerate() {
        if count < FIT_FLOOR {
            continue;
        }
        let (low, high) = game::bin_span(bin);
        // Density, not count: bins get wider as they go, and dividing by the width is what turns
        // the histogram into the distribution the exponent is about.
        let middle = ((low as f32) * (high as f32)).sqrt();
        let density = count as f32 / (high - low) as f32;
        points.push((middle.log2(), density.log2()));
    }
    if points.len() < 3 {
        return None;
    }
    let n = points.len() as f32;
    let mean_x = points.iter().map(|(x, _)| x).sum::<f32>() / n;
    let mean_y = points.iter().map(|(_, y)| y).sum::<f32>() / n;
    let mut top = 0.0;
    let mut bottom = 0.0;
    for (x, y) in &points {
        top += (x - mean_x) * (y - mean_y);
        bottom += (x - mean_x) * (x - mean_x);
    }
    if bottom.abs() < 1e-6 {
        return None;
    }
    let slope = top / bottom;
    Some((slope, mean_y - slope * mean_x, points.len()))
}

/// Where a bin's density sits on the panel, `0` at the bottom and `1` at the top.
fn panel_height(density_log: f32, top: f32) -> f32 {
    ((density_log - (top - PANEL_RANGE)) / PANEL_RANGE).clamp(0.0, 1.0)
}

/// Draw the histogram: a bar for each bin, and the line fitted through them.
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // standard ECS system shape
fn draw_histogram(
    mut bars: Query<(&Bar, &mut Sprite, &mut Transform2D), (Without<Fit>, Without<Backing>)>,
    mut fits: Query<(&mut Sprite, &mut Transform2D), (With<Fit>, Without<Backing>)>,
    mut backings: Query<&mut Sprite, With<Backing>>,
    mut captions: Query<&mut Text, With<Caption>>,
    sizes: Res<Sizes>,
    looks: Option<Res<Looks>>,
    palette: Res<Palette>,
    showing_it: Res<Showing>,
) {
    let Some(looks) = looks else { return };
    let (levels, flare, surround) = showing(&looks, &palette);
    for mut backing in &mut backings {
        // A shade darker than the surround, so the panel is a window rather than a card.
        backing.color = Color::rgba(
            surround[0] * 0.5,
            surround[1] * 0.5,
            surround[2] * 0.5,
            if showing_it.0 { 0.72 } else { 0.0 },
        );
    }
    let shown = sizes.widest().max(1) + 1;
    let step = PANEL.x / shown as f32;

    // The tallest bar sets the scale, and the scale is in octaves so it hardly ever moves.
    let mut top = f32::MIN;
    for (bin, &count) in sizes.bins.iter().enumerate().take(shown) {
        if count == 0 {
            continue;
        }
        let (low, high) = game::bin_span(bin);
        top = top.max((count as f32 / (high - low) as f32).log2());
    }
    if top == f32::MIN {
        top = 1.0;
    }

    for (bar, mut sprite, mut transform) in &mut bars {
        let count = sizes.bins[bar.0];
        if !showing_it.0 || bar.0 >= shown || count == 0 {
            sprite.color = Color::rgba(0.0, 0.0, 0.0, 0.0);
            continue;
        }
        let (low, high) = game::bin_span(bar.0);
        let density = (count as f32 / (high - low) as f32).log2();
        let height = (panel_height(density, top) * PANEL.y).max(1.5);
        sprite.custom_size = Some(vec2(step * 0.78, height));
        sprite.color = Color::rgba(levels[3][0], levels[3][1], levels[3][2], 0.9);
        transform.translation = PANEL_AT + vec2((bar.0 as f32 + 0.5) * step, 0.0);
    }

    let fitted = fit_line(&sizes);
    for (mut sprite, mut transform) in &mut fits {
        let Some((slope, intercept, _)) = fitted else {
            sprite.color = Color::rgba(0.0, 0.0, 0.0, 0.0);
            continue;
        };
        if !showing_it.0 {
            sprite.color = Color::rgba(0.0, 0.0, 0.0, 0.0);
            continue;
        }
        // The fit is in log2(size); a bin is half an octave, so bin/2 is the x it wants.
        let ends = [(0usize, 0.0), (shown - 1, 0.0)].map(|(bin, _)| {
            let x = PANEL_AT.x + (bin as f32 + 0.5) * step;
            let y =
                PANEL_AT.y + panel_height(slope * (bin as f32 / 2.0) + intercept, top) * PANEL.y;
            vec2(x, y)
        });
        let span = ends[1] - ends[0];
        sprite.custom_size = Some(vec2(span.length(), 1.6));
        sprite.color = Color::rgba(flare[0], flare[1], flare[2], 0.75);
        transform.translation = ends[0] + span * 0.5;
        transform.rotation = span.to_angle();
    }

    for mut caption in &mut captions {
        caption.value = match fitted {
            Some((slope, _, bins)) if showing_it.0 => format!(
                "how many avalanches of each size    slope {slope:.2} over {bins} bins    both axes logarithmic",
            ),
            Some(_) => String::new(),
            None if showing_it.0 => "how many avalanches of each size    measuring".to_string(),
            None => String::new(),
        };
        caption.color = Color::rgba(flare[0], flare[1], flare[2], 0.55);
    }
}

/// The readouts.
///
/// The first line is the one to watch. Whatever the table starts at, that number walks to about
/// 2.1 and stays there, and nothing in the rule mentions 2.1.
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // standard ECS system shape
fn readouts(
    mut headlines: Query<&mut Text, (With<Headline>, Without<Readout>)>,
    mut readouts: Query<&mut Text, (With<Readout>, Without<Headline>)>,
    table: Res<Table>,
    ledger: Res<Ledger>,
    slide: Res<Slide>,
    rain: Res<Rain>,
    speed: Res<Speed>,
    paused: Res<Paused>,
    looks: Option<Res<Looks>>,
    palette: Res<Palette>,
) {
    let Some(looks) = looks else { return };
    let (levels, _, _) = showing(&looks, &palette);
    for mut headline in &mut headlines {
        headline.value = format!("{:.3} grains a cell", table.mean());
        headline.color = Color::rgba(levels[4][0], levels[4][1], levels[4][2], 0.95);
    }
    for mut readout in &mut readouts {
        readout.value = format!(
            "it settles near 2.12 whether it starts full or empty\n\
             {} avalanches    biggest {} topples, {} waves\n\
             {} dropped    {} off the edge    {} moving{}{}{}",
            ledger.measured,
            ledger.biggest,
            ledger.longest,
            ledger.poured,
            ledger.lost,
            table.unstable() + usize::from(slide.running),
            if rain.0 { "" } else { "    feed off" },
            if speed.0 == 1.0 {
                String::new()
            } else {
                format!("    {:.2}x", speed.0)
            },
            if paused.0 { "    still" } else { "" },
        );
        readout.color = Color::rgba(levels[3][0], levels[3][1], levels[3][2], 0.85);
    }
}

/// Sand moving, and the sound of a big one.
#[allow(clippy::too_many_arguments)] // standard ECS system shape
fn play_sounds(
    mut audio: ResMut<Audio>,
    sounds: Option<Res<Sounds>>,
    assets: Res<Assets<Sound>>,
    table: Res<Table>,
    ledger: Res<Ledger>,
    muted: Res<Muted>,
    paused: Res<Paused>,
    time: Res<Time>,
    mut since: Local<f32>,
    mut last_biggest: Local<u32>,
) {
    let Some(sounds) = sounds else { return };
    if muted.0 || paused.0 {
        return;
    }
    *since += time.frame_delta;
    let moving = table.unstable();
    if moving > 0 && *since > 0.11 {
        *since = 0.0;
        // How loud depends on how much of the table is in motion, so a small slide whispers and
        // a big one roars without either of them being a separate sound.
        let weight = (moving as f32 / 900.0).clamp(0.06, 1.0);
        audio.play_with(
            &assets,
            sounds.hiss,
            PlayParams {
                volume: 0.18 + 0.55 * weight,
                pitch: 0.85 + 0.4 * weight,
                pan: 0.0,
            },
        );
    }
    if ledger.biggest > *last_biggest {
        // A new record only, so the rumble stays rare enough to mean something.
        if *last_biggest > 0 && ledger.biggest > 2_000 {
            audio.play_with(
                &assets,
                sounds.rumble,
                PlayParams {
                    volume: 0.6,
                    pitch: 0.9,
                    pan: 0.0,
                },
            );
        }
        *last_biggest = ledger.biggest;
    }
}

/// A handful landing.
fn play_handful(
    mut audio: ResMut<Audio>,
    sounds: Option<Res<Sounds>>,
    assets: Res<Assets<Sound>>,
    input: Res<Input>,
    muted: Res<Muted>,
) {
    let Some(sounds) = sounds else { return };
    if muted.0 {
        return;
    }
    if input.mouse_just_pressed(MouseButton::Right) || input.just_pressed(Key::B) {
        audio.play_with(
            &assets,
            sounds.tick,
            PlayParams {
                volume: 0.5,
                pitch: 1.0,
                pan: 0.0,
            },
        );
    }
}

/// What the hint says.
const HINT_LINES: &str = "pour with the pointer    right-click a handful    f load    r sweep    x forget    t feed    c colours    h histogram";

/// The hint fades away and comes back whenever a key is pressed.
fn hint(
    mut hints: Query<&mut Text, With<Hint>>,
    input: Res<Input>,
    time: Res<Time>,
    looks: Option<Res<Looks>>,
    palette: Res<Palette>,
    mut shown: Local<f32>,
) {
    let touched = [
        Key::C,
        Key::H,
        Key::M,
        Key::F,
        Key::R,
        Key::T,
        Key::X,
        Key::B,
        Key::Space,
        Key::Up,
        Key::Down,
        Key::Digit0,
    ]
    .iter()
    .any(|key| input.pressed(*key))
        || input.mouse_pressed(MouseButton::Left);
    *shown = if touched {
        0.0
    } else {
        *shown + time.frame_delta
    };
    let fade = ((HINT_LIFE - *shown) / 2.0).clamp(0.0, 1.0);
    let colour = looks.map_or([0.8, 0.8, 0.8], |looks| showing(&looks, &palette).0[2]);
    for mut hint in &mut hints {
        hint.color = Color::rgba(colour[0], colour[1], colour[2], 0.7 * fade);
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Avalanche".into(),
        window_size: (ARENA.x as u32, ARENA.y as u32),
        clear_color: paint(PALETTES[0].1.2, 1.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Palette::default())
    .insert_resource(Muted::default())
    .insert_resource(Showing::default())
    .add_startup(setup)
    .add_frame_system(look_controls)
    .add_frame_system(advance_palette)
    .add_frame_system(paint_table)
    .add_frame_system(draw_histogram)
    .add_frame_system(readouts)
    .add_frame_system(play_sounds)
    .add_frame_system(play_handful)
    .add_frame_system(hint)
    .run();
}
