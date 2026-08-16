//! Mesmerize, windowed: the light. The current lives in `game.rs` and stays renderer-free;
//! everything here is color, size and glow.
//!
//! `cargo run -p mesmerize`
//!
//! - move the pointer to draw through the current
//! - `c` changes the palette, `b` shows or hides the breath guide
//! - hold `n` for more light, `m` for less
//! - `space` stills it, `up`/`down` change the pace, `0` restores it, `r` reseeds

use fulcrum::prelude::*;
use mesmerize::game::{
    self, Breath, Census, Field, GamePlugin, Mote, Paused, Speed, Velocity, edge_presence,
};
use simulacra_assets::assets;

/// Mote width across its travel, in world units.
const MOTE_WIDTH: f32 = 7.5;
/// How long a mote is at rest, as a multiple of its width, and how much longer the current
/// draws it out at full speed. Stretching each mote along its heading is what turns a field
/// of dots into a field of strokes — the eye reads a stroke as motion and a dot as an object.
const MOTE_STRETCH: (f32, f32) = (1.6, 3.4);
/// How much the breath swells a mote at its peak.
const MOTE_SWELL: f32 = 0.26;
/// A mote's brightness at its fullest. Low, because the picture is made of overlaps: a
/// hundred faint motes stacking into a bloom looks like light, and a hundred bright ones look
/// like confetti.
const MOTE_ALPHA: f32 = 0.30;
/// How much the breath brightens the field at its peak.
const BREATH_GLOW: f32 = 0.30;
/// Diameter of the breath guide at the bottom of the breath, as a fraction of the field.
const RING_SMALL: f32 = 0.24;
/// And at the top of it.
const RING_LARGE: f32 = 0.50;
/// How bright the guide ring is. Enough to follow if you are looking for it, faint enough to
/// disappear if you are not: an invitation to breathe along, not an object competing with the
/// light.
const RING_ALPHA: f32 = 0.085;
/// Seconds the hint stays up before it fades away and leaves you alone.
const HINT_LIFE: f32 = 12.0;
/// Seconds a palette takes to cross into the next one.
const PALETTE_CROSSFADE: f32 = 2.5;

/// Four palettes, each five stops from its deepest to its brightest.
///
/// All of them are analogous — neighbours on the wheel — rather than complementary. Contrast
/// draws the eye and holds it, which is exactly the wrong thing here; a narrow band of hues
/// lets the eye rest anywhere and keeps the whole field reading as one thing.
///
/// Every stop is luminous, including the deep ones. Truly dark stops seem like the natural
/// bottom of a gradient, but a mote drawn at a third of full alpha over a near-black ground
/// simply disappears — so half of each palette would do nothing, and all four would look the
/// same shade of dim. Depth here comes from how many motes overlap, not from any one being
/// dark.
const PALETTES: [([[f32; 3]; 5], &str); 4] = [
    (
        [
            [0.15, 0.30, 0.68],
            [0.14, 0.55, 0.82],
            [0.24, 0.80, 0.76],
            [0.55, 0.92, 0.72],
            [0.90, 0.98, 0.86],
        ],
        "tide",
    ),
    (
        [
            [0.46, 0.14, 0.52],
            [0.72, 0.20, 0.50],
            [0.92, 0.36, 0.44],
            [1.00, 0.56, 0.40],
            [1.00, 0.86, 0.66],
        ],
        "ember",
    ),
    (
        [
            [0.14, 0.42, 0.36],
            [0.20, 0.62, 0.34],
            [0.46, 0.76, 0.34],
            [0.72, 0.88, 0.44],
            [0.94, 0.97, 0.74],
        ],
        "moss",
    ),
    (
        [
            [0.36, 0.24, 0.68],
            [0.50, 0.32, 0.82],
            [0.66, 0.46, 0.92],
            [0.82, 0.64, 0.96],
            [0.96, 0.89, 1.00],
        ],
        "dusk",
    ),
];

/// Texture handles, loaded once.
#[derive(Resource)]
struct Art {
    mote: Handle<Texture>,
    ring: Handle<Texture>,
}

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

/// Whether the breath guide is showing.
///
/// Off to begin with. The field already swells and brightens on the breath, which carries the
/// rhythm without putting an object in the middle of the picture; the ring is there for when
/// you want something definite to breathe along with, and `b` brings it in.
#[derive(Resource, Default)]
struct Guide(bool);

/// Marks the breath guide ring.
#[derive(Component)]
struct Ring;

/// Marks the hint line.
#[derive(Component)]
struct Hint;

/// A color from one palette at `position` in `0..1`.
fn sample(palette: usize, position: f32) -> [f32; 3] {
    let stops = PALETTES[palette].0;
    let scaled = position.rem_euclid(1.0) * (stops.len() - 1) as f32;
    let low = scaled.floor() as usize;
    let high = (low + 1).min(stops.len() - 1);
    let blend = scaled - low as f32;
    let ease = blend * blend * (3.0 - 2.0 * blend);
    [
        stops[low][0] + (stops[high][0] - stops[low][0]) * ease,
        stops[low][1] + (stops[high][1] - stops[low][1]) * ease,
        stops[low][2] + (stops[high][2] - stops[low][2]) * ease,
    ]
}

/// A color from the palette as it currently stands, mid-crossfade or not.
fn blended(palette: &Palette, position: f32) -> [f32; 3] {
    let to = sample(palette.current, position);
    if palette.blend >= 1.0 {
        return to;
    }
    let from = sample(palette.previous, position);
    let ease = palette.blend * palette.blend * (3.0 - 2.0 * palette.blend);
    [
        from[0] + (to[0] - from[0]) * ease,
        from[1] + (to[1] - from[1]) * ease,
        from[2] + (to[2] - from[2]) * ease,
    ]
}

/// Load the art, and put up the guide ring and the hint.
fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let art = Art {
        mote: assets.load("mote.png"),
        ring: assets.load("ring.png"),
    };
    commands.spawn((
        Ring,
        Sprite::new(art.ring).with_z(-1.0),
        Transform2D::default(),
    ));
    commands.spawn((
        Hint,
        Text::new(HINT_LINES)
            .with_size(8.0)
            .with_align(HAlign::Center)
            .with_z(10.0),
        Transform2D::default(),
    ));
    commands.insert_resource(art);
}

/// Keep window, camera, and field in step, and hold the hint at the bottom of the frame.
fn fit_window(
    window: Res<WindowInfo>,
    field: Res<Field>,
    mut camera: ResMut<Camera2D>,
    mut outbox: ResMut<CommandOutbox>,
    mut requested: Local<Option<Vec2>>,
    mut hints: Query<&mut Transform2D, With<Hint>>,
) {
    let size = vec2(window.width as f32, window.height as f32);
    if size.x < 1.0 || size.y < 1.0 {
        return;
    }
    let wanted = game::field_for_window(size);
    if wanted != field.0 && *requested != Some(wanted) {
        outbox.send(game::FIELD_COMMAND, game::field_payload(wanted));
        *requested = Some(wanted);
    }
    camera.zoom = (size.x / field.0.x).max(size.y / field.0.y);
    for mut hint in &mut hints {
        hint.translation = vec2(0.0, -field.0.y / 2.0 + 34.0);
    }
}

/// C crosses to the next palette, B shows or hides the breath guide. Debounced against the
/// previous frame, since a frame system can see one tick's edge twice.
fn look_controls(
    input: Res<Input>,
    mut palette: ResMut<Palette>,
    mut guide: ResMut<Guide>,
    mut held: Local<(bool, bool)>,
) {
    let (palette_down, guide_down) = (input.pressed(Key::C), input.pressed(Key::B));
    if palette_down && !held.0 {
        palette.previous = palette.current;
        palette.current = (palette.current + 1) % PALETTES.len();
        palette.blend = 0.0;
    }
    if guide_down && !held.1 {
        guide.0 = !guide.0;
    }
    *held = (palette_down, guide_down);
}

/// Give every new mote its sprite.
fn dress_motes(
    mut commands: Commands,
    art: Option<Res<Art>>,
    motes: Query<Entity, (With<Mote>, Without<Sprite>)>,
) {
    let Some(art) = art else { return };
    for mote in &motes {
        commands
            .entity(mote)
            .try_insert(Sprite::new(art.mote).with_z(0.0));
    }
}

/// Color and size every mote.
///
/// Three things multiply into a mote's alpha and none of them ever snaps: where it is in its
/// life, how close it is to the boundary, and where the breath is. The result is that motes
/// arrive, cross, and leave without a single hard edge anywhere in the picture.
fn light_motes(
    mut motes: Query<(&Mote, &Transform2D, &Velocity, &mut Sprite)>,
    field: Res<Field>,
    breath: Res<Breath>,
    palette: Res<Palette>,
    time: Res<Time>,
    mut drift: Local<f32>,
) {
    // The palette itself travels, slowly, so a mote's color is never quite what it was.
    *drift += time.frame_delta * 0.012;
    let swell = 1.0 + MOTE_SWELL * breath.phase;
    let glow = 1.0 - BREATH_GLOW + BREATH_GLOW * breath.phase;
    let width = MOTE_WIDTH * swell;

    for (mote, transform, velocity, mut sprite) in &mut motes {
        let presence = mote.presence() * edge_presence(transform.translation, field.0);
        let color = blended(&palette, mote.stream + *drift);
        // Faster motes draw longer strokes, so the current's shape shows in the marks it
        // leaves as well as in where they are.
        let pace = (velocity.0.length() / game::FLOW_SPEED).clamp(0.0, 1.6);
        let length = width * (MOTE_STRETCH.0 + MOTE_STRETCH.1 * pace);
        sprite.custom_size = Some(vec2(length, width));
        sprite.color = Color::rgba(color[0], color[1], color[2], MOTE_ALPHA * presence * glow);
    }
}

/// Cross the palette over, once a change has been asked for.
fn advance_palette(mut palette: ResMut<Palette>, time: Res<Time>) {
    if palette.blend < 1.0 {
        palette.blend = (palette.blend + time.frame_delta / PALETTE_CROSSFADE).min(1.0);
    }
}

/// The breath guide: a ring that swells and settles on the breath, for you to follow.
fn breath_guide(
    mut rings: Query<(&mut Sprite, &mut Transform2D), With<Ring>>,
    field: Res<Field>,
    breath: Res<Breath>,
    palette: Res<Palette>,
    guide: Res<Guide>,
) {
    let span = field.0.x.min(field.0.y);
    let diameter = span * (RING_SMALL + (RING_LARGE - RING_SMALL) * breath.phase);
    let color = blended(&palette, 0.82);
    for (mut sprite, mut transform) in &mut rings {
        sprite.custom_size = Some(Vec2::splat(diameter));
        // Brightest at the turns of the breath, faintest mid-travel, so the ring pulses with
        // the rhythm rather than merely tracing it.
        let emphasis = 0.6 + 0.4 * (breath.phase * 2.0 - 1.0).abs();
        sprite.color = Color::rgba(
            color[0],
            color[1],
            color[2],
            if guide.0 { RING_ALPHA * emphasis } else { 0.0 },
        );
        transform.translation = Vec2::ZERO;
    }
}

/// The hint fades away after a while and comes back whenever a key is pressed, so the piece
/// spends nearly all of its time with nothing on it but light.
fn hint(
    mut hints: Query<&mut Text, With<Hint>>,
    input: Res<Input>,
    time: Res<Time>,
    palette: Res<Palette>,
    mut shown: Local<f32>,
    mut started: Local<bool>,
) {
    if !*started {
        *shown = 0.0;
        *started = true;
    }
    let touched = [
        Key::C,
        Key::B,
        Key::N,
        Key::M,
        Key::R,
        Key::Space,
        Key::Up,
        Key::Down,
    ]
    .iter()
    .any(|key| input.pressed(*key));
    *shown = if touched {
        0.0
    } else {
        *shown + time.frame_delta
    };

    let fade = ((HINT_LIFE - *shown) / 2.0).clamp(0.0, 1.0);
    let color = blended(&palette, 0.7);
    for mut text in &mut hints {
        text.color = Color::rgba(color[0], color[1], color[2], 0.30 * fade);
    }
}

/// What the hint says. The only text the piece ever shows, and only for a little while.
const HINT_LINES: &str = "move the pointer to draw through it    c palette    b breath guide\n\
                          n / m  more or less light    space still    up / down pace    r reseed";

/// Keep the hint's words current. Separate from the fade so neither has to know about the
/// other's business.
fn hint_text(
    mut hints: Query<&mut Text, With<Hint>>,
    census: Res<Census>,
    speed: Res<Speed>,
    paused: Res<Paused>,
) {
    for mut text in &mut hints {
        text.value = format!(
            "{HINT_LINES}\n{} motes{}{}",
            census.0,
            if speed.0 == 1.0 {
                String::new()
            } else {
                format!("    {:.2}x", speed.0)
            },
            if paused.0 { "    still" } else { "" },
        );
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Mesmerize".into(),
        window_size: (game::DEFAULT_FIELD.x as u32, game::DEFAULT_FIELD.y as u32),
        // Not black: a near-black with a little blue in it gives the motes something to bloom
        // against, and reads as depth rather than as a void.
        clear_color: Color::rgb(0.021, 0.026, 0.045),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Palette::default())
    .insert_resource(Guide::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(look_controls)
    .add_frame_system(advance_palette)
    .add_frame_system(dress_motes)
    .add_frame_system(light_motes)
    .add_frame_system(breath_guide)
    .add_frame_system(hint_text)
    .add_frame_system(hint)
    .run();
}
