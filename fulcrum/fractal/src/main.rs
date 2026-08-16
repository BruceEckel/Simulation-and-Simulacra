//! Fractals, windowed: everything you can see. The mathematics lives in `game.rs` and has no
//! opinion about color; this binary is the half that does.
//!
//! `cargo run -p fractal --release`
//!
//! - `1`-`0` pick a fractal, `Tab` walks along the row
//! - arrows pan, `Z`/`X` zoom, and the wheel or a click zooms at the pointer
//! - `Q`/`E` spend fewer or more iterations on each point
//! - `P` changes palette, `C` stops and starts the colors cycling
//! - `R` returns to where this fractal started, `Space` holds everything still
//! - `H` puts the readout away

use fractal::game::{
    self, CLOUD_POINTS, Cloud, Court, Depth, FRACTALS, Field, GamePlugin, Grid, Motion, Sample,
    Selection, Speck, View,
};
use fulcrum::prelude::*;
use simulacra_assets::assets;

/// Readout text height in world units. The built-in pixel font is sharpest at multiples of 8.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the picture's top-left corner.
const READOUT_MARGIN: f32 = 12.0;
/// Size of the readout's backing panel, in world units. Fixed rather than measured: the
/// readout's shape never changes, only its numbers.
const PANEL: Vec2 = Vec2::new(648.0, 84.0);
/// How many characters wide the readout is written to be. Only used to push the progress note
/// out to the right-hand end of the title line.
const LINE_WIDTH: usize = 78;

/// How much bigger than its cell each patch is drawn. A hair of overlap, so that rounding
/// cannot leave a grid of dark seams across the picture.
const PATCH_OVERLAP: f32 = 1.02;
/// How big a cloud's specks are, as a fraction of what a cell would have been. Under one, so
/// that a dense part of an attractor still reads as dense rather than filling in solid.
///
/// Sized against a cell rather than in pixels so that a speck is the same size as the finest
/// detail the other family can show. It does not change with the zoom: zooming into a cloud
/// should bring more of the attractor into view, not magnify the dots it is drawn with.
const SPECK_SCALE: f32 = 0.75;

/// How many turns of color an escape count is spread over.
///
/// The square root under it is the important part. Escape counts pile up towards the boundary
/// — that is what a boundary is — so plotting them straight spends every band but the first in
/// the last pixel before the edge. Taking the root spreads them out where you can see them.
const BAND_DENSITY: f32 = 0.55;

/// How bright the brightest part of an interior gets. Well under one: the interior has real
/// structure and is worth showing, but it is still the part of the picture that is *not* the
/// boundary, and it should not compete with the bands for attention.
const INTERIOR_LIGHT: f32 = 0.55;

/// The dimmest a speck is allowed to be, as its brightest channel.
///
/// A cloud is drawn on a near-black ground, and a palette's darkest stops would simply
/// disappear into it — the fern would come out with a missing frond. Lifting a speck scales
/// all three channels together rather than washing it towards white, so it keeps its hue.
const SPECK_FLOOR: f32 = 0.55;

/// How far apart the maps of a chaos game are placed on the color ring, so that which map put
/// a speck somewhere is legible as its color.
const MAP_SPREAD: f32 = 0.17;
/// How many turns of color are laid across a cloud from one corner to the other.
const TONE_TURNS: f32 = 0.6;

/// Turns of the color ring per second while cycling.
const CYCLE_RATE: f32 = 0.05;

/// The shared white texture every square here is a tinted copy of.
#[derive(Resource)]
struct White(Handle<Texture>);

/// One cell of an escape-time picture.
#[derive(Component)]
struct Patch(usize);

/// One point of a chaos-game cloud.
#[derive(Component)]
struct Speckle(usize);

/// Marks the readout.
#[derive(Component)]
struct Readout;

/// Marks the panel that keeps the readout legible over a busy picture.
#[derive(Component)]
struct Panel;

/// A palette is a ring of colors rather than a line of them: the last stop blends back into
/// the first.
///
/// That is not decoration. An escape count has no top — points arbitrarily close to the
/// boundary take arbitrarily long to leave — so a ramp with two ends would spend the whole
/// interesting part of every picture parked against one of them. A ring never runs out.
struct PaletteSpec {
    /// Its name, for the readout.
    name: &'static str,
    /// Its stops, evenly spaced around the ring.
    stops: &'static [[f32; 3]],
}

/// Seven palettes, each closing back on itself.
const PALETTES: &[PaletteSpec] = &[
    PaletteSpec {
        name: "Nebula",
        stops: &[
            [0.03, 0.01, 0.13],
            [0.25, 0.05, 0.42],
            [0.66, 0.11, 0.52],
            [0.98, 0.44, 0.35],
            [1.00, 0.87, 0.60],
            [0.35, 0.55, 0.85],
            [0.08, 0.16, 0.42],
        ],
    },
    PaletteSpec {
        name: "Aurora",
        stops: &[
            [0.01, 0.06, 0.10],
            [0.03, 0.35, 0.30],
            [0.20, 0.85, 0.55],
            [0.62, 0.98, 0.72],
            [0.30, 0.75, 0.90],
            [0.35, 0.30, 0.80],
            [0.10, 0.10, 0.28],
        ],
    },
    PaletteSpec {
        name: "Ember",
        stops: &[
            [0.02, 0.00, 0.01],
            [0.35, 0.02, 0.02],
            [0.80, 0.14, 0.02],
            [0.99, 0.53, 0.06],
            [1.00, 0.88, 0.45],
            [1.00, 0.98, 0.90],
            [0.45, 0.16, 0.08],
        ],
    },
    PaletteSpec {
        name: "Spectrum",
        stops: &[
            [1.00, 0.15, 0.20],
            [1.00, 0.65, 0.10],
            [0.95, 0.95, 0.15],
            [0.25, 0.85, 0.30],
            [0.15, 0.80, 0.85],
            [0.20, 0.35, 0.90],
            [0.60, 0.25, 0.85],
        ],
    },
    PaletteSpec {
        name: "Ice",
        stops: &[
            [0.01, 0.03, 0.10],
            [0.05, 0.20, 0.45],
            [0.20, 0.55, 0.80],
            [0.55, 0.85, 0.95],
            [0.92, 0.98, 1.00],
            [0.35, 0.60, 0.85],
        ],
    },
    PaletteSpec {
        name: "Candy",
        stops: &[
            [0.99, 0.72, 0.82],
            [0.99, 0.88, 0.70],
            [0.78, 0.96, 0.75],
            [0.70, 0.90, 0.98],
            [0.80, 0.75, 0.98],
            [0.98, 0.75, 0.92],
        ],
    },
    PaletteSpec {
        name: "Peacock",
        stops: &[
            [0.02, 0.10, 0.14],
            [0.02, 0.42, 0.45],
            [0.10, 0.72, 0.62],
            [0.85, 0.72, 0.22],
            [0.72, 0.30, 0.15],
            [0.30, 0.10, 0.35],
            [0.05, 0.18, 0.40],
        ],
    },
];

/// Everything the pictures are colored by. None of it is simulation state: change any of it
/// and the same numbers come out in different colors, with nothing recomputed.
#[derive(Resource)]
struct Painter {
    /// Which of [`PALETTES`] is in use.
    palette: usize,
    /// How far the palette has been turned, in whole rings.
    shift: f32,
    /// Whether it keeps turning.
    cycling: bool,
    /// Whether the readout is shown.
    readout: bool,
}

impl Default for Painter {
    fn default() -> Self {
        Self {
            palette: 0,
            shift: 0.0,
            cycling: true,
            readout: true,
        }
    }
}

impl Painter {
    /// The palette in use.
    fn spec(&self) -> &'static PaletteSpec {
        &PALETTES[self.palette % PALETTES.len()]
    }
}

/// A color from a place on the ring. `turn` wraps, so it can be any number at all.
fn ring(stops: &[[f32; 3]], turn: f32) -> Color {
    let count = stops.len();
    let place = turn.rem_euclid(1.0) * count as f32;
    let low = (place as usize) % count;
    let high = (low + 1) % count;
    let raw = place - place.floor();
    // Smoothstep rather than a straight blend: with only a handful of stops, a linear one
    // leaves a visible crease at every stop, and there is nothing in the mathematics that
    // deserves a crease.
    let blend = raw * raw * (3.0 - 2.0 * raw);
    Color::rgb(
        stops[low][0] + (stops[high][0] - stops[low][0]) * blend,
        stops[low][1] + (stops[high][1] - stops[low][1]) * blend,
        stops[low][2] + (stops[high][2] - stops[low][2]) * blend,
    )
}

/// What color one point of an escape-time picture is.
fn field_color(sample: Sample, spec: &PaletteSpec, shift: f32) -> Color {
    if sample.inside {
        // The inside of a set is not empty. `value` is how near the orbit ever came to the
        // origin, and that is a picture in its own right — the filaments and whorls that a
        // flat black interior throws away. Kept dark all the same, so the boundary still
        // reads as the edge of something.
        let near = sample.value.clamp(0.0, 1.0).powf(0.40);
        let tint = ring(spec.stops, shift + 0.5 + near * 0.2);
        Color::rgb(
            tint.r * near * INTERIOR_LIGHT,
            tint.g * near * INTERIOR_LIGHT,
            tint.b * near * INTERIOR_LIGHT,
        )
    } else {
        // A third of the ring per basin, so which of Newton's three roots a point ran to is
        // legible as a color before you have counted a single band. Every other fractal here
        // has one basin and takes no offset.
        let basin = sample.band as f32 / 3.0;
        ring(
            spec.stops,
            sample.value.max(0.0).sqrt() * BAND_DENSITY + basin + shift,
        )
    }
}

/// What color one speck of a cloud is.
fn cloud_color(speck: Speck, spec: &PaletteSpec, shift: f32) -> Color {
    let base = ring(
        spec.stops,
        shift + speck.band as f32 * MAP_SPREAD + speck.tone * TONE_TURNS,
    );
    let brightest = base.r.max(base.g).max(base.b);
    if brightest >= SPECK_FLOOR || brightest <= f32::EPSILON {
        return base;
    }
    let gain = SPECK_FLOOR / brightest;
    Color::rgb(base.r * gain, base.g * gain, base.b * gain)
}

/// Load the texture and put the readout up. The pictures themselves are spawned by
/// [`sync_pictures`], which has to wait until it knows what shape they are.
fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let white = assets.load("white.png");
    commands.insert_resource(White(white));
    commands.spawn((
        Panel,
        Sprite::new(white)
            .with_size(PANEL)
            .with_color(Color::rgba(0.02, 0.02, 0.04, 0.74))
            .with_z(9.0),
        Transform2D::default(),
    ));
    commands.spawn((
        Readout,
        Text::new("").with_size(READOUT_SIZE).with_z(10.0),
        Transform2D::default(),
    ));
}

/// Keep window, camera and court in step, and put the readout in the corner of whatever shape
/// the window has ended up. The resize itself is a command the simulation applies; the zoom is
/// cosmetic and applied here.
fn fit_window(
    window: Res<WindowInfo>,
    court: Res<Court>,
    mut camera: ResMut<Camera2D>,
    mut outbox: ResMut<CommandOutbox>,
    mut requested: Local<Option<Vec2>>,
    mut readouts: Query<&mut Transform2D, With<Readout>>,
    mut panels: Query<&mut Transform2D, (With<Panel>, Without<Readout>)>,
) {
    let size = vec2(window.width as f32, window.height as f32);
    if size.x < 1.0 || size.y < 1.0 {
        return; // minimized
    }
    if *requested != Some(size) {
        outbox.send(game::RESIZE_COMMAND, game::window_payload(size));
        *requested = Some(size);
    }
    camera.zoom = (size.x / court.0.x).max(size.y / court.0.y);

    let corner = vec2(
        -court.0.x / 2.0 + READOUT_MARGIN,
        court.0.y / 2.0 - READOUT_MARGIN,
    );
    for mut readout in &mut readouts {
        readout.translation = corner;
    }
    // The text hangs down and to the right of its corner; the panel is centred on that block.
    for mut panel in &mut panels {
        panel.translation = corner + vec2(PANEL.x / 2.0 - READOUT_MARGIN, -PANEL.y / 2.0 + 6.0);
    }
}

/// Keep the right sprites in existence: a grid of patches for an escape-time fractal, a bag of
/// specks for a chaos-game one, and never both at once.
///
/// Rebuilt only when the answer changes — a new fractal, or a window resized enough to change
/// the grid — because rebuilding it is tens of thousands of entities and doing that per frame
/// would be the most expensive thing in the program by a wide margin.
fn sync_pictures(
    mut commands: Commands,
    white: Option<Res<White>>,
    grid: Res<Grid>,
    selection: Res<Selection>,
    mut built: Local<Option<(Grid, bool)>>,
    patches: Query<Entity, With<Patch>>,
    speckles: Query<Entity, With<Speckle>>,
) {
    let Some(white) = white else { return };
    let cloud = selection.0.is_cloud();
    let wanted = (*grid, cloud);
    if *built == Some(wanted) {
        return;
    }
    *built = Some(wanted);

    for entity in &patches {
        commands.entity(entity).despawn();
    }
    for entity in &speckles {
        commands.entity(entity).despawn();
    }

    if cloud {
        for slot in 0..CLOUD_POINTS {
            commands.spawn((
                Speckle(slot),
                Sprite::new(white.0).with_z(2.0),
                Transform2D::default(),
            ));
        }
    } else {
        for slot in 0..grid.cells() {
            commands.spawn((
                Patch(slot),
                Sprite::new(white.0).with_z(1.0),
                Transform2D::default(),
            ));
        }
    }
}

/// Lay out and color the escape-time picture.
fn paint_patches(
    grid: Res<Grid>,
    court: Res<Court>,
    field: Res<Field>,
    painter: Res<Painter>,
    mut patches: Query<(&Patch, &mut Sprite, &mut Transform2D)>,
) {
    // A resize lands on the grid and the field on different ticks; skip the frame in between
    // rather than draw one against the other's shape.
    if field.samples.len() != grid.cells() {
        return;
    }
    let cell = vec2(
        court.0.x / grid.width as f32,
        court.0.y / grid.height as f32,
    );
    let corner = -court.0 / 2.0;
    let spec = painter.spec();
    for (patch, mut sprite, mut transform) in &mut patches {
        let Some(&sample) = field.samples.get(patch.0) else {
            continue;
        };
        let column = (patch.0 % grid.width as usize) as f32;
        let row = (patch.0 / grid.width as usize) as f32;
        transform.translation = corner + vec2((column + 0.5) * cell.x, (row + 0.5) * cell.y);
        sprite.custom_size = Some(cell * PATCH_OVERLAP);
        sprite.color = field_color(sample, spec, painter.shift);
    }
}

/// Lay out and color the chaos-game picture.
fn paint_speckles(
    grid: Res<Grid>,
    court: Res<Court>,
    view: Res<View>,
    cloud: Res<Cloud>,
    painter: Res<Painter>,
    mut speckles: Query<(&Speckle, &mut Sprite, &mut Transform2D)>,
) {
    let size = Vec2::splat(court.0.x / grid.width as f32 * SPECK_SCALE);
    let spec = painter.spec();
    for (speckle, mut sprite, mut transform) in &mut speckles {
        let Some(&speck) = cloud.points.get(speckle.0) else {
            // A cloud that has not filled yet: the spare specks wait, taking up no room.
            sprite.custom_size = Some(Vec2::ZERO);
            sprite.color = Color::rgba(0.0, 0.0, 0.0, 0.0);
            continue;
        };
        transform.translation =
            view.world_of_complex(*grid, court.0, speck.x as f64, speck.y as f64);
        sprite.custom_size = Some(size);
        sprite.color = cloud_color(speck, spec, painter.shift);
    }
}

/// `P` changes palette, `C` stops and starts the cycling, `H` puts the readout away.
///
/// Debounced against the previous frame rather than using `just_pressed`, since a frame system
/// can see the same tick's edge twice.
fn painter_controls(
    input: Res<Input>,
    time: Res<Time>,
    motion: Res<Motion>,
    mut painter: ResMut<Painter>,
    mut held: Local<[bool; 3]>,
) {
    let down = [
        input.pressed(Key::P),
        input.pressed(Key::C),
        input.pressed(Key::H),
    ];
    if down[0] && !held[0] {
        painter.palette = (painter.palette + 1) % PALETTES.len();
    }
    if down[1] && !held[1] {
        painter.cycling = !painter.cycling;
    }
    if down[2] && !held[2] {
        painter.readout = !painter.readout;
    }
    *held = down;

    // Turning the ring costs nothing: it changes how the samples are colored, not what they
    // are, so a cycling picture recomputes exactly as often as a still one, which is never.
    if painter.cycling && motion.running {
        painter.shift = (painter.shift + CYCLE_RATE * time.frame_delta).rem_euclid(1.0);
    }
}

/// How many decimal places the centre deserves at this magnification. Deep in, the first eight
/// digits stop changing and the rest are the whole story.
fn places(span: f64) -> usize {
    ((-span.log10()).ceil().max(0.0) as usize + 4).min(17)
}

/// A magnification, short enough to read at a glance.
fn zoom_label(zoom: f64) -> String {
    if zoom < 10_000.0 {
        format!("{zoom:.1}x")
    } else {
        format!("{zoom:.1e}x")
    }
}

/// What the viewer is showing and what it is still working on.
#[expect(clippy::too_many_arguments, reason = "a readout reads everything")]
fn readout(
    selection: Res<Selection>,
    view: Res<View>,
    grid: Res<Grid>,
    depth: Res<Depth>,
    field: Res<Field>,
    cloud: Res<Cloud>,
    motion: Res<Motion>,
    painter: Res<Painter>,
    mut texts: Query<&mut Text, With<Readout>>,
    mut panels: Query<&mut Sprite, With<Panel>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    for mut panel in &mut panels {
        panel.color.a = if painter.readout { 0.74 } else { 0.0 };
    }
    if !painter.readout {
        text.value = String::new();
        return;
    }

    let kind = selection.0;
    // Against its own home, not against a fixed scale: the fern and the Mandelbrot set start
    // at wildly different widths, and "twice as close as I started" is what you want to know.
    let zoom = kind.home(*grid).span / view.span;
    let digits = places(view.span);
    let progress = if kind.is_cloud() {
        format!("{:>3.0}% drawn", cloud.fullness() * 100.0)
    } else {
        format!("detail 1:{}", field.detail())
    };
    let effort = if kind.is_cloud() {
        format!("specks {:>6}", cloud.points.len())
    } else {
        format!("iterations {:>4}", depth.0)
    };

    let title = format!(
        "FRACTAL {}/{}  {}",
        kind.index() + 1,
        FRACTALS.len(),
        kind.name()
    );
    // Push the progress note out to the right-hand end of the title line.
    let width = LINE_WIDTH.saturating_sub(title.chars().count());
    text.value = format!(
        "{title}{progress:>width$}\n\
         {}\n\n\
         centre {:>+.digits$} {:>+.digits$}   zoom {:>10}   {effort}\n\
         palette {}{}{}\n\n\
         1-0 pick   Tab next   arrows pan   Z/X zoom   wheel or click zooms at pointer\n\
         Q/E iterations   R home   P palette   C cycling   Space hold   H hide",
        kind.blurb(),
        view.center_x,
        view.center_y,
        zoom_label(zoom),
        painter.spec().name,
        if painter.cycling { ", cycling" } else { "" },
        if motion.running { "" } else { "   HELD" },
    );
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Fractals".into(),
        window_size: (game::DEFAULT_WINDOW.x as u32, game::DEFAULT_WINDOW.y as u32),
        clear_color: Color::rgb(0.01, 0.01, 0.02),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Painter::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(sync_pictures)
    .add_frame_system(painter_controls)
    .add_frame_system(paint_patches)
    .add_frame_system(paint_speckles)
    .add_frame_system(readout)
    .run();
}
