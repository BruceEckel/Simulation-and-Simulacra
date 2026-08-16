//! Popped, windowed: the balloons, the animals, and the noise they make. The sky itself lives
//! in `game.rs` and stays renderer-free; everything here is drawing and sound.
//!
//! `cargo run -p popped --release`
//!
//! - move the pointer near a balloon and the passengers will wave at you
//! - click a balloon to pop it
//! - `m` mutes it, `space` stills it, `up`/`down` change the pace, `0` restores it
//!
//! Every animal is built out of parts each frame rather than drawn: a body, a head, ears, a
//! muzzle, four limbs and a face. That is what lets one set of shapes give five species in eight
//! colours pulling four expressions, and it is why the same rabbit can wave, realise, scream,
//! bounce and then walk off in a huff without a single frame of animation being authored.

use fulcrum::prelude::*;
use popped::game::{
    self, ARENA, Animal, Balloon, Basket, COATS, Census, GROUND, GamePlugin, Mood, Noise, Paused,
    Puff, Scrap, Species, Speed, Tally, Voice,
};
use simulacra_assets::assets;
use std::f32::consts::{PI, TAU};

/// How tall an animal is at size 1, in world units, from its feet to the top of its head.
///
/// Everything about the joke is in the faces, so they are drawn a size larger than the scene
/// really wants: a passenger who is a smudge in a basket is not somebody you can feel sorry for.
const STATURE: f32 = 53.0;
/// Seconds a puff of dust lasts.
const DUST_LIFE: f32 = 0.55;
/// How many clouds are up there.
const CLOUDS: usize = 11;
/// How many bands the sky is built from.
const SKY_BANDS: usize = 32;
/// Seconds the hint stays up before it fades and leaves you to it.
const HINT_LIFE: f32 = 20.0;

/// The sky, top and bottom, in sRGB. Converted to linear once at startup: the renderer works in
/// linear light, where a mid blue is a much smaller number than it looks.
const SKY_HIGH: [f32; 3] = [0.286, 0.588, 0.878];
/// See [`SKY_HIGH`].
const SKY_LOW: [f32; 3] = [0.780, 0.890, 0.960];

/// Eight coats of paint for the balloons and the animals, in sRGB. Pastel, saturated, and all
/// of them cheerful: the joke only works if the setup is sweet.
const PAINTS: [[f32; 3]; COATS as usize] = [
    [0.972, 0.478, 0.510],
    [0.988, 0.706, 0.361],
    [0.976, 0.878, 0.427],
    [0.596, 0.847, 0.541],
    [0.427, 0.827, 0.812],
    [0.494, 0.647, 0.929],
    [0.769, 0.596, 0.918],
    [0.945, 0.616, 0.435],
];

/// The grass, and the darker strip at the top of it.
const GRASS: [f32; 3] = [0.435, 0.706, 0.365];
/// See [`GRASS`].
const GRASS_EDGE: [f32; 3] = [0.322, 0.588, 0.290];
/// Basket wicker.
const WICKER: [f32; 3] = [0.769, 0.573, 0.322];

/// Texture handles, loaded once.
#[derive(Resource, Clone)]
struct Art {
    balloon: Handle<Texture>,
    scrap: Handle<Texture>,
    basket: Handle<Texture>,
    body: Handle<Texture>,
    head: Handle<Texture>,
    muzzle: Handle<Texture>,
    ear_round: Handle<Texture>,
    ear_tall: Handle<Texture>,
    ear_point: Handle<Texture>,
    limb: Handle<Texture>,
    eye_happy: Handle<Texture>,
    eye_open: Handle<Texture>,
    eye_shock: Handle<Texture>,
    eye_dizzy: Handle<Texture>,
    mouth_smile: Handle<Texture>,
    mouth_scream: Handle<Texture>,
    mark: Handle<Texture>,
    star: Handle<Texture>,
    chute: Handle<Texture>,
    cloud: Handle<Texture>,
    puff: Handle<Texture>,
    blob: Handle<Texture>,
    white: Handle<Texture>,
}

/// Sound handles, loaded once.
#[derive(Resource)]
struct Sounds {
    pop: Handle<Sound>,
    scream: Handle<Sound>,
    bonk: Handle<Sound>,
    boing: Handle<Sound>,
    raspberry: Handle<Sound>,
    chime: Handle<Sound>,
}

/// The paints, in linear light.
#[derive(Resource)]
struct Coats([[f32; 3]; COATS as usize]);

/// Whether the noise is turned off.
#[derive(Resource, Default)]
struct Muted(bool);

/// Marks an animal that has already been given its parts.
#[derive(Component)]
struct Dressed;

/// Which piece of an animal this entity draws.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Shadow,
    LegLeft,
    LegRight,
    ArmLeft,
    ArmRight,
    Body,
    EarLeft,
    EarRight,
    Head,
    Muzzle,
    EyeLeft,
    EyeRight,
    Mouth,
    Mark,
    Star(u8),
    Chute,
}

/// Every part an animal is made of, in the order they are drawn.
const PARTS: [Role; 17] = [
    Role::Shadow,
    Role::LegLeft,
    Role::LegRight,
    Role::ArmLeft,
    Role::ArmRight,
    Role::EarLeft,
    Role::EarRight,
    Role::Body,
    Role::Head,
    Role::Muzzle,
    Role::EyeLeft,
    Role::EyeRight,
    Role::Mouth,
    Role::Mark,
    Role::Star(0),
    Role::Star(1),
    Role::Star(2),
];

/// Where a part goes and what it looks like this frame.
struct Piece {
    /// Where it sits, measured from the animal's feet before it is leaned or scaled.
    at: Vec2,
    /// Which way up.
    angle: f32,
    /// How big, in world units at size 1.
    size: Vec2,
    /// Which shape.
    texture: Handle<Texture>,
    /// What colour.
    colour: Color,
    /// Where in the stack of parts it is drawn.
    layer: f32,
}

/// One piece of one animal.
#[derive(Component)]
struct Part {
    owner: Entity,
    role: Role,
}

/// One of the two ropes holding a basket up.
#[derive(Component)]
struct Rope {
    basket: Entity,
    side: f32,
}

/// A puff of dust on the ground.
#[derive(Component)]
struct Dust {
    age: f32,
    size: f32,
}

/// A cloud, drifting.
#[derive(Component)]
struct Cloud {
    speed: f32,
}

/// Marks the line about you.
#[derive(Component)]
struct Board;

/// Marks the running totals.
#[derive(Component)]
struct Ledger;

/// Marks the hint line.
#[derive(Component)]
struct Hint;

/// A small deterministic generator, for scattering scenery. Kept away from `SimRng`: the clouds
/// are the view's business, and drawing them from the simulation's stream would let the scenery
/// change what happens in the sky.
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

/// A colour written in sRGB, ready for the renderer.
fn paint(srgb: [f32; 3], alpha: f32) -> Color {
    Color::rgba(linear(srgb[0]), linear(srgb[1]), linear(srgb[2]), alpha)
}

/// A coat of paint, lightened or darkened.
fn coated(coats: &Coats, coat: u8, shade: f32, alpha: f32) -> Color {
    let base = coats.0[(coat as usize).min(COATS as usize - 1)];
    Color::rgba(base[0] * shade, base[1] * shade, base[2] * shade, alpha)
}

/// Load everything, and put up the sky, the ground and the clouds.
fn setup(
    mut commands: Commands,
    mut assets: AssetLoader,
    mut sounds: SoundLoader,
    mut audio: ResMut<Audio>,
    mut camera: ResMut<Camera2D>,
) {
    // The sky is a simulation constant, so the window is only ever a view of it.
    camera.scaling = ScalingMode::Letterbox {
        width: ARENA.x,
        height: ARENA.y,
    };

    let art = Art {
        balloon: assets.load("balloon.png"),
        scrap: assets.load("scrap.png"),
        basket: assets.load("basket.png"),
        body: assets.load("body.png"),
        head: assets.load("head.png"),
        muzzle: assets.load("muzzle.png"),
        ear_round: assets.load("ear_round.png"),
        ear_tall: assets.load("ear_tall.png"),
        ear_point: assets.load("ear_point.png"),
        limb: assets.load("limb.png"),
        eye_happy: assets.load("eye_happy.png"),
        eye_open: assets.load("eye_open.png"),
        eye_shock: assets.load("eye_shock.png"),
        eye_dizzy: assets.load("eye_dizzy.png"),
        mouth_smile: assets.load("mouth_smile.png"),
        mouth_scream: assets.load("mouth_scream.png"),
        mark: assets.load("mark.png"),
        star: assets.load("star.png"),
        chute: assets.load("chute.png"),
        cloud: assets.load("cloud.png"),
        puff: assets.load("puff.png"),
        blob: assets.load("blob.png"),
        white: assets.load("white.png"),
    };
    commands.insert_resource(Sounds {
        pop: sounds.load("pop.wav"),
        scream: sounds.load("scream.wav"),
        bonk: sounds.load("bonk.wav"),
        boing: sounds.load("boing.wav"),
        raspberry: sounds.load("raspberry.wav"),
        chime: sounds.load("chime.wav"),
    });
    audio.set_master_volume(0.7);
    commands.insert_resource(Coats(
        PAINTS.map(|colour| [linear(colour[0]), linear(colour[1]), linear(colour[2])]),
    ));

    build_scene(&mut commands, &art);
    commands.spawn((
        Board,
        Text::new("")
            .with_size(13.0)
            .with_align(HAlign::Center)
            .with_z(100.0),
        Transform2D::from_translation(vec2(0.0, ARENA.y * 0.5 - 32.0)),
    ));
    commands.spawn((
        Ledger,
        Text::new("")
            .with_size(8.0)
            .with_align(HAlign::Center)
            .with_z(100.0),
        Transform2D::from_translation(vec2(0.0, ARENA.y * 0.5 - 60.0)),
    ));
    commands.spawn((
        Hint,
        Text::new(HINT_LINES)
            .with_size(8.0)
            .with_align(HAlign::Center)
            .with_z(100.0),
        Transform2D::from_translation(vec2(0.0, -ARENA.y * 0.5 + 26.0)),
    ));
    commands.insert_resource(art);
}

/// The sky, the sun, the clouds and the ground.
fn build_scene(commands: &mut Commands, art: &Art) {
    let mut dice = Dice(0x0B00_B1E5);

    // A gradient in bands. Daylight sits high enough up the curve that the steps between them
    // do not show, which is not true of the night skies in the other pieces.
    let top = ARENA.y * 0.5;
    let band = (top - GROUND) / SKY_BANDS as f32;
    for index in 0..SKY_BANDS {
        let up = (index as f32 + 0.5) / SKY_BANDS as f32;
        let colour = [
            SKY_LOW[0] + (SKY_HIGH[0] - SKY_LOW[0]) * up,
            SKY_LOW[1] + (SKY_HIGH[1] - SKY_LOW[1]) * up,
            SKY_LOW[2] + (SKY_HIGH[2] - SKY_LOW[2]) * up,
        ];
        let mut sprite = Sprite::new(art.white).with_z(-100.0);
        sprite.custom_size = Some(vec2(ARENA.x + 4.0, band * 1.08));
        sprite.color = paint(colour, 1.0);
        commands.spawn((
            sprite,
            Transform2D::from_translation(vec2(0.0, GROUND + band * (index as f32 + 0.5))),
        ));
    }

    // The sun, as a soft blob rather than a disc: nobody is meant to look at it.
    let mut sun = Sprite::new(art.blob).with_z(-98.0);
    sun.custom_size = Some(Vec2::splat(420.0));
    sun.color = paint([1.0, 0.97, 0.80], 0.55);
    commands.spawn((
        sun,
        Transform2D::from_translation(vec2(-ARENA.x * 0.34, ARENA.y * 0.36)),
    ));

    for _ in 0..CLOUDS {
        let scale = dice.range(0.6, 1.6);
        let mut sprite = Sprite::new(art.cloud).with_z(-90.0);
        sprite.custom_size = Some(vec2(230.0, 115.0) * scale);
        sprite.color = paint([1.0, 1.0, 1.0], dice.range(0.5, 0.9));
        commands.spawn((
            Cloud {
                // Nearer clouds are bigger and go faster, which is the whole of the parallax.
                speed: dice.range(3.0, 9.0) * scale,
            },
            sprite,
            Transform2D::from_translation(vec2(
                dice.range(-0.5, 0.5) * ARENA.x,
                dice.range(-0.05, 0.45) * ARENA.y,
            )),
        ));
    }

    // The ground: a field, with a darker lip along the top of it.
    let depth = GROUND + ARENA.y * 0.5;
    let mut field = Sprite::new(art.white).with_z(15.0);
    field.custom_size = Some(vec2(ARENA.x + 4.0, depth));
    field.color = paint(GRASS, 1.0);
    commands.spawn((
        field,
        Transform2D::from_translation(vec2(0.0, GROUND - depth * 0.5)),
    ));
    let mut lip = Sprite::new(art.white).with_z(16.0);
    lip.custom_size = Some(vec2(ARENA.x + 4.0, 10.0));
    lip.color = paint(GRASS_EDGE, 1.0);
    commands.spawn((lip, Transform2D::from_translation(vec2(0.0, GROUND - 5.0))));

    // Bushes, so the ground is not an empty stripe.
    for _ in 0..26 {
        let scale = dice.range(0.5, 1.3);
        let mut bush = Sprite::new(art.cloud).with_z(17.0);
        bush.custom_size = Some(vec2(90.0, 44.0) * scale);
        bush.color = paint(GRASS_EDGE, 1.0);
        commands.spawn((
            bush,
            Transform2D::from_translation(vec2(
                dice.range(-0.52, 0.52) * ARENA.x,
                GROUND - dice.range(6.0, 0.42 * depth),
            )),
        ));
    }
}

/// Give the simulation's balloons, baskets and scraps their sprites, and hang the ropes.
fn dress_rigs(
    mut commands: Commands,
    art: Option<Res<Art>>,
    balloons: Query<Entity, (With<Balloon>, Without<Sprite>)>,
    baskets: Query<Entity, (With<Basket>, Without<Sprite>)>,
    scraps: Query<Entity, (With<Scrap>, Without<Sprite>)>,
) {
    let Some(art) = art else { return };
    for entity in &balloons {
        commands
            .entity(entity)
            .try_insert(Sprite::new(art.balloon).with_z(10.0));
    }
    for entity in &baskets {
        commands
            .entity(entity)
            .try_insert(Sprite::new(art.basket).with_z(12.0));
        for side in [-1.0, 1.0] {
            let mut rope = Sprite::new(art.white).with_z(9.0);
            rope.color = paint([0.35, 0.28, 0.22], 1.0);
            commands.spawn((
                Rope {
                    basket: entity,
                    side,
                },
                rope,
                Transform2D::default(),
            ));
        }
    }
    for entity in &scraps {
        commands
            .entity(entity)
            .try_insert(Sprite::new(art.scrap).with_z(35.0));
    }
}

/// Colour and size the balloons, the baskets and the scraps.
#[allow(clippy::type_complexity)] // standard ECS system shape
fn paint_rigs(
    mut balloons: Query<(&Balloon, &mut Sprite), Without<Basket>>,
    mut baskets: Query<(&Basket, &mut Sprite, &mut Transform2D), Without<Balloon>>,
    mut scraps: Query<(&Scrap, &mut Sprite), (Without<Balloon>, Without<Basket>)>,
    coats: Option<Res<Coats>>,
) {
    let Some(coats) = coats else { return };
    for (balloon, mut sprite) in &mut balloons {
        sprite.custom_size = Some(vec2(balloon.radius * 2.05, balloon.radius * 2.25));
        sprite.color = coated(&coats, balloon.coat, 1.0, 1.0);
    }
    for (basket, mut sprite, mut transform) in &mut baskets {
        sprite.custom_size = Some(vec2(basket.width * 1.5, basket.width * 1.1));
        sprite.color = paint(WICKER, 1.0);
        transform.rotation = basket.lean;
    }
    for (scrap, mut sprite) in &mut scraps {
        let left = 1.0 - (scrap.age / scrap.life).clamp(0.0, 1.0);
        sprite.custom_size =
            Some(vec2(scrap.radius * 1.5, scrap.radius * 1.1) * (0.5 + left * 0.5));
        sprite.color = coated(&coats, scrap.coat, 1.0, left.min(0.9));
    }
}

/// Stretch the ropes between each basket and whatever is holding it up.
#[allow(clippy::type_complexity)] // standard ECS system shape
fn hang_ropes(
    mut commands: Commands,
    mut ropes: Query<(Entity, &Rope, &mut Sprite, &mut Transform2D)>,
    baskets: Query<(&Basket, &Transform2D), Without<Rope>>,
    balloons: Query<(&Transform2D, &Balloon), (Without<Rope>, Without<Basket>)>,
) {
    for (entity, rope, mut sprite, mut transform) in &mut ropes {
        let Ok((basket, hanging)) = baskets.get(rope.basket) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Some(above) = basket
            .holder
            .and_then(|holder| balloons.get(holder).ok())
            // The ropes are tied to the throat of the envelope, not to the middle of it.
            .map(|(transform, balloon)| {
                transform.translation
                    + vec2(rope.side * balloon.radius * 0.22, -balloon.radius * 1.02)
            })
        else {
            // Nothing holding it up any more: the ropes go with the balloon.
            sprite.color = Color::rgba(0.0, 0.0, 0.0, 0.0);
            continue;
        };
        let (sin, cos) = basket.lean.sin_cos();
        let corner = hanging.translation
            + vec2(
                rope.side * basket.width * 0.5 * cos,
                rope.side * basket.width * 0.5 * sin,
            )
            + vec2(sin, cos) * basket.width * 0.5;
        let span = above - corner;
        sprite.custom_size = Some(vec2(span.length(), 2.5));
        transform.translation = corner + span * 0.5;
        transform.rotation = span.to_angle();
    }
}

/// Build every new animal out of parts.
fn dress_animals(
    mut commands: Commands,
    art: Option<Res<Art>>,
    animals: Query<Entity, (With<Animal>, Without<Dressed>)>,
) {
    let Some(art) = art else { return };
    for owner in &animals {
        for role in PARTS {
            commands.spawn((
                Part { owner, role },
                Sprite::new(art.blob),
                Transform2D::default(),
            ));
        }
        // The parachute is drawn over everything else, so it comes last and separately.
        commands.spawn((
            Part {
                owner,
                role: Role::Chute,
            },
            Sprite::new(art.chute).with_z(26.0),
            Transform2D::default(),
        ));
        commands.entity(owner).try_insert(Dressed);
    }
}

/// Where a part sits on an animal, which way up, how big, which texture and what colour.
///
/// The one function that makes five species out of one set of shapes. Everything is measured
/// from the animal's feet in units of [`STATURE`], so the same layout works at any size.
fn pose(animal: &Animal, role: Role, art: &Art, coats: &Coats, height: f32) -> Option<Piece> {
    let coat = coated(coats, animal.coat, 1.0, 1.0);
    let pale = {
        let base = coats.0[(animal.coat as usize).min(COATS as usize - 1)];
        Color::rgba(
            base[0] + (1.0 - base[0]) * 0.72,
            base[1] + (1.0 - base[1]) * 0.72,
            base[2] + (1.0 - base[2]) * 0.72,
            1.0,
        )
    };
    let ink = Color::rgba(0.06, 0.05, 0.08, 1.0);
    let flail = animal.flail;

    // How the arms and the legs are held, per mood.
    //
    // Angles are measured for the right arm, with zero pointing straight out to the right and
    // up being positive; the left arm is the same angle reflected, which is what `mirror` does.
    // Doing it any other way ends up with an animal waving its left arm across its own face.
    let mirror = |angle: f32| PI - angle;
    let (arm_right, arm_left, leg_swing) = match animal.mood {
        Mood::Riding => {
            // A wave, when you come near, with one arm, the way anybody waves.
            let wave = animal.wave * (0.5 + 0.5 * (flail * 6.0).sin());
            (-1.05 + 2.5 * wave, mirror(-1.05 - 0.08 * animal.wave), 0.0)
        }
        // Both arms straight up. Nothing else reads as "oh no" so quickly.
        Mood::Beat => (
            1.45 + 0.05 * (flail * 30.0).sin(),
            mirror(1.45 - 0.05 * (flail * 30.0).sin()),
            0.15 * (flail * 26.0).sin(),
        ),
        Mood::Falling => (
            0.4 + 1.5 * flail.sin(),
            mirror(0.4 + 1.5 * (flail + 1.9).sin()),
            0.9 * (flail * 1.1).sin(),
        ),
        // Holding the lines, and pleased about it.
        Mood::Chuting => (1.15, mirror(1.15), 0.12 * (flail * 2.0).sin()),
        Mood::Dazed => (-1.0, mirror(-1.0), 0.0),
        Mood::Bowing => (0.15, mirror(0.15), 0.0),
        Mood::Trudging => (
            // One fist, shaken in your general direction, the whole way off the screen.
            1.5 + 0.28 * (flail * 2.2).sin(),
            mirror(-0.55 + 0.6 * flail.sin()),
            0.55 * flail.sin(),
        ),
    };

    // The face.
    let (eye, mouth, mouth_scale) = match animal.mood {
        Mood::Riding | Mood::Bowing => (art.eye_happy, art.mouth_smile, 1.0),
        Mood::Beat => (art.eye_shock, art.mouth_scream, 0.55),
        Mood::Falling => (art.eye_shock, art.mouth_scream, 1.0),
        Mood::Chuting => (art.eye_happy, art.mouth_smile, 0.9),
        Mood::Dazed => (art.eye_dizzy, art.mouth_scream, 0.5),
        Mood::Trudging => (art.eye_open, art.mouth_smile, 0.7),
    };

    let (ear_art, ear_at, ear_size, ear_tilt) = match animal.species {
        Species::Bear => (art.ear_round, vec2(0.28, 0.94), vec2(0.30, 0.30), 0.0),
        Species::Rabbit => (art.ear_tall, vec2(0.16, 1.16), vec2(0.20, 0.60), 0.12),
        Species::Cat => (art.ear_point, vec2(0.26, 1.00), vec2(0.30, 0.34), 0.15),
        Species::Pig => (art.ear_point, vec2(0.30, 0.88), vec2(0.28, 0.28), 1.05),
        Species::Frog => (art.ear_round, vec2(0.24, 0.98), vec2(0.34, 0.34), 0.0),
    };
    // A frog's eyes are up on the bulges, which is most of what makes a frog a frog.
    let eye_at = if animal.species == Species::Frog {
        vec2(0.24, 0.98)
    } else {
        vec2(0.17, 0.78)
    };
    let eye_size = if animal.species == Species::Frog {
        0.20
    } else {
        0.19
    };

    let unit = |offset: Vec2| offset * STATURE;
    let sized = |size: Vec2| size * STATURE;

    let piece = match role {
        Role::Shadow => {
            // A shadow on the ground under anybody who is off it, growing as they arrive. The
            // only cue for how far there is left to fall.
            if animal.mood == Mood::Riding {
                return None;
            }
            let above = (height - GROUND).max(0.0);
            let near = 1.0 - (above / 520.0).clamp(0.0, 1.0);
            let width = (0.5 + 0.5 * near) * STATURE * 0.8;
            return Some(Piece {
                at: vec2(0.0, GROUND - height + 3.0),
                angle: 0.0,
                size: vec2(width, width * 0.32),
                texture: art.blob,
                colour: Color::rgba(0.0, 0.0, 0.0, 0.10 + 0.22 * near),
                layer: -0.6,
            });
        }
        Role::LegLeft | Role::LegRight => {
            let side = if role == Role::LegLeft { -1.0 } else { 1.0 };
            // Legs hang down from the hip and swing back and forth about it.
            let angle = -PI / 2.0 + leg_swing * side;
            let hip = unit(vec2(side * 0.13, 0.20));
            let reach = STATURE * 0.13;
            (
                hip + vec2(angle.cos(), angle.sin()) * reach,
                angle,
                sized(vec2(0.26, 0.15)),
                art.limb,
                coat,
                -0.2,
            )
        }
        Role::ArmLeft | Role::ArmRight => {
            let (side, angle) = if role == Role::ArmLeft {
                (-1.0, arm_left)
            } else {
                (1.0, arm_right)
            };
            // The arm turns about the shoulder, so its middle swings round with it.
            let shoulder = unit(vec2(side * 0.22, 0.50));
            let reach = STATURE * 0.16;
            (
                shoulder + vec2(angle.cos(), angle.sin()) * reach,
                angle,
                sized(vec2(0.30, 0.15)),
                art.limb,
                coat,
                -0.1,
            )
        }
        Role::Body => (
            unit(vec2(0.0, 0.36)),
            0.0,
            sized(vec2(0.58, 0.62)),
            art.body,
            coat,
            0.0,
        ),
        Role::EarLeft | Role::EarRight => {
            let side = if role == Role::EarLeft { -1.0 } else { 1.0 };
            (
                unit(vec2(side * ear_at.x, ear_at.y)),
                -side * ear_tilt,
                sized(ear_size),
                ear_art,
                coat,
                -0.3,
            )
        }
        Role::Head => (
            unit(vec2(0.0, 0.80)),
            0.0,
            sized(vec2(0.72, 0.68)),
            art.head,
            coat,
            0.1,
        ),
        Role::Muzzle => (
            unit(vec2(0.0, 0.66)),
            0.0,
            sized(vec2(0.40, 0.26)),
            art.muzzle,
            pale,
            0.2,
        ),
        Role::EyeLeft | Role::EyeRight => {
            let side = if role == Role::EyeLeft { -1.0 } else { 1.0 };
            (
                unit(vec2(side * eye_at.x, eye_at.y)),
                0.0,
                sized(Vec2::splat(eye_size)),
                eye,
                Color::WHITE,
                0.3,
            )
        }
        Role::Mouth => (
            unit(vec2(0.0, 0.63)),
            0.0,
            sized(vec2(0.26, 0.22)) * mouth_scale,
            mouth,
            if animal.mood == Mood::Falling || animal.mood == Mood::Beat {
                Color::rgba(0.45, 0.16, 0.22, 1.0)
            } else {
                ink
            },
            0.35,
        ),
        Role::Mark => {
            // The exclamation mark, up for exactly as long as the beat lasts.
            if animal.mood != Mood::Beat {
                return None;
            }
            let jitter = (animal.timer * 60.0).sin() * 0.06;
            (
                unit(vec2(0.34, 1.42)),
                jitter,
                sized(vec2(0.16, 0.34)),
                art.mark,
                Color::rgba(1.0, 0.95, 0.25, 1.0),
                0.5,
            )
        }
        Role::Star(index) => {
            if animal.mood != Mood::Dazed {
                return None;
            }
            let angle = flail * 2.0 + TAU * index as f32 / 3.0;
            (
                unit(vec2(0.0, 1.05)) + vec2(angle.cos() * 22.0, angle.sin() * 7.0),
                angle,
                sized(Vec2::splat(0.22)),
                art.star,
                Color::rgba(1.0, 0.92, 0.30, 0.95),
                0.6,
            )
        }
        Role::Chute => {
            if animal.mood != Mood::Chuting {
                return None;
            }
            (
                unit(vec2(0.0, 2.10)),
                (flail * 1.5).sin() * 0.05,
                sized(vec2(2.6, 1.8)),
                art.chute,
                coated(coats, (animal.coat + 3) % COATS, 1.0, 1.0),
                0.7,
            )
        }
    };
    Some(Piece {
        at: piece.0,
        angle: piece.1,
        size: piece.2,
        texture: piece.3,
        colour: piece.4,
        layer: piece.5,
    })
}

/// Lay every animal out, part by part.
fn pose_animals(
    mut commands: Commands,
    mut parts: Query<(Entity, &Part, &mut Sprite, &mut Transform2D)>,
    animals: Query<(&Animal, &Transform2D), Without<Part>>,
    art: Option<Res<Art>>,
    coats: Option<Res<Coats>>,
) {
    let (Some(art), Some(coats)) = (art, coats) else {
        return;
    };
    for (entity, part, mut sprite, mut transform) in &mut parts {
        let Ok((animal, standing)) = animals.get(part.owner) else {
            // The animal has walked off; its parts go with it.
            commands.entity(entity).despawn();
            continue;
        };
        let Some(piece) = pose(animal, part.role, &art, &coats, standing.translation.y) else {
            sprite.color = Color::rgba(0.0, 0.0, 0.0, 0.0);
            continue;
        };

        // How the whole animal is held: sitting down when dazed, bent over when bowing, and
        // wobbling when falling.
        let (lean, squat) = match animal.mood {
            Mood::Falling => ((animal.flail * 0.35).sin() * 0.35, 0.0),
            Mood::Dazed => (0.18, -0.16),
            Mood::Bowing => (
                0.42 * (1.0 - (animal.timer / game::BOW).clamp(0.0, 1.0)),
                0.0,
            ),
            Mood::Trudging => ((animal.flail * 1.0).sin() * 0.06, 0.0),
            _ => (0.0, 0.0),
        };
        let (sin, cos) = lean.sin_cos();
        let scaled = (piece.at + vec2(0.0, squat * STATURE)) * animal.size;
        // The shadow belongs on the ground, not on the animal, so it does not lean with it.
        let placed = if part.role == Role::Shadow {
            scaled
        } else {
            vec2(
                scaled.x * cos - scaled.y * sin,
                scaled.x * sin + scaled.y * cos,
            )
        };
        let facing = if animal.mood == Mood::Trudging {
            animal.facing
        } else {
            1.0
        };

        transform.translation = standing.translation + vec2(placed.x * facing, placed.y);
        transform.rotation = if part.role == Role::Shadow {
            0.0
        } else {
            piece.angle * facing + lean
        };
        sprite.texture = piece.texture;
        sprite.custom_size = Some(piece.size * animal.size);
        sprite.color = piece.colour;
        // Riding animals sit behind the basket, so the wicker hides their legs and they read as
        // being in it rather than on it. Everybody else is in front of everything.
        sprite.z = if animal.mood == Mood::Riding {
            11.0 + piece.layer * 0.1
        } else {
            20.0 + piece.layer
        };
        sprite.flip_x = facing < 0.0;
    }
}

/// Dust, thrown up by an arrival.
fn dust(
    mut commands: Commands,
    mut puffs: EventReader<Puff>,
    mut clouds: Query<(Entity, &mut Dust, &mut Sprite, &mut Transform2D)>,
    art: Option<Res<Art>>,
    paused: Res<Paused>,
    time: Res<Time>,
) {
    let Some(art) = art else { return };
    for puff in puffs.read() {
        for side in [-1.0, 1.0] {
            let mut sprite = Sprite::new(art.puff).with_z(30.0);
            sprite.flip_x = side < 0.0;
            commands.spawn((
                Dust {
                    age: 0.0,
                    size: puff.size,
                },
                sprite,
                Transform2D::from_translation(puff.at + vec2(side * 12.0 * puff.size, 6.0)),
            ));
        }
    }
    let step = if paused.0 { 0.0 } else { time.frame_delta };
    for (entity, mut cloud, mut sprite, mut transform) in &mut clouds {
        cloud.age += step;
        if cloud.age >= DUST_LIFE {
            commands.entity(entity).despawn();
            continue;
        }
        let along = cloud.age / DUST_LIFE;
        let width = (34.0 + 60.0 * along) * cloud.size;
        sprite.custom_size = Some(vec2(width, width * 0.6));
        sprite.color = paint([0.90, 0.86, 0.74], (1.0 - along) * 0.75);
        transform.translation.x += (if sprite.flip_x { -1.0 } else { 1.0 }) * step * 40.0;
    }
}

/// The clouds go by.
fn drift_clouds(
    mut clouds: Query<(&Cloud, &mut Transform2D)>,
    paused: Res<Paused>,
    time: Res<Time>,
) {
    let step = if paused.0 { 0.0 } else { time.frame_delta };
    for (cloud, mut transform) in &mut clouds {
        transform.translation.x += cloud.speed * step;
        if transform.translation.x > ARENA.x * 0.5 + 200.0 {
            transform.translation.x = -ARENA.x * 0.5 - 200.0;
        }
    }
}

/// Turn the simulation's noises into actual noise.
fn play_noises(
    mut noises: EventReader<Noise>,
    mut audio: ResMut<Audio>,
    sounds: Option<Res<Sounds>>,
    assets: Res<Assets<Sound>>,
    muted: Res<Muted>,
) {
    let Some(sounds) = sounds else { return };
    for noise in noises.read() {
        if muted.0 {
            continue;
        }
        let handle = match noise.voice {
            Voice::Pop => sounds.pop,
            Voice::Scream => sounds.scream,
            Voice::Bonk => sounds.bonk,
            Voice::Boing => sounds.boing,
            Voice::Raspberry => sounds.raspberry,
            Voice::Chime => sounds.chime,
        };
        audio.play_with(
            &assets,
            handle,
            PlayParams {
                volume: noise.volume.clamp(0.0, 1.0),
                pitch: noise.pitch,
                pan: noise.pan,
            },
        );
    }
}

/// M turns the noise off and on. Debounced against the previous frame, since a frame system can
/// see one tick's edge twice.
fn sound_controls(
    input: Res<Input>,
    mut muted: ResMut<Muted>,
    mut audio: ResMut<Audio>,
    mut held: Local<bool>,
) {
    let down = input.pressed(Key::M);
    if down && !*held {
        muted.0 = !muted.0;
        audio.set_master_volume(if muted.0 { 0.0 } else { 0.7 });
    }
    *held = down;
}

/// What the hint says.
const HINT_LINES: &str = "click a balloon    m sound    space still    up / down pace    0 normal";

/// What the scoreboard says about you.
///
/// The counting is the joke's third act: the piece never stops you, never scores you, and never
/// says you should not have. It simply keeps a note.
fn verdict(tally: &Tally) -> &'static str {
    match tally.popped {
        0 => "a lovely day for a balloon ride",
        1 => "oh",
        2..=4 => "they are fine. they are basically fine.",
        5..=9 => "the animals have noticed a pattern",
        10..=19 => "you are on a list",
        20..=39 => "the balloon association has been informed",
        _ => "nobody was hurt. all of them are furious.",
    }
}

/// Keep the scoreboard current.
fn scoreboard(
    mut boards: Query<&mut Text, (With<Board>, Without<Ledger>)>,
    mut ledgers: Query<&mut Text, (With<Ledger>, Without<Board>)>,
    tally: Res<Tally>,
    census: Res<Census>,
    speed: Res<Speed>,
    paused: Res<Paused>,
) {
    for mut board in &mut boards {
        board.value = verdict(&tally).to_string();
        board.color = paint([0.14, 0.22, 0.34], 0.9);
    }
    for mut ledger in &mut ledgers {
        ledger.value = format!(
            "{} popped    {} by a falling animal    {} got away    {} landed    {} parachutes    {} took a bow    {} up there{}{}",
            tally.popped,
            tally.chained,
            tally.escaped,
            tally.landed,
            tally.chuted,
            tally.graceful,
            census.animals,
            if speed.0 == 1.0 {
                String::new()
            } else {
                format!("    {:.2}x", speed.0)
            },
            if paused.0 { "    still" } else { "" },
        );
        ledger.color = paint([0.16, 0.26, 0.38], 0.7);
    }
}

/// The hint fades away and comes back whenever a key is pressed.
fn hint(
    mut hints: Query<&mut Text, With<Hint>>,
    input: Res<Input>,
    time: Res<Time>,
    mut shown: Local<f32>,
) {
    let touched = [Key::M, Key::Space, Key::Up, Key::Down, Key::Digit0]
        .iter()
        .any(|key| input.pressed(*key));
    *shown = if touched {
        0.0
    } else {
        *shown + time.frame_delta
    };
    let fade = ((HINT_LIFE - *shown) / 2.0).clamp(0.0, 1.0);
    for mut hint in &mut hints {
        hint.color = paint([0.16, 0.24, 0.36], 0.55 * fade);
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Popped".into(),
        window_size: (ARENA.x as u32, ARENA.y as u32),
        // The bars around the letterboxed sky, in the same blue as the top of it.
        clear_color: paint(SKY_HIGH, 1.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Muted::default())
    .add_startup(setup)
    .add_frame_system(sound_controls)
    .add_frame_system(dress_rigs)
    .add_frame_system(dress_animals)
    .add_frame_system(paint_rigs)
    .add_frame_system(hang_ropes)
    .add_frame_system(pose_animals)
    .add_frame_system(dust)
    .add_frame_system(drift_clouds)
    .add_frame_system(play_noises)
    .add_frame_system(scoreboard)
    .add_frame_system(hint)
    .run();
}
