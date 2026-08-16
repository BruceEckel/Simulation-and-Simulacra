//! The jig, windowed: what the bones look like and what they sound like. The body and its
//! physics live in `game.rs` and have no opinion about any of this.
//!
//! `cargo run -p jig --release`
//!
//! - `up`/`down` set the tempo, all the way down to no music at all
//! - `left`/`right` set the tone: how hard the joints hold the pose
//! - `1`-`5` pick a step, `R` stands the skeleton back up
//! - `P` changes palette, `M` mutes, `H` puts the readout away
//!
//! Nothing here is animated. The only thing anybody told this skeleton to do is move its hips
//! along a small closed curve; the arms, the spine and the shins are all working it out for
//! themselves, eighteen pendulums at a time.

use fulcrum::prelude::*;
use jig::game::{
    BONES, Beat, GamePlugin, Part, Routine, Side, Skeleton, TONE_MAX, Tone, direction,
};
use simulacra_assets::assets;

/// How tall a stage the skeleton dances on, in world units. The figure is about 380 of them
/// from heel to crown, so this leaves it room to move without ever leaving the window.
const STAGE: f32 = 560.0;
/// How much of that the window has to be able to show across, so a narrow window shrinks the
/// figure rather than cropping it.
const STAGE_ACROSS: f32 = 460.0;

/// How thick each kind of bone is drawn, in world units.
const fn thickness(part: Part) -> f32 {
    match part {
        Part::Spine | Part::Chest => 15.0,
        Part::Neck => 10.0,
        Part::Skull => 12.0,
        Part::Collar => 9.0,
        Part::UpperArm | Part::Forearm => 12.0,
        Part::Hand | Part::Foot => 8.0,
        Part::Thigh => 17.0,
        Part::Shin => 14.0,
    }
}

/// How big a knob sits at each kind of joint.
const fn knuckle(part: Part) -> f32 {
    match part {
        Part::Spine | Part::Chest => 16.0,
        Part::Thigh => 21.0,
        Part::Shin => 17.0,
        Part::UpperArm => 16.0,
        Part::Forearm => 14.0,
        _ => 10.0,
    }
}

/// How near the front of the picture a bone is drawn. The two sides of a body overlap, and
/// something has to be in front; putting the whole of one arm ahead of the trunk and the whole
/// of the other behind it reads as a body seen from the front rather than as a diagram.
fn depth(side: Side) -> f32 {
    match side {
        Side::Left => 1.0,
        Side::Middle => 3.0,
        Side::Right => 5.0,
    }
}

/// How big the skull is drawn, as a multiple of the length of the bone it is hung on.
const SKULL_SIZE: f32 = 1.18;
/// How big the ribcage is drawn, as a multiple of the chest bone's length.
const RIBS_SIZE: f32 = 1.6;
/// How big the pelvis is drawn.
const PELVIS_SIZE: Vec2 = Vec2::new(64.0, 44.0);
/// How big a hand and a foot are drawn.
const HAND_SIZE: Vec2 = Vec2::new(24.0, 30.0);
/// How big a foot is drawn.
const FOOT_SIZE: Vec2 = Vec2::new(22.0, 34.0);
/// How wide the pool of light is, and how tall.
const GLOW_SIZE: Vec2 = Vec2::new(720.0, 620.0);

/// Readout text height in world units. The built-in pixel font is sharpest at multiples of 8.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the corner of the window.
const READOUT_MARGIN: f32 = 14.0;
/// Size of the readout's backing panel, in world units.
const PANEL: Vec2 = Vec2::new(690.0, 80.0);
/// How many characters wide the readout is written to be.
const LINE_WIDTH: usize = 76;

/// Seconds a knock takes to fade out of the sound budget, so that a joint chattering against
/// its stop is one noise and not two hundred.
const KNOCK_GAP: f32 = 0.035;
/// How many knocks a second may be heard at once, at most.
const KNOCK_BUDGET: f32 = 14.0;

/// One palette: the bone, the shadowed side of it, the ground, and the light it stands in.
struct Look {
    /// Its name, for the readout.
    name: &'static str,
    /// The bone itself, at its brightest.
    bone: [f32; 3],
    /// What the shaded parts of a bone go towards.
    shade: [f32; 3],
    /// Behind everything.
    ground: [f32; 3],
    /// The pool of light on the ground.
    glow: [f32; 3],
}

/// Four palettes, written in sRGB and converted to linear light once at startup.
const LOOKS: &[Look] = &[
    Look {
        name: "bone",
        bone: [0.976, 0.949, 0.878],
        shade: [0.310, 0.271, 0.239],
        ground: [0.055, 0.051, 0.059],
        glow: [0.259, 0.212, 0.176],
    },
    Look {
        name: "ember",
        bone: [1.000, 0.878, 0.663],
        shade: [0.400, 0.145, 0.098],
        ground: [0.071, 0.035, 0.043],
        glow: [0.478, 0.161, 0.086],
    },
    Look {
        name: "ice",
        bone: [0.902, 0.965, 1.000],
        shade: [0.180, 0.271, 0.400],
        ground: [0.031, 0.043, 0.071],
        glow: [0.161, 0.271, 0.439],
    },
    Look {
        name: "jade",
        bone: [0.941, 1.000, 0.898],
        shade: [0.161, 0.322, 0.239],
        ground: [0.027, 0.055, 0.047],
        glow: [0.129, 0.310, 0.204],
    },
];

/// One palette in linear light, ready to hand to the renderer.
#[derive(Clone, Copy)]
struct Levels {
    bone: [f32; 3],
    shade: [f32; 3],
    ground: [f32; 3],
    glow: [f32; 3],
}

/// Every palette, converted once.
#[derive(Resource)]
struct Looks(Vec<Levels>);

/// Texture handles, loaded once.
#[derive(Resource, Clone, Copy)]
struct Art {
    white: Handle<Texture>,
    shaft: Handle<Texture>,
    knob: Handle<Texture>,
    skull: Handle<Texture>,
    ribs: Handle<Texture>,
    pelvis: Handle<Texture>,
    hand: Handle<Texture>,
    foot: Handle<Texture>,
    glow: Handle<Texture>,
}

/// Sound handles, loaded once.
#[derive(Resource, Clone, Copy)]
struct Sounds {
    kick: Handle<Sound>,
    hat: Handle<Sound>,
    clack: Handle<Sound>,
}

/// Everything that is a matter of taste rather than of fact.
#[derive(Resource)]
struct Painter {
    /// Which of [`LOOKS`] is in use.
    palette: usize,
    /// Whether the readout is shown.
    readout: bool,
    /// Whether it makes any noise.
    muted: bool,
    /// How much of the beat's flash is left, from one down to nothing.
    flash: f32,
}

impl Default for Painter {
    fn default() -> Self {
        Self {
            palette: 0,
            readout: true,
            muted: false,
            flash: 0.0,
        }
    }
}

/// What a piece of the skeleton is fastened to.
#[derive(Component, Clone, Copy)]
enum Fixing {
    /// Along a bone, from its joint to its far end.
    Shaft(usize),
    /// At a bone's joint.
    Knuckle(usize),
    /// Standing on the far end of a bone, drawn upright rather than along it.
    Cap(usize),
    /// Hung on a bone, this far along it.
    Slung(usize, f32),
    /// On the pelvis, which does not turn.
    Hips,
}

/// One drawn piece of the skeleton.
#[derive(Component)]
struct Piece {
    /// Where it is fastened.
    fixing: Fixing,
    /// How big it is, in world units. `None` for a shaft, which is as long as its bone.
    size: Option<Vec2>,
    /// How bright it is: one is the palette's bone colour, less goes towards the shade.
    tint: f32,
}

/// Marks the pool of light.
#[derive(Component)]
struct Glow;

/// Marks the flat colour behind everything.
#[derive(Component)]
struct Ground;

/// Marks the readout.
#[derive(Component)]
struct Readout;

/// Marks the panel that keeps the readout legible.
#[derive(Component)]
struct Panel;

/// One sRGB channel in linear light. The renderer works in linear light, and a palette written
/// as sRGB comes out roughly seven times too bright without this.
fn linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// A colour from a palette, blended between the shaded and the lit bone.
fn boned(levels: &Levels, tint: f32) -> Color {
    Color::rgb(
        levels.shade[0] + (levels.bone[0] - levels.shade[0]) * tint,
        levels.shade[1] + (levels.bone[1] - levels.shade[1]) * tint,
        levels.shade[2] + (levels.bone[2] - levels.shade[2]) * tint,
    )
}

/// Load everything and put the skeleton together, piece by piece.
fn setup(
    mut commands: Commands,
    mut assets: AssetLoader,
    mut sounds: SoundLoader,
    mut audio: ResMut<Audio>,
) {
    let art = Art {
        white: assets.load("white.png"),
        shaft: assets.load("shaft.png"),
        knob: assets.load("knob.png"),
        skull: assets.load("skull.png"),
        ribs: assets.load("ribs.png"),
        pelvis: assets.load("pelvis.png"),
        hand: assets.load("hand.png"),
        foot: assets.load("foot.png"),
        glow: assets.load("glow.png"),
    };
    commands.insert_resource(art);
    commands.insert_resource(Sounds {
        kick: sounds.load("kick.wav"),
        hat: sounds.load("hat.wav"),
        clack: sounds.load("clack.wav"),
    });
    audio.set_master_volume(0.5);

    commands.insert_resource(Looks(
        LOOKS
            .iter()
            .map(|look| Levels {
                bone: look.bone.map(linear),
                shade: look.shade.map(linear),
                ground: look.ground.map(linear),
                glow: look.glow.map(linear),
            })
            .collect(),
    ));

    commands.spawn((
        Ground,
        Sprite::new(art.white).with_z(0.0),
        Transform2D::default(),
    ));
    commands.spawn((
        Glow,
        Sprite::new(art.glow).with_size(GLOW_SIZE).with_z(0.5),
        Transform2D::default(),
    ));

    let mut piece = |fixing: Fixing, texture: Handle<Texture>, size: Option<Vec2>, tint: f32, z| {
        commands.spawn((
            Piece { fixing, size, tint },
            Sprite::new(texture).with_z(z),
            Transform2D::default(),
        ));
    };

    // The pelvis is not a bone — it is the thing everything else hangs off — so it is drawn
    // once, on the centre line, and never turns.
    piece(Fixing::Hips, art.pelvis, Some(PELVIS_SIZE), 0.94, 3.4);

    for (index, spec) in BONES.iter().enumerate() {
        let front = depth(spec.side);
        // A skull is not drawn as a bone with a skull stuck on it; it *is* the drawn piece.
        if spec.part != Part::Skull {
            piece(Fixing::Shaft(index), art.shaft, None, 1.0, front);
        }
        piece(
            Fixing::Knuckle(index),
            art.knob,
            Some(Vec2::splat(knuckle(spec.part))),
            0.92,
            front + 0.2,
        );
        match spec.part {
            // Hung on the middle of its bone rather than standing on the end of it: the
            // skull *is* the last bone of the spine, not something balanced on top of one.
            Part::Skull => piece(
                Fixing::Slung(index, 0.66),
                art.skull,
                Some(Vec2::splat(spec.length * SKULL_SIZE)),
                1.0,
                front + 0.4,
            ),
            Part::Chest => piece(
                Fixing::Slung(index, 0.5),
                art.ribs,
                Some(Vec2::splat(spec.length * RIBS_SIZE)),
                0.86,
                front + 0.4,
            ),
            Part::Hand => piece(
                Fixing::Cap(index),
                art.hand,
                Some(HAND_SIZE),
                0.96,
                front + 0.4,
            ),
            Part::Foot => piece(
                Fixing::Cap(index),
                art.foot,
                Some(FOOT_SIZE),
                0.96,
                front + 0.4,
            ),
            _ => {}
        }
    }

    commands.spawn((
        Panel,
        Sprite::new(art.white)
            .with_size(PANEL)
            .with_color(Color::rgba(0.0, 0.0, 0.0, 0.55))
            .with_z(19.0),
        Transform2D::default(),
    ));
    commands.spawn((
        Readout,
        Text::new("").with_size(READOUT_SIZE).with_z(20.0),
        Transform2D::default(),
    ));
}

/// Fit the stage to whatever shape the window has ended up, and put the readout in its corner.
///
/// The simulation is never told about any of this. A skeleton is a skeleton whatever size the
/// window is.
fn fit_window(
    window: Res<WindowInfo>,
    mut camera: ResMut<Camera2D>,
    mut grounds: Query<&mut Sprite, With<Ground>>,
    mut readouts: Query<&mut Transform2D, With<Readout>>,
    mut panels: Query<&mut Transform2D, (With<Panel>, Without<Readout>)>,
) {
    let size = vec2(window.width as f32, window.height as f32);
    if size.x < 1.0 || size.y < 1.0 {
        return; // minimized
    }
    camera.zoom = (size.y / STAGE).min(size.x / STAGE_ACROSS);
    let visible = size / camera.zoom;

    for mut ground in &mut grounds {
        ground.custom_size = Some(visible);
    }
    let corner = vec2(
        -visible.x / 2.0 + READOUT_MARGIN,
        visible.y / 2.0 - READOUT_MARGIN,
    );
    for mut readout in &mut readouts {
        readout.translation = corner;
    }
    for mut panel in &mut panels {
        panel.translation = corner + vec2(PANEL.x / 2.0 - READOUT_MARGIN, -PANEL.y / 2.0 + 8.0);
    }
}

/// Put every piece of the skeleton where the simulation says its bone has got to.
///
/// A sprite's own x runs along the bone for the shafts, and straight up for the pieces that
/// have a right way up — a skull, a hand, a ribcage. A bone at angle `θ` points along
/// `(sin θ, −cos θ)`, so the two of them want `θ − π/2` and `θ − π`.
fn place_pieces(
    skeleton: Res<Skeleton>,
    painter: Res<Painter>,
    looks: Option<Res<Looks>>,
    mut pieces: Query<(&Piece, &mut Sprite, &mut Transform2D)>,
) {
    let Some(looks) = looks else { return };
    let levels = looks.0[painter.palette % looks.0.len()];

    for (piece, mut sprite, mut transform) in &mut pieces {
        match piece.fixing {
            Fixing::Shaft(index) => {
                let place = skeleton.places[index];
                let spec = &BONES[index];
                transform.translation = (place.pivot + place.tip) * 0.5;
                transform.rotation = place.angle - std::f32::consts::FRAC_PI_2;
                sprite.custom_size = Some(vec2(spec.length, thickness(spec.part)));
            }
            Fixing::Knuckle(index) => {
                transform.translation = skeleton.places[index].pivot;
                transform.rotation = 0.0;
                sprite.custom_size = piece.size;
            }
            Fixing::Cap(index) => {
                let place = skeleton.places[index];
                let spec = &BONES[index];
                // Standing on the far end of its bone and half its own height beyond it, so a
                // skull sits on a neck rather than through it.
                let size = piece.size.unwrap_or(Vec2::ZERO);
                transform.translation = place.tip + direction(place.angle) * (size.y * 0.28);
                transform.rotation = place.angle - std::f32::consts::PI;
                sprite.custom_size = piece.size;
                sprite.flip_x = spec.side == Side::Left;
            }
            Fixing::Slung(index, along) => {
                let place = skeleton.places[index];
                transform.translation =
                    place.pivot + direction(place.angle) * (BONES[index].length * along);
                transform.rotation = place.angle - std::f32::consts::PI;
                sprite.custom_size = piece.size;
            }
            Fixing::Hips => {
                transform.translation = skeleton.hips;
                transform.rotation = 0.0;
                sprite.custom_size = piece.size;
            }
        }
        sprite.color = boned(&levels, piece.tint);
    }
}

/// Colour the ground and the pool of light, and give the light a lift on every beat.
fn paint_stage(
    painter: Res<Painter>,
    looks: Option<Res<Looks>>,
    skeleton: Res<Skeleton>,
    mut glows: Query<(&mut Sprite, &mut Transform2D), With<Glow>>,
    mut grounds: Query<&mut Sprite, (With<Ground>, Without<Glow>)>,
) {
    let Some(looks) = looks else { return };
    let levels = looks.0[painter.palette % looks.0.len()];
    let lift = 1.0 + 0.55 * painter.flash;
    for (mut sprite, mut transform) in &mut glows {
        // The light follows the hips, which is what makes the flash read as the dancer being
        // lit rather than the room.
        transform.translation = skeleton.hips * 0.5;
        sprite.color = Color::rgba(
            levels.glow[0] * lift,
            levels.glow[1] * lift,
            levels.glow[2] * lift,
            1.0,
        );
    }
    for mut ground in &mut grounds {
        ground.color = Color::rgb(levels.ground[0], levels.ground[1], levels.ground[2]);
    }
}

/// `P` changes palette, `M` mutes, `H` puts the readout away; and the flash fades.
///
/// Debounced against the previous frame rather than using `just_pressed`, since a frame system
/// can see the same tick's edge twice.
fn painter_controls(
    input: Res<Input>,
    time: Res<Time>,
    beat: Res<Beat>,
    mut painter: ResMut<Painter>,
    mut held: Local<[bool; 3]>,
    mut counted: Local<u64>,
) {
    let down = [
        input.pressed(Key::P),
        input.pressed(Key::M),
        input.pressed(Key::H),
    ];
    if down[0] && !held[0] {
        painter.palette = (painter.palette + 1) % LOOKS.len();
    }
    if down[1] && !held[1] {
        painter.muted = !painter.muted;
    }
    if down[2] && !held[2] {
        painter.readout = !painter.readout;
    }
    *held = down;

    if beat.count != *counted {
        *counted = beat.count;
        painter.flash = 1.0;
    }
    painter.flash = (painter.flash - time.frame_delta * 4.0).max(0.0);
}

/// What the skeleton is being asked to do, and what it is making of it.
#[expect(clippy::too_many_arguments, reason = "a readout reads everything")]
fn readout(
    beat: Res<Beat>,
    tone: Res<Tone>,
    routine: Res<Routine>,
    skeleton: Res<Skeleton>,
    painter: Res<Painter>,
    mut texts: Query<&mut Text, With<Readout>>,
    mut panels: Query<&mut Sprite, With<Panel>>,
    mut rattle: Local<f32>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    for mut panel in &mut panels {
        panel.color.a = if painter.readout { 0.55 } else { 0.0 };
    }
    if !painter.readout {
        text.value = String::new();
        return;
    }

    // A slow average, so the number is readable rather than a blur.
    *rattle = *rattle * 0.94 + skeleton.knocks.len() as f32 * 0.06 * 60.0;

    let step = routine.step();
    let arm = BONES
        .iter()
        .find(|spec| spec.part == Part::UpperArm)
        .map(|spec| spec.length)
        .unwrap_or(64.0);
    let kapitza = step.kapitza(arm, beat.rate());

    let title = format!("JIG  {}", step.name);
    let tempo = if beat.tempo <= 0.0 {
        "the band has stopped".to_string()
    } else {
        format!("{:.0} beats a minute", beat.tempo)
    };
    let width = LINE_WIDTH.saturating_sub(title.chars().count());
    text.value = format!(
        "{title}{tempo:>width$}\n\
         {}\n\n\
         tone {:.2} of {TONE_MAX:.0}   {}        rattle {:>3.0}/s   \
         Kapitza {kapitza:.2}   {}\n\
         up/down tempo   left/right tone   1-5 step   r stand up   \
         p palette   m mute   h hide",
        step.blurb,
        tone.0,
        if tone.0 < 1.0 {
            "under 1: nothing can hold itself up"
        } else if kapitza > 1.0 {
            "over 1: upside down is stable too"
        } else {
            "over 1: it can stand"
        },
        rattle.min(999.0),
        LOOKS[painter.palette % LOOKS.len()].name,
    );
}

/// The band, and the bones knocking together.
///
/// The knocks are rationed. A joint leaning on its stop through a whole bar produces a genuine
/// arrival every time it lifts off and comes back, which can be a hundred a second, and a
/// hundred woodblocks a second is not a rattle but a chainsaw.
#[expect(clippy::too_many_arguments, reason = "standard ECS system shape")]
fn play_sounds(
    mut audio: ResMut<Audio>,
    sounds: Option<Res<Sounds>>,
    assets: Res<Assets<Sound>>,
    beat: Res<Beat>,
    skeleton: Res<Skeleton>,
    painter: Res<Painter>,
    time: Res<Time>,
    mut counted: Local<u64>,
    mut halfway: Local<bool>,
    mut allowance: Local<f32>,
    mut since: Local<f32>,
) {
    let Some(sounds) = sounds else { return };
    if painter.muted {
        *counted = beat.count;
        return;
    }

    if beat.count != *counted {
        *counted = beat.count;
        *halfway = false;
        audio.play_with(
            &assets,
            sounds.kick,
            PlayParams {
                volume: 0.75,
                pitch: 1.0,
                pan: 0.0,
            },
        );
    }
    // The offbeat, once per beat, when the phase passes half way round.
    if !*halfway && beat.phase > std::f32::consts::PI && beat.tempo > 0.0 {
        *halfway = true;
        audio.play_with(
            &assets,
            sounds.hat,
            PlayParams {
                volume: 0.22,
                pitch: 1.0,
                pan: 0.0,
            },
        );
    }

    *allowance = (*allowance + time.frame_delta * KNOCK_BUDGET).min(3.0);
    *since += time.frame_delta;
    for knock in &skeleton.knocks {
        if *allowance < 1.0 || *since < KNOCK_GAP {
            break;
        }
        *allowance -= 1.0;
        *since = 0.0;
        let spec = &BONES[knock.bone];
        // Pitched by how big the bone is, so a thigh thuds where a finger tacks, and panned by
        // which side of the body it happened on.
        let heft = (spec.length * spec.weight / 60.0).clamp(0.3, 2.2);
        audio.play_with(
            &assets,
            sounds.clack,
            PlayParams {
                volume: (0.10 + 0.05 * knock.speed).min(0.42),
                pitch: 1.5 / heft.sqrt(),
                pan: match spec.side {
                    Side::Left => -0.35,
                    Side::Middle => 0.0,
                    Side::Right => 0.35,
                },
            },
        );
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Jig".into(),
        window_size: (1180, 880),
        clear_color: Color::rgb(0.01, 0.01, 0.01),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Painter::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(painter_controls)
    .add_frame_system(place_pieces)
    .add_frame_system(paint_stage)
    .add_frame_system(readout)
    .add_frame_system(play_sounds)
    .run();
}
