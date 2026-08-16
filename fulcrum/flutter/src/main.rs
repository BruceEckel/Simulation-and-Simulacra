//! Flutter, windowed: the moths, the lamp they circle, and the readout. The room itself lives
//! in `game.rs` and stays renderer-free; everything here is drawing.
//!
//! `cargo run -p flutter --release`
//!
//! - `up` / `down` — more moths, fewer moths (hold: the swarm scales, so it goes a long way)
//! - `left` / `right` — slower, faster; `0` back to normal; `space` holds everything still
//! - move the pointer — the lamp goes with it; `l` puts it out
//! - `r` — a fresh swarm
//!
//! A moth is one sprite from an eight-frame sheet. Which frame is not stored anywhere: it is
//! read off the moth's wingbeat phase every frame ([`game::wing_frame`]), which is simulation
//! state, so the wingbeat slows down and speeds up with everything else instead of ticking
//! along at its own rate while the swarm crawls.

use flutter::game::{
    self, ARENA, Clock, Flock, GamePlugin, Lamp, MAX_MOTHS, Moth, Paused, Speed, WINGSPAN,
};
use fulcrum::prelude::*;
use simulacra_assets::assets;

/// Coats of paint for the moths, in sRGB: bone, pale gold, dusty rose, ash, verdigris, lilac.
/// All of them dusty and none of them bright — a moth is a night-time creature, and the lamp
/// is the only thing on screen allowed to be a colour.
const COAT_COUNT: usize = 6;
/// See [`COAT_COUNT`].
const COATS: [[f32; 3]; COAT_COUNT] = [
    [0.949, 0.918, 0.831],
    [0.898, 0.788, 0.510],
    [0.839, 0.635, 0.612],
    [0.706, 0.725, 0.776],
    [0.573, 0.749, 0.714],
    [0.714, 0.663, 0.847],
];

/// The lamp's colour, in sRGB. Sodium-lamp amber.
const LAMPLIGHT: [f32; 3] = [1.0, 0.816, 0.502];
/// How wide the lamp's glow is drawn, in world units.
const LAMP_SIZE: f32 = 460.0;
/// How brightly the glow is drawn when lit, and how fast it fades when switched.
const LAMP_ALPHA: f32 = 0.5;
/// Per-second rate the glow fades in or out.
const LAMP_FADE: f32 = 5.0;

/// The night the whole thing happens in, in sRGB.
const NIGHT: [f32; 3] = [0.043, 0.047, 0.098];

/// The wingbeat sheet: one row of `WING_FRAMES` tiles. Kept as a resource because every moth
/// that arrives mid-flight needs it, and moths arrive by the thousand. (The lamp's glow is
/// loaded once and lives on the one entity that draws it, so it needs no home here.)
#[derive(Resource)]
struct Art {
    moth: Handle<SpriteSheet>,
}

/// The moths' colours, in linear light.
#[derive(Resource)]
struct Coats([[f32; 3]; COAT_COUNT]);

/// The glow that stands in for the lamp.
#[derive(Component)]
struct Lantern;

/// The counts, top left.
#[derive(Component)]
struct Readout;

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

/// What a moth's `tone` means: which coat, and how much of the night is still on it. The
/// fractional part shades the coat, so six colours give a swarm with no two moths alike.
fn coat_of(coats: &Coats, tone: f32) -> Color {
    let place = tone.clamp(0.0, 0.999) * coats.0.len() as f32;
    let base = coats.0[place as usize];
    let shade = 0.62 + 0.38 * place.fract();
    Color::rgba(base[0] * shade, base[1] * shade, base[2] * shade, 0.94)
}

/// Load the art, aim the camera, and put up the lamp and the readout.
fn setup(mut commands: Commands, mut assets: AssetLoader, mut camera: ResMut<Camera2D>) {
    camera.scaling = ScalingMode::Letterbox {
        width: ARENA.x,
        height: ARENA.y,
    };

    let sheet = assets.load("moth.png");
    let moth = assets.add_sheet(SpriteSheet::from_grid(
        sheet,
        Vec2::splat(32.0),
        game::WING_FRAMES,
        1,
    ));
    let glow = assets.load("glow.png");
    commands.insert_resource(Art { moth });
    commands.insert_resource(Coats(
        COATS.map(|coat| [linear(coat[0]), linear(coat[1]), linear(coat[2])]),
    ));

    commands.spawn((
        Lantern,
        Sprite::new(glow)
            .with_size(Vec2::splat(LAMP_SIZE))
            .with_color(paint(LAMPLIGHT, LAMP_ALPHA))
            .with_z(-1.0),
        Transform2D::IDENTITY,
    ));
    commands.spawn((
        Readout,
        Text::new("")
            .with_size(16.0)
            .with_color(paint([0.878, 0.898, 0.949], 0.9))
            .with_z(10.0),
        Transform2D::from_xy(-ARENA.x / 2.0 + 20.0, ARENA.y / 2.0 - 30.0),
    ));
    commands.spawn((
        Text::new(
            "up / down  more or fewer moths     left / right  slower or faster     \
             0  normal speed     space  hold still     l  lamp     r  new swarm",
        )
        .with_size(8.0)
        .with_align(HAlign::Center)
        .with_color(paint([0.647, 0.678, 0.769], 0.65))
        .with_z(10.0),
        Transform2D::from_xy(0.0, -ARENA.y / 2.0 + 22.0),
    ));
}

/// Give every moth that does not have one a sprite: its coat, its wingspan, and a place in the
/// stack. Moths arrive by the thousand while the simulation runs, so this is where they get
/// dressed rather than at startup.
fn dress_moths(
    mut commands: Commands,
    art: Res<Art>,
    coats: Res<Coats>,
    bare: Query<(Entity, &Moth), Without<Sprite>>,
) {
    for (entity, moth) in &bare {
        // Big moths in front of small ones: the swarm gets a front and a back, which is most of
        // what makes a flat pile of sprites read as depth.
        let depth = (moth.wingspan - WINGSPAN.0) / (WINGSPAN.1 - WINGSPAN.0);
        commands.entity(entity).insert(
            Sprite::from_sheet(art.moth, 0)
                .with_size(Vec2::splat(moth.wingspan))
                .with_color(coat_of(&coats, moth.tone))
                .with_z(depth),
        );
    }
}

/// Show the frame the moth's wingbeat is on.
fn beat_wings(art: Res<Art>, mut moths: Query<(&Moth, &mut Sprite)>) {
    for (moth, mut sprite) in &mut moths {
        sprite.region = Some(SpriteRegion {
            sheet: art.moth,
            index: game::wing_frame(moth.wing),
        });
    }
}

/// Put the glow where the lamp is, and fade it in or out when it is switched.
fn place_lamp(
    lamp: Res<Lamp>,
    time: Res<Time>,
    mut lit: Local<f32>,
    mut lanterns: Query<(&mut Transform2D, &mut Sprite), With<Lantern>>,
) {
    let want = if lamp.on { 1.0 } else { 0.0 };
    *lit += (want - *lit) * (time.frame_delta * LAMP_FADE).min(1.0);
    for (mut transform, mut sprite) in &mut lanterns {
        transform.translation = lamp.at;
        sprite.color = paint(LAMPLIGHT, LAMP_ALPHA * *lit);
    }
}

/// The two dials, and what they are costing.
fn update_readout(
    flock: Res<Flock>,
    speed: Res<Speed>,
    paused: Res<Paused>,
    clock: Res<Clock>,
    time: Res<Time>,
    mut fps: Local<f32>,
    mut readouts: Query<&mut Text, With<Readout>>,
) {
    if time.frame_delta > 0.0 {
        // Smoothed, or the number is unreadable.
        *fps += (1.0 / time.frame_delta - *fps) * 0.08;
    }
    let pace = if paused.0 {
        "still".to_string()
    } else {
        format!("{:.2}x", speed.0)
    };
    let Ok(mut text) = readouts.single_mut() else {
        return;
    };
    let value = format!(
        "moths {} / {}\nspeed {}\nclock {:.0}s\n{:.0} fps",
        flock.count, MAX_MOTHS, pace, clock.0, *fps
    );
    if text.value != value {
        text.value = value;
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Flutter".into(),
        window_size: (ARENA.x as u32, ARENA.y as u32),
        clear_color: paint(NIGHT, 1.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .add_startup(setup)
    .add_frame_system(dress_moths)
    .add_frame_system(beat_wings)
    .add_frame_system(place_lamp)
    .add_frame_system(update_readout)
    .run();
}
