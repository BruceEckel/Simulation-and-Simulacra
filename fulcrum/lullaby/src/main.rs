//! Lullaby, windowed: the light and the voice. The night itself lives in `game.rs` and stays
//! renderer-free; everything here is color, size, and sound.
//!
//! `cargo run -p lullaby`
//!
//! - it starts by itself and needs nothing from you
//! - `space` if you are still awake, `s` for sound, `1`-`9` for how long the night is
//! - `up` / `down` for brightness, `r` to begin again

use fulcrum::prelude::*;
use lullaby::game::{
    self, Breath, Depth, Field, GamePlugin, Night, Star, ease, luminance, star_presence,
    voice_level,
};
use simulacra_assets::assets;

/// Width of the smallest and largest star, in world units, before the night narrows them.
const STAR_WIDTH: (f32, f32) = (3.2, 11.0);
/// How much wider a star is drawn at the top of the night than once the sky has settled.
///
/// The light comes into focus as it cools. Early on each star is a wide, soft smudge and the
/// field is a haze; by the end they are small and exact. It is the same arrival the motion
/// makes, said a second way, and it is most of why the settled sky looks like a decision rather
/// than like the haze having run out.
const STAR_FOCUS: (f32, f32) = (1.8, 0.85);
/// A star's alpha at its brightest.
///
/// Low for two reasons. The picture at the beginning is made of overlaps, and a thousand faint
/// smudges stacking up reads as glow where a thousand bright ones reads as spilled sugar. And
/// this is meant to be watched in a dark bedroom by somebody trying to stop being awake, which
/// is a much dimmer picture than anything meant to be looked at.
///
/// Note that these numbers read far darker than they arrive. The renderer takes them as linear
/// light and the display shows them through a gamma curve, so the dark end of the range is
/// stretched hard on the way to the screen: a value here of a tenth lands nearer a third. Every
/// brightness in this file was chosen against the screen rather than against the number.
const STAR_ALPHA: f32 = 0.17;
/// How much the breath brightens the sky at the top of the draw, and how much of that is left
/// once the night has drawn on. It goes to nothing: a sky still pulsing after you have stopped
/// looking at it is a sky still asking for attention.
const BREATH_GLOW: f32 = 0.22;

/// Diameter of the breath halo at the bottom and top of the breath, as a share of the field.
const HALO_SIZE: (f32, f32) = (0.62, 1.05);
/// How bright it is. Barely there; it is meant to be caught at the edge of vision, or through
/// most of a closed eye, and never to be an object in the middle of the picture.
const HALO_ALPHA: f32 = 0.03;

/// The color the window is cleared to at the top of the night. Not quite black: a trace of warm
/// violet gives the stars something to sit in. It goes to true black as the light does, so the
/// end of the night is an honestly dark screen rather than a dark grey one.
///
/// This looks absurdly small written down and is not. See [`STAR_ALPHA`]: the gamma curve does
/// its largest stretching exactly here, at the bottom, so this arrives on screen around seven
/// times brighter than it reads. Written as the near-black it looks like, it would show up as a
/// flat lilac wash, which is the last thing a dark room needs.
const GROUND: [f32; 3] = [0.0030, 0.0019, 0.0042];

/// Seconds the hint stays up before it goes away and leaves you alone.
const HINT_LIFE: f32 = 14.0;
/// How far a held brightness key moves the ceiling per second, and the range it moves in.
const BRIGHTNESS_RATE: f32 = 0.35;
const BRIGHTNESS_LIMITS: (f32, f32) = (0.12, 1.0);
/// Seconds the voice takes to come and go when you ask for it.
const VOICE_TOGGLE: f32 = 3.0;

/// How long the two breath sounds are as recorded, in seconds. The voice is stretched to fit the
/// breath from these.
const DRAW_SECONDS: f32 = 4.5;
const RELEASE_SECONDS: f32 = 5.5;
/// Loudness of each, before the night's own fades are applied. The release is the quieter of the
/// two, as it is in a person.
const DRAW_LEVEL: f32 = 0.55;
const RELEASE_LEVEL: f32 = 0.40;

/// Two palettes, five stops each from the deepest to the brightest.
///
/// Both are warm and neither contains any real blue. That is the one piece of received wisdom
/// about light and sleep worth designing around, and it happens to be the right choice for a
/// picture that spends its whole life getting dimmer: a dim warm color still reads as light,
/// while a dim blue one just reads as cold.
const PALETTES: [([[f32; 3]; 5], &str); 2] = [
    (
        [
            [0.34, 0.11, 0.05],
            [0.60, 0.23, 0.08],
            [0.84, 0.41, 0.14],
            [0.96, 0.62, 0.30],
            [1.00, 0.84, 0.60],
        ],
        "ember",
    ),
    (
        [
            [0.30, 0.24, 0.18],
            [0.52, 0.44, 0.32],
            [0.74, 0.65, 0.49],
            [0.90, 0.83, 0.67],
            [1.00, 0.97, 0.88],
        ],
        "moonlight",
    ),
];

/// Art and sound handles, loaded once.
#[derive(Resource)]
struct Kit {
    star: Handle<Texture>,
    halo: Handle<Texture>,
    draw: Handle<Sound>,
    release: Handle<Sound>,
}

/// Which palette is showing.
#[derive(Resource, Default)]
struct Palette(usize);

/// The ceiling on brightness, for a room darker or lighter than the one this was set up in.
#[derive(Resource)]
struct Brightness(f32);

impl Default for Brightness {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Whether the voice is wanted, and how much of it has arrived.
///
/// Sounds already playing are never cut; the level is only read when a new breath begins, so
/// turning the voice off lets the breath in progress finish on its own.
#[derive(Resource)]
struct Voice {
    wanted: bool,
    level: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            wanted: true,
            level: 0.0,
        }
    }
}

/// Marks the breath halo.
#[derive(Component)]
struct Halo;

/// Marks the hint line.
#[derive(Component)]
struct Hint;

/// A color from the palette at `position` in `0..1`.
fn sample(palette: usize, position: f32) -> [f32; 3] {
    let stops = PALETTES[palette.min(PALETTES.len() - 1)].0;
    let scaled = position.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let low = scaled.floor() as usize;
    let high = (low + 1).min(stops.len() - 1);
    let blend = ease(scaled - low as f32);
    [
        stops[low][0] + (stops[high][0] - stops[low][0]) * blend,
        stops[low][1] + (stops[high][1] - stops[low][1]) * blend,
        stops[low][2] + (stops[high][2] - stops[low][2]) * blend,
    ]
}

/// Load the art and the voice, and put up the halo and the hint.
fn setup(mut commands: Commands, mut assets: AssetLoader, mut sounds: SoundLoader) {
    let kit = Kit {
        star: assets.load("star.png"),
        halo: assets.load("halo.png"),
        draw: sounds.load("draw.wav"),
        release: sounds.load("release.wav"),
    };
    commands.spawn((
        Halo,
        Sprite::new(kit.halo).with_z(-1.0),
        Transform2D::default(),
    ));
    commands.spawn((
        Hint,
        Text::new("")
            .with_size(8.0)
            .with_align(HAlign::Center)
            .with_z(10.0),
        Transform2D::default(),
    ));
    commands.insert_resource(kit);
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
    // Clear of the bottom by more than the three lines it runs to, so the last of them is not
    // sitting on the edge of the window at any shape you might drag it into.
    for mut hint in &mut hints {
        hint.translation = vec2(0.0, -field.0.y / 2.0 + 52.0);
    }
}

/// `c` changes the palette, `s` asks for the voice or sends it away, `up` and `down` set the
/// brightness. Debounced against the previous frame, since a frame system can see one tick's
/// edge twice.
fn look_controls(
    input: Res<Input>,
    time: Res<Time>,
    mut palette: ResMut<Palette>,
    mut voice: ResMut<Voice>,
    mut brightness: ResMut<Brightness>,
    mut held: Local<(bool, bool)>,
) {
    let (palette_down, voice_down) = (input.pressed(Key::C), input.pressed(Key::S));
    if palette_down && !held.0 {
        palette.0 = (palette.0 + 1) % PALETTES.len();
    }
    if voice_down && !held.1 {
        voice.wanted = !voice.wanted;
    }
    *held = (palette_down, voice_down);

    if input.pressed(Key::Up) {
        brightness.0 += BRIGHTNESS_RATE * time.frame_delta;
    }
    if input.pressed(Key::Down) {
        brightness.0 -= BRIGHTNESS_RATE * time.frame_delta;
    }
    brightness.0 = brightness.0.clamp(BRIGHTNESS_LIMITS.0, BRIGHTNESS_LIMITS.1);

    let target = if voice.wanted { 1.0 } else { 0.0 };
    let step = time.frame_delta / VOICE_TOGGLE;
    voice.level = if target > voice.level {
        (voice.level + step).min(1.0)
    } else {
        (voice.level - step).max(0.0)
    };
}

/// Give every star its sprite.
fn dress_stars(
    mut commands: Commands,
    kit: Option<Res<Kit>>,
    stars: Query<Entity, (With<Star>, Without<Sprite>)>,
) {
    let Some(kit) = kit else { return };
    for star in &stars {
        commands
            .entity(star)
            .try_insert(Sprite::new(kit.star).with_z(0.0));
    }
}

/// Color and size every star.
///
/// Four things multiply into a star's alpha and not one of them can move quickly: the ceiling you
/// set, the night's own falling light, this star's private going-out, and the breath. The result
/// is that a star arrives at nothing without ever having been seen to change.
fn light_stars(
    mut stars: Query<(&Star, &mut Sprite)>,
    depth: Res<Depth>,
    breath: Res<Breath>,
    palette: Res<Palette>,
    brightness: Res<Brightness>,
) {
    let ceiling = luminance(depth.now) * brightness.0;
    let glow = BREATH_GLOW * (1.0 - depth.now) * breath.phase;
    let focus = STAR_FOCUS.0 + (STAR_FOCUS.1 - STAR_FOCUS.0) * ease(depth.now / game::SETTLED);

    for (star, mut sprite) in &mut stars {
        let width = (STAR_WIDTH.0 + (STAR_WIDTH.1 - STAR_WIDTH.0) * star.size) * focus;
        sprite.custom_size = Some(Vec2::splat(width));
        let color = sample(palette.0, star.warmth);
        let alpha = STAR_ALPHA * star_presence(star.dim_at, depth.now) * ceiling * (1.0 + glow);
        sprite.color = Color::rgba(color[0], color[1], color[2], alpha.min(1.0));
    }
}

/// The halo: one broad, faint swell in the middle of the window, on the breath.
///
/// This is the piece's only deliberate guide, and it is deliberately almost invisible. Something
/// large and dim can be followed with your eyes half shut, or not followed at all, which is the
/// only kind of instruction worth giving somebody who is trying to stop paying attention.
fn breath_halo(
    mut halos: Query<(&mut Sprite, &mut Transform2D), With<Halo>>,
    field: Res<Field>,
    depth: Res<Depth>,
    breath: Res<Breath>,
    palette: Res<Palette>,
    brightness: Res<Brightness>,
) {
    let span = field.0.x.min(field.0.y);
    let diameter = span * (HALO_SIZE.0 + (HALO_SIZE.1 - HALO_SIZE.0) * breath.phase);
    let color = sample(palette.0, 0.30);
    let alpha = HALO_ALPHA * luminance(depth.now) * brightness.0 * (0.35 + 0.65 * breath.phase);
    for (mut sprite, mut transform) in &mut halos {
        sprite.custom_size = Some(Vec2::splat(diameter));
        sprite.color = Color::rgba(color[0], color[1], color[2], alpha);
        transform.translation = Vec2::ZERO;
    }
}

/// Take the ground down to true black along with everything else.
fn dim_ground(mut config: ResMut<FulcrumConfig>, depth: Res<Depth>, brightness: Res<Brightness>) {
    let level = luminance(depth.now) * brightness.0;
    config.clear_color = Color::rgb(GROUND[0] * level, GROUND[1] * level, GROUND[2] * level);
}

/// The voice: one sound at the start of the draw, another at the start of the release.
///
/// A simulation system rather than a frame system, because it answers an edge (the tick a breath
/// turns over), and a frame system running slower than the simulation can miss one. A missed
/// breath is a silent breath, which is exactly the sort of small wrong event this piece cannot
/// afford. It only reads simulation state, so it cannot affect determinism.
///
/// What it compares is the breath's counters against the ones it last answered, so it needs no
/// ordering against the system that advances them. The counters are noted every tick, silent or
/// not, so that turning the voice back on answers the next breath rather than a backlog of the
/// ones that happened while it was off.
///
/// Both sounds are stretched toward the length of the phase they cover, but only by the square
/// root of it. Stretched the whole way, a sixteen-second breath would drop the release an octave
/// and put most of it under what a laptop can reproduce; stretched by half, it deepens as the
/// breath deepens and then trails off into silence before the bottom of the out-breath, which is
/// where a pause belongs anyway.
fn speak(
    breath: Res<Breath>,
    depth: Res<Depth>,
    voice: Res<Voice>,
    kit: Option<Res<Kit>>,
    sounds: Res<Assets<Sound>>,
    mut audio: ResMut<Audio>,
    mut answered: Local<(u32, u32)>,
) {
    let previous = *answered;
    *answered = (breath.draws, breath.releases);
    let Some(kit) = kit else { return };
    let level = voice.level * voice_level(depth.now, depth.elapsed);
    if level <= 0.001 {
        return;
    }
    let stretch = |recorded: f32, actual: f32| (recorded / actual.max(0.1)).sqrt().clamp(0.4, 1.6);
    if breath.draws != previous.0 {
        audio.play_with(
            &sounds,
            kit.draw,
            PlayParams {
                volume: level * DRAW_LEVEL,
                pitch: stretch(DRAW_SECONDS, breath.period * breath.inhale),
                pan: 0.0,
            },
        );
    }
    if breath.releases != previous.1 {
        audio.play_with(
            &sounds,
            kit.release,
            PlayParams {
                volume: level * RELEASE_LEVEL,
                pitch: stretch(RELEASE_SECONDS, breath.period * (1.0 - breath.inhale)),
                pan: 0.0,
            },
        );
    }
}

/// What the hint says while it is up.
const HINT_LINES: &str = "space  still awake     s  sound     c  color     1-9  length of the night\n\
                          up / down  brightness     r  begin again";

/// Keep the hint's words current. Separate from the fade so that neither has to know about the
/// other's business.
fn hint_text(
    mut hints: Query<&mut Text, With<Hint>>,
    night: Res<Night>,
    depth: Res<Depth>,
    voice: Res<Voice>,
) {
    let left = ((night.0 - depth.elapsed).max(0.0) / 60.0).ceil() as i32;
    for mut text in &mut hints {
        text.value = format!(
            "{HINT_LINES}\n{} minute night, {left} to go{}",
            (night.0 / 60.0).round() as i32,
            if voice.wanted { "" } else { "    silent" },
        );
    }
}

/// The hint goes away after a few seconds and comes back whenever a key is pressed, so the piece
/// spends nearly all of its life with nothing on it at all. It fades on the night's own light, so
/// once the sky is out there is nothing left that could put words on a dark screen.
fn hint(
    mut hints: Query<&mut Text, With<Hint>>,
    input: Res<Input>,
    time: Res<Time>,
    depth: Res<Depth>,
    palette: Res<Palette>,
    brightness: Res<Brightness>,
    mut shown: Local<f32>,
) {
    let touched = [
        Key::Space,
        Key::S,
        Key::C,
        Key::R,
        Key::Up,
        Key::Down,
        Key::Digit1,
        Key::Digit2,
        Key::Digit3,
        Key::Digit4,
        Key::Digit5,
        Key::Digit6,
        Key::Digit7,
        Key::Digit8,
        Key::Digit9,
    ]
    .iter()
    .any(|key| input.pressed(*key));
    *shown = if touched {
        0.0
    } else {
        *shown + time.frame_delta
    };

    let fade = ((HINT_LIFE - *shown) / 2.5).clamp(0.0, 1.0);
    let color = sample(palette.0, 0.55);
    let alpha = 0.34 * fade * luminance(depth.now) * brightness.0;
    for mut text in &mut hints {
        text.color = Color::rgba(color[0], color[1], color[2], alpha);
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Lullaby".into(),
        window_size: (game::DEFAULT_FIELD.x as u32, game::DEFAULT_FIELD.y as u32),
        clear_color: Color::rgb(GROUND[0], GROUND[1], GROUND[2]),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Palette::default())
    .insert_resource(Brightness::default())
    .insert_resource(Voice::default())
    .add_startup(setup)
    .add_system(speak)
    .add_frame_system(fit_window)
    .add_frame_system(look_controls)
    .add_frame_system(dress_stars)
    .add_frame_system(light_stars)
    .add_frame_system(breath_halo)
    .add_frame_system(dim_ground)
    .add_frame_system(hint_text)
    .add_frame_system(hint)
    .run();
}
