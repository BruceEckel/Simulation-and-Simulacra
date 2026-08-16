//! Starry Night, windowed: what the colours are. The painting and its current live in
//! `game.rs` and stay renderer-free; everything here is pigment.
//!
//! `cargo run -p starry --release`
//!
//! - drag through the sky to smear the paint, and let go to watch it find its way back
//! - click to hang a new star, `x` to take the last one down
//! - `c` changes the palette, `h` stops the paint healing, `r` lays the canvas down again
//! - hold `n` for more paint, `m` for less
//! - `space` stills it (and it is a painting again), `up`/`down` change the pace, `0` restores
//!
//! The simulation says only what layer a stroke is painting and where it sits in that layer's
//! range. Nothing in it knows that the sky is blue, which is what lets the same painting be
//! repainted at dawn, or in ink, without a single stroke changing its mind.

use fulcrum::prelude::*;
use simulacra_assets::assets;
use starry::game::{self, CANVAS, Census, GamePlugin, Healing, Layer, Paused, Sky, Speed, Stroke};

/// How solid the paint is. High: this is oil, not light, and the picture is made of strokes
/// covering strokes rather than of glow stacking up.
const PAINT_ALPHA: f32 = 0.94;
/// Seconds a palette takes to cross into the next one.
const PALETTE_CROSSFADE: f32 = 1.6;
/// Seconds the hint stays up before it fades and leaves you to look.
const HINT_LIFE: f32 = 16.0;
/// How far apart the underpainting's blotches sit, in world units.
const COAT_SPACING: f32 = 20.0;
/// How big a blotch is compared to that spacing.
const COAT_SIZE: f32 = 2.1;

/// How many layers there are to give colours to.
const LAYERS: usize = 9;
/// The two ends of one layer's range of colour.
type Ramps = [[[f32; 3]; 2]; LAYERS];

/// Four palettes, each giving every layer a dark end and a light end.
///
/// Written in sRGB, the way a colour picker gives them, and converted to linear light once at
/// startup: the renderer works in linear, where a mid blue is a much smaller number than it
/// looks. Writing these as linear by hand is how paintings end up looking washed out.
///
/// Order: sky, halo, star, moon, hill, ground, village, window, cypress.
const PALETTES: [(&str, Ramps); 4] = [
    (
        "night",
        [
            [[0.035, 0.080, 0.290], [0.580, 0.720, 0.870]],
            [[0.480, 0.380, 0.120], [1.000, 0.930, 0.560]],
            [[0.980, 0.930, 0.680], [1.000, 1.000, 0.940]],
            [[0.980, 0.780, 0.280], [1.000, 0.960, 0.720]],
            [[0.016, 0.038, 0.080], [0.100, 0.180, 0.235]],
            [[0.026, 0.045, 0.070], [0.155, 0.165, 0.125]],
            [[0.025, 0.035, 0.075], [0.380, 0.400, 0.460]],
            [[0.850, 0.580, 0.200], [1.000, 0.880, 0.450]],
            [[0.020, 0.030, 0.025], [0.150, 0.150, 0.060]],
        ],
    ),
    (
        "dawn",
        [
            [[0.180, 0.110, 0.300], [1.000, 0.760, 0.520]],
            [[0.700, 0.330, 0.240], [1.000, 0.880, 0.620]],
            [[1.000, 0.880, 0.700], [1.000, 0.980, 0.940]],
            [[1.000, 0.720, 0.420], [1.000, 0.930, 0.780]],
            [[0.150, 0.100, 0.200], [0.420, 0.300, 0.340]],
            [[0.180, 0.130, 0.170], [0.480, 0.380, 0.300]],
            [[0.080, 0.055, 0.100], [0.300, 0.220, 0.240]],
            [[0.900, 0.560, 0.240], [1.000, 0.860, 0.520]],
            [[0.070, 0.050, 0.060], [0.380, 0.220, 0.140]],
        ],
    ),
    (
        "ink",
        [
            [[0.040, 0.070, 0.130], [0.780, 0.840, 0.900]],
            [[0.300, 0.360, 0.440], [0.900, 0.940, 1.000]],
            [[0.880, 0.920, 0.980], [1.000, 1.000, 1.000]],
            [[0.860, 0.900, 0.960], [1.000, 1.000, 1.000]],
            [[0.060, 0.090, 0.140], [0.360, 0.420, 0.500]],
            [[0.080, 0.110, 0.160], [0.420, 0.470, 0.540]],
            [[0.030, 0.050, 0.090], [0.380, 0.420, 0.480]],
            [[0.700, 0.760, 0.860], [1.000, 1.000, 1.000]],
            [[0.020, 0.030, 0.050], [0.260, 0.300, 0.360]],
        ],
    ),
    (
        "fauve",
        [
            [[0.300, 0.030, 0.260], [1.000, 0.820, 0.200]],
            [[0.900, 0.240, 0.320], [1.000, 0.960, 0.400]],
            [[1.000, 0.950, 0.500], [1.000, 1.000, 0.940]],
            [[1.000, 0.400, 0.240], [1.000, 0.880, 0.400]],
            [[0.060, 0.240, 0.260], [0.200, 0.760, 0.560]],
            [[0.180, 0.080, 0.260], [0.760, 0.300, 0.420]],
            [[0.100, 0.040, 0.160], [0.640, 0.180, 0.300]],
            [[1.000, 0.900, 0.300], [1.000, 1.000, 0.760]],
            [[0.080, 0.020, 0.120], [0.420, 0.120, 0.480]],
        ],
    ),
];

/// Texture handles, loaded once.
#[derive(Resource, Clone)]
struct Brushes {
    stroke: Handle<Texture>,
    dab: Handle<Texture>,
    coat: Handle<Texture>,
}

/// Every palette, converted to linear light once.
#[derive(Resource)]
struct Paints([Ramps; PALETTES.len()]);

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

/// A blotch of the underpainting: the coat that stops the canvas showing between strokes.
#[derive(Component)]
struct Coat {
    layer: Layer,
    tone: f32,
}

/// Marks the hint line.
#[derive(Component)]
struct Hint;

/// A small deterministic generator, for scattering the underpainting. Kept away from `SimRng`:
/// the coat is the view's business, and drawing it from the simulation's stream would let a
/// change of scenery change the painting.
struct Dice(u32);

impl Dice {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 16_777_216.0
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next()
    }
}

/// One sRGB channel in linear light.
fn linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Which set of colours a layer draws from.
fn slot(layer: Layer) -> usize {
    match layer {
        Layer::Sky => 0,
        Layer::Halo => 1,
        Layer::Star => 2,
        Layer::Moon => 3,
        Layer::Hill => 4,
        Layer::Ground => 5,
        Layer::Village => 6,
        Layer::Window => 7,
        Layer::Cypress => 8,
    }
}

/// The colour of one layer at one tone, in a single palette.
fn pigment(paints: &Paints, palette: usize, layer: Layer, tone: f32) -> [f32; 3] {
    let ramp = paints.0[palette][slot(layer)];
    let ease = tone.clamp(0.0, 1.0);
    [
        ramp[0][0] + (ramp[1][0] - ramp[0][0]) * ease,
        ramp[0][1] + (ramp[1][1] - ramp[0][1]) * ease,
        ramp[0][2] + (ramp[1][2] - ramp[0][2]) * ease,
    ]
}

/// The colour as it currently stands, mid-crossfade or not.
fn blended(paints: &Paints, palette: &Palette, layer: Layer, tone: f32) -> [f32; 3] {
    let to = pigment(paints, palette.current, layer, tone);
    if palette.blend >= 1.0 {
        return to;
    }
    let from = pigment(paints, palette.previous, layer, tone);
    let ease = palette.blend * palette.blend * (3.0 - 2.0 * palette.blend);
    [
        from[0] + (to[0] - from[0]) * ease,
        from[1] + (to[1] - from[1]) * ease,
        from[2] + (to[2] - from[2]) * ease,
    ]
}

/// Load the brushes, mix the paints, and lay the underpainting.
fn setup(
    mut commands: Commands,
    mut assets: AssetLoader,
    mut camera: ResMut<Camera2D>,
    sky: Res<Sky>,
) {
    // A painting has a shape of its own, so the canvas keeps its proportions and the window
    // gets bars rather than the composition being stretched to fit it.
    camera.scaling = ScalingMode::Letterbox {
        width: CANVAS.x,
        height: CANVAS.y,
    };

    let brushes = Brushes {
        stroke: assets.load("stroke.png"),
        dab: assets.load("dab.png"),
        coat: assets.load("coat.png"),
    };

    let mut paints = PALETTES.map(|(_, ramps)| ramps);
    for palette in &mut paints {
        for ramp in palette.iter_mut() {
            for end in ramp.iter_mut() {
                for channel in end.iter_mut() {
                    *channel = linear(*channel);
                }
            }
        }
    }

    lay_coat(&mut commands, &brushes, &sky);
    commands.spawn((
        Hint,
        Text::new(HINT_LINES)
            .with_size(8.0)
            .with_align(HAlign::Center)
            .with_z(10.0),
        Transform2D::from_translation(vec2(0.0, -CANVAS.y / 2.0 + 26.0)),
    ));
    commands.insert_resource(Paints(paints));
    commands.insert_resource(brushes);
}

/// The first coat: a jittered grid of blotches, each the colour of what belongs there.
///
/// Strokes are scattered, and scattered cover leaves holes however much of it there is. The
/// coat is what a painter does first for the same reason: it takes the canvas out of the
/// picture so the strokes only have to be paint, not cover.
fn lay_coat(commands: &mut Commands, brushes: &Brushes, sky: &Sky) {
    let mut dice = Dice(0x51A2_7E3D);
    let across = (CANVAS.x / COAT_SPACING).ceil() as i32;
    let up = (CANVAS.y / COAT_SPACING).ceil() as i32;
    for row in 0..up {
        for column in 0..across {
            let at = vec2(
                -CANVAS.x / 2.0 + (column as f32 + 0.5) * COAT_SPACING,
                -CANVAS.y / 2.0 + (row as f32 + 0.5) * COAT_SPACING,
            ) + vec2(
                dice.range(-0.45, 0.45) * COAT_SPACING,
                dice.range(-0.45, 0.45) * COAT_SPACING,
            );
            let (layer, tone) = game::paint_at(at, sky, 0.0);
            let angle = if layer.airborne() {
                game::flow(at, sky, 0.0).to_angle()
            } else {
                game::lie_of_the_land(layer, at)
            };
            let mut sprite = Sprite::new(brushes.coat).with_z(-1.0);
            sprite.custom_size = Some(Vec2::splat(COAT_SPACING * COAT_SIZE * dice.range(0.8, 1.2)));
            commands.spawn((
                Coat { layer, tone },
                sprite,
                Transform2D {
                    translation: at,
                    rotation: angle,
                    scale: Vec2::ONE,
                },
            ));
        }
    }
}

/// C crosses to the next palette. Debounced against the previous frame, since a frame system
/// can see one tick's edge twice.
fn look_controls(input: Res<Input>, mut palette: ResMut<Palette>, mut held: Local<bool>) {
    let down = input.pressed(Key::C);
    if down && !*held {
        palette.previous = palette.current;
        palette.current = (palette.current + 1) % PALETTES.len();
        palette.blend = 0.0;
    }
    *held = down;
}

/// Cross the palette over, once a change has been asked for.
fn advance_palette(mut palette: ResMut<Palette>, time: Res<Time>) {
    if palette.blend < 1.0 {
        palette.blend = (palette.blend + time.frame_delta / PALETTE_CROSSFADE).min(1.0);
    }
}

/// Give every new stroke a brush.
fn dress(
    mut commands: Commands,
    brushes: Option<Res<Brushes>>,
    strokes: Query<Entity, (With<Stroke>, Without<Sprite>)>,
) {
    let Some(brushes) = brushes else { return };
    for entity in &strokes {
        commands
            .entity(entity)
            .try_insert(Sprite::new(brushes.stroke));
    }
}

/// Colour and size every stroke of paint.
fn paint_strokes(
    mut strokes: Query<(&Stroke, &mut Sprite)>,
    brushes: Option<Res<Brushes>>,
    paints: Option<Res<Paints>>,
    palette: Res<Palette>,
) {
    let (Some(brushes), Some(paints)) = (brushes, paints) else {
        return;
    };
    for (stroke, mut sprite) in &mut strokes {
        let colour = blended(&paints, &palette, stroke.layer, stroke.tone);
        // A star is a dab of the brush put down and lifted, not a stroke drawn along.
        sprite.texture = match stroke.layer {
            Layer::Star | Layer::Moon | Layer::Window => brushes.dab,
            _ => brushes.stroke,
        };
        sprite.custom_size = Some(stroke.size);
        // Strokes cross each other in no particular order, which is what a painted surface is.
        sprite.z = stroke.seed;
        sprite.color = Color::rgba(
            colour[0],
            colour[1],
            colour[2],
            PAINT_ALPHA * stroke.presence(),
        );
    }
}

/// Colour the underpainting.
fn paint_coat(
    mut coats: Query<(&Coat, &mut Sprite)>,
    paints: Option<Res<Paints>>,
    palette: Res<Palette>,
) {
    let Some(paints) = paints else { return };
    for (coat, mut sprite) in &mut coats {
        let colour = blended(&paints, &palette, coat.layer, coat.tone);
        sprite.color = Color::rgb(colour[0], colour[1], colour[2]);
    }
}

/// What the hint says. The only text the piece ever shows, and only for a little while.
const HINT_LINES: &str = "drag to smear the paint    click the sky for a star    x takes one down\n\
                          c palette    h healing    n / m more or less paint    r repaint    space still";

/// Keep the hint's words current. Separate from the fade so neither has to know about the
/// other's business.
fn hint_text(
    mut hints: Query<&mut Text, With<Hint>>,
    census: Res<Census>,
    sky: Res<Sky>,
    speed: Res<Speed>,
    paused: Res<Paused>,
    healing: Res<Healing>,
    palette: Res<Palette>,
) {
    for mut hint in &mut hints {
        hint.value = format!(
            "{HINT_LINES}\n{}    {} strokes    {} stars{}{}{}",
            PALETTES[palette.current].0,
            census.0,
            sky.stars.len(),
            if speed.0 == 1.0 {
                String::new()
            } else {
                format!("    {:.2}x", speed.0)
            },
            if paused.0 { "    still" } else { "" },
            if healing.0 { "" } else { "    healing off" },
        );
    }
}

/// The hint fades away and comes back whenever a key is pressed, so the piece spends nearly all
/// of its time with nothing on it but paint.
fn hint(
    mut hints: Query<&mut Text, With<Hint>>,
    input: Res<Input>,
    time: Res<Time>,
    paints: Option<Res<Paints>>,
    palette: Res<Palette>,
    mut shown: Local<f32>,
) {
    let touched = [
        Key::C,
        Key::H,
        Key::N,
        Key::M,
        Key::R,
        Key::X,
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
    // The hint is written in the same paint as the stars, so it belongs to the picture.
    let colour = paints
        .map(|paints| pigment(&paints, palette.current, Layer::Halo, 0.9))
        .unwrap_or([0.9, 0.9, 0.8]);
    for mut hint in &mut hints {
        hint.color = Color::rgba(colour[0], colour[1], colour[2], 0.5 * fade);
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Starry Night".into(),
        window_size: (CANVAS.x as u32, CANVAS.y as u32),
        // The bars around a letterboxed canvas, and the ground the first coat is laid on.
        clear_color: Color::rgb(0.006, 0.008, 0.020),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Palette::default())
    .add_startup(setup)
    .add_frame_system(look_controls)
    .add_frame_system(advance_palette)
    .add_frame_system(dress)
    .add_frame_system(paint_strokes)
    .add_frame_system(paint_coat)
    .add_frame_system(hint_text)
    .add_frame_system(hint)
    .run();
}
