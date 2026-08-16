//! Spectacle, windowed: the sky, the shore, the water, and the noise. The show itself lives in
//! `game.rs` and stays renderer-free; everything here is color, size, glow and sound.
//!
//! `cargo run -p spectacle`
//!
//! - click the sky to fire a shell wherever you point
//! - `f` brings the finale forward, `c` changes the colors, `m` mutes it
//! - `space` stills it, `up`/`down` change the pace, `0` restores it
//!
//! The scenery is not decoration. A firework on plain black is a shape with no scale and no
//! distance; put a shoreline under it and water beneath that, and the same shape becomes an
//! event happening a long way off, over a town. Nearly every piece of the scenery also
//! brightens when a shell breaks, through [`Lit`], which is what ties the two halves of the
//! picture together: the light in the sky and the light on the water are the same light.

use fulcrum::prelude::*;
use simulacra_assets::assets;
use spectacle::game::{
    self, Bloom, Census, Elapsed, Field, Flash, GamePlugin, Paused, Report, Shell, Show, Smoke,
    Spark, Speed, Velocity, Voice,
};
use std::f32::consts::TAU;

/// How bright a star is at its hottest.
const SPARK_ALPHA: f32 = 0.92;
/// How much a star stretches along its travel, at rest and at full speed. A stretched star
/// reads as motion and a round one reads as an object, so the stretch is most of what makes a
/// break look like it is expanding rather than merely being large.
const SPARK_STRETCH: (f32, f32) = (1.0, 2.2);
/// Speed at which a star is drawn at full stretch, in units per second.
const STRETCH_SPEED: f32 = 320.0;
/// How bright a climbing shell is.
const SHELL_ALPHA: f32 = 0.8;
/// How thick smoke is at its fullest.
///
/// Very low, and worth being ruthless about: puffs are enormous, they pile up several deep
/// over a busy passage, and each one is a veil over everything behind it. Smoke that is
/// visible in its own right turns the whole picture to fog within a minute. At this weight it
/// is invisible until something breaks near it, which is also how it behaves at night.
const SMOKE_ALPHA: f32 = 0.017;
/// How much of the sky a break washes out at its brightest.
const SKY_WASH: f32 = 0.012;
/// How long a reflection stays on the water after the break that made it, in seconds.
const SHIMMER_LIFE: f32 = 2.4;
/// How bright a reflection is at its brightest. Water gives back a fraction of what it gets,
/// and the fraction is small.
const SHIMMER_ALPHA: f32 = 0.15;
/// Seconds the hint stays up before it fades and leaves you to watch.
const HINT_LIFE: f32 = 14.0;
/// Seconds a palette takes to cross into the next one.
const PALETTE_CROSSFADE: f32 = 2.0;

/// How many buildings stand along the far shore.
const BUILDINGS: usize = 40;
/// How many stars are out.
const NIGHT_STARS: usize = 140;
/// How many lines of light lie on the water.
const RIPPLES: usize = 34;

/// The night sky, and the glow of the town along the bottom of it.
///
/// These numbers look far too dark written down, and are not: the renderer takes them as
/// linear light and the display shows them through a gamma curve, so a value of 0.02 arrives
/// on screen about seven times brighter than it reads here. The dark end of a night sky is
/// exactly where that stretch is largest, which is also why the sky is a pair of soft glows
/// rather than a stack of bands: banding that would be invisible in the numbers is plain on
/// the screen.
const SKY_HIGH: [f32; 3] = [0.0016, 0.0022, 0.0055];
/// See [`SKY_HIGH`].
const SKY_GLOW: [f32; 3] = [0.055, 0.048, 0.062];

/// Four palettes, each filling by role the eight slots the simulation asks for.
///
/// The simulation never picks a color, only a role: a willow is gold and a salute is white
/// because that is what they burn. What gold looks like is entirely this table's business, so
/// a palette restyles the whole show without a shell knowing anything has happened.
const PALETTES: [([[f32; 3]; 8], &str); 4] = [
    (
        [
            [1.00, 0.18, 0.20],
            [1.00, 0.48, 0.14],
            [1.00, 0.80, 0.35],
            [0.32, 1.00, 0.38],
            [0.36, 0.95, 1.00],
            [0.36, 0.48, 1.00],
            [1.00, 0.34, 0.86],
            [1.00, 0.97, 0.92],
        ],
        "carnival",
    ),
    (
        [
            [1.00, 0.22, 0.10],
            [1.00, 0.46, 0.10],
            [1.00, 0.76, 0.30],
            [0.96, 0.90, 0.42],
            [1.00, 0.62, 0.36],
            [0.90, 0.34, 0.28],
            [1.00, 0.44, 0.60],
            [1.00, 0.92, 0.80],
        ],
        "ember",
    ),
    (
        [
            [0.55, 0.30, 1.00],
            [0.36, 0.52, 1.00],
            [0.55, 0.90, 1.00],
            [0.30, 1.00, 0.78],
            [0.30, 0.96, 1.00],
            [0.24, 0.44, 1.00],
            [0.78, 0.46, 1.00],
            [0.90, 0.97, 1.00],
        ],
        "harbour",
    ),
    (
        [
            [1.00, 0.56, 0.60],
            [1.00, 0.72, 0.48],
            [1.00, 0.90, 0.62],
            [0.66, 1.00, 0.72],
            [0.66, 0.96, 1.00],
            [0.62, 0.72, 1.00],
            [0.94, 0.68, 1.00],
            [1.00, 0.98, 0.94],
        ],
        "sherbet",
    ),
];

/// Texture handles, loaded once.
#[derive(Resource, Clone)]
struct Art {
    spark: Handle<Texture>,
    flare: Handle<Texture>,
    glow: Handle<Texture>,
    smoke: Handle<Texture>,
    white: Handle<Texture>,
}

/// Sound handles, loaded once.
#[derive(Resource)]
struct Noise {
    boom: Handle<Sound>,
    crack: Handle<Sound>,
    crackle: Handle<Sound>,
    launch: Handle<Sound>,
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

/// Whether the noise is turned off.
#[derive(Resource, Default)]
struct Muted(bool);

/// How a piece of scenery is sized.
#[derive(Clone, Copy)]
enum Extent {
    /// A fixed size in world units. Right for anything with a real size, like a building: the
    /// field keeps its area through a resize, so world units keep their meaning.
    Units(Vec2),
    /// A fraction of the field, for the things that have to span it whatever shape it is.
    Field(Vec2),
}

/// How high a piece of scenery sits.
#[derive(Clone, Copy)]
enum Height {
    /// World units above the water line. For objects standing on the far shore.
    Above(f32),
    /// A fraction of the field's height from its middle. For the sky, which stretches.
    Level(f32),
}

/// A piece of scenery, placed so that it follows any window shape.
#[derive(Component)]
struct Placed {
    /// Horizontal position, as a fraction of the field's width from its middle.
    x: f32,
    /// Vertical position.
    y: Height,
    /// How big it is.
    size: Extent,
    /// A shift in world units, applied last. For the pieces that belong to another piece
    /// rather than to the field, like a window in a building: the offset has to stay the same
    /// size as the thing it sits in, whatever shape the window takes.
    nudge: Vec2,
}

impl Placed {
    fn new(x: f32, y: Height, size: Extent) -> Self {
        Self {
            x,
            y,
            size,
            nudge: Vec2::ZERO,
        }
    }

    fn nudged(mut self, nudge: Vec2) -> Self {
        self.nudge = nudge;
        self
    }
}

/// What a piece of scenery looks like with nothing happening over it.
#[derive(Component)]
struct Base(Color);

/// How much of a break's light this piece catches, `0..1`.
#[derive(Component)]
struct Lit(f32);

/// A star in the night sky, with a blink of its own.
#[derive(Component)]
struct Twinkle {
    seed: f32,
    rate: f32,
    alpha: f32,
}

/// A line of light lying on the water, sliding slowly sideways.
#[derive(Component)]
struct Ripple {
    speed: f32,
}

/// The reflection of one break, wobbling on the water beneath it.
#[derive(Component)]
struct Shimmer {
    age: f32,
    color: u8,
    power: f32,
    width: f32,
    height: f32,
    phase: f32,
}

/// The ball of light a break makes at the moment it opens.
#[derive(Component)]
struct Burstlight {
    age: f32,
    color: u8,
    power: f32,
}

/// The whole-sky wash a break throws.
#[derive(Component)]
struct SkyWash;

/// Marks the hint line.
#[derive(Component)]
struct Hint;

/// A small deterministic generator, for scenery that should look scattered and be the same
/// every run.
///
/// Deliberately not `SimRng`: the shoreline is the view's business, and drawing it from the
/// simulation's stream would let a change of scenery change the show.
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

/// A color from one palette's slot.
fn slot(palette: usize, color: u8) -> [f32; 3] {
    PALETTES[palette].0[(color as usize).min(7)]
}

/// A slot's color as it currently stands, mid-crossfade or not.
fn blended(palette: &Palette, color: u8) -> [f32; 3] {
    let to = slot(palette.current, color);
    if palette.blend >= 1.0 {
        return to;
    }
    let from = slot(palette.previous, color);
    let ease = palette.blend * palette.blend * (3.0 - 2.0 * palette.blend);
    [
        from[0] + (to[0] - from[0]) * ease,
        from[1] + (to[1] - from[1]) * ease,
        from[2] + (to[2] - from[2]) * ease,
    ]
}

/// Load the art and the noise, then build the shore.
fn setup(
    mut commands: Commands,
    mut assets: AssetLoader,
    mut sounds: SoundLoader,
    mut audio: ResMut<Audio>,
) {
    let art = Art {
        spark: assets.load("spark.png"),
        flare: assets.load("flare.png"),
        glow: assets.load("glow.png"),
        smoke: assets.load("smoke.png"),
        white: assets.load("white.png"),
    };
    commands.insert_resource(Noise {
        boom: sounds.load("boom.wav"),
        crack: sounds.load("crack.wav"),
        crackle: sounds.load("crackle.wav"),
        launch: sounds.load("launch.wav"),
    });
    audio.set_master_volume(0.85);
    build_scene(&mut commands, &art);
    commands.spawn((
        Hint,
        Text::new(HINT_LINES)
            .with_size(8.0)
            .with_align(HAlign::Center)
            .with_z(30.0),
        Transform2D::default(),
    ));
    commands.insert_resource(art);
}

/// A rectangle of scenery.
#[expect(
    clippy::too_many_arguments,
    reason = "a rectangle is a position, a size, a color and a z"
)]
fn piece(
    commands: &mut Commands,
    art: &Art,
    x: f32,
    y: Height,
    size: Extent,
    anchor: Vec2,
    color: Color,
    lit: f32,
    z: f32,
) -> Entity {
    let mut sprite = Sprite::new(art.white).with_z(z);
    sprite.anchor = anchor;
    commands
        .spawn((
            Placed::new(x, y, size),
            Base(color),
            Lit(lit),
            sprite,
            Transform2D::default(),
        ))
        .id()
}

/// The sky, the far shore, the water, and the things that live on them.
///
/// Everything here is placed in fractions of the field or in units above the water line, and
/// laid out again whenever the window changes, so the same shore fits a tall window and a wide
/// one.
fn build_scene(commands: &mut Commands, art: &Art) {
    let horizon = game::WATER_SHARE - 0.5;
    let mut dice = Dice(0x5EED_1234);

    // The sky: one flat sheet the color of the top of the night, and a pair of wide soft glows
    // sitting on the horizon. The glows are the town you cannot see, and they are what stops
    // the sky reading as a void; being textures rather than rectangles, they also give a
    // gradient with nothing in it for the eye to catch on.
    piece(
        commands,
        art,
        0.0,
        Height::Level(0.0),
        Extent::Field(vec2(1.02, 1.02)),
        Vec2::splat(0.5),
        Color::rgb(SKY_HIGH[0], SKY_HIGH[1], SKY_HIGH[2]),
        0.012,
        -60.0,
    );
    for (height, strength) in [(1.15, 0.55), (0.42, 0.8), (0.16, 1.0)] {
        let mut haze = Sprite::new(art.glow).with_z(-59.0);
        haze.anchor = vec2(0.5, 0.0);
        commands.spawn((
            Placed::new(
                0.0,
                Height::Above(-30.0),
                Extent::Field(vec2(1.7, (0.5 - horizon) * height)),
            ),
            Base(Color::rgba(
                SKY_GLOW[0],
                SKY_GLOW[1],
                SKY_GLOW[2],
                0.42 * strength,
            )),
            Lit(0.02),
            haze,
            Transform2D::default(),
        ));
    }

    // Stars, dim and slow. They are the only thing on screen that is not either water or on
    // fire, and they give the sky somewhere to be.
    for _ in 0..NIGHT_STARS {
        let size = dice.range(1.6, 3.4);
        let mut sprite = Sprite::new(art.spark).with_z(-55.0);
        sprite.anchor = Vec2::splat(0.5);
        commands.spawn((
            Placed::new(
                dice.range(-0.5, 0.5),
                Height::Level(dice.range(horizon + 0.04, 0.48)),
                Extent::Units(Vec2::splat(size)),
            ),
            Base(Color::rgba(0.86, 0.90, 1.0, 1.0)),
            Lit(0.0),
            Twinkle {
                seed: dice.range(0.0, 1.0),
                rate: dice.range(0.15, 0.5),
                alpha: dice.range(0.05, 0.28),
            },
            sprite,
            Transform2D::default(),
        ));
    }

    // The glow of the town along the water line.
    let mut glow = Sprite::new(art.glow).with_z(-50.0);
    glow.anchor = Vec2::splat(0.5);
    commands.spawn((
        Placed::new(0.0, Height::Above(10.0), Extent::Field(vec2(1.4, 0.16))),
        Base(Color::rgba(0.30, 0.20, 0.10, 0.22)),
        Lit(0.03),
        glow,
        Transform2D::default(),
    ));

    // The far shore: dark blocks with a few lit windows. Almost black, because their whole job
    // is to be a hard edge under the sky, and to light up when something goes off above them.
    let mut x = -0.52;
    for _ in 0..BUILDINGS {
        let width = dice.range(16.0, 64.0);
        let height = dice.range(10.0, 78.0);
        piece(
            commands,
            art,
            x,
            Height::Above(-2.0),
            Extent::Units(vec2(width, height)),
            vec2(0.5, 0.0),
            Color::rgb(0.0035, 0.0050, 0.0105),
            0.03,
            -45.0,
        );
        let windows = (width / 16.0) as i32;
        for _ in 0..windows {
            if dice.next() > 0.55 {
                continue;
            }
            let mut lamp = Sprite::new(art.white).with_z(-44.0);
            lamp.anchor = Vec2::splat(0.5);
            commands.spawn((
                Placed::new(
                    x,
                    Height::Above(dice.range(4.0, height - 4.0) - 2.0),
                    Extent::Units(vec2(1.8, 2.6)),
                )
                .nudged(vec2(dice.range(-width * 0.32, width * 0.32), 0.0)),
                Base(Color::rgba(1.0, 0.72, 0.34, dice.range(0.16, 0.55))),
                Lit(0.0),
                lamp,
                Transform2D::default(),
            ));
        }
        x += dice.range(0.018, 0.036);
        if x > 0.52 {
            x = -0.52;
        }
    }

    // The water: darker than the sky it sits under, and the piece that catches the most light.
    piece(
        commands,
        art,
        0.0,
        Height::Level(horizon),
        Extent::Field(vec2(1.02, game::WATER_SHARE + 0.01)),
        vec2(0.5, 1.0),
        Color::rgb(0.0022, 0.0042, 0.0090),
        0.045,
        -40.0,
    );

    // Lines of light on the water, drifting. Slow enough to read as swell rather than as
    // scrolling.
    for _ in 0..RIPPLES {
        let depth = dice.range(0.02, game::WATER_SHARE);
        let mut line = Sprite::new(art.glow).with_z(-38.0);
        line.anchor = Vec2::splat(0.5);
        commands.spawn((
            Placed::new(
                dice.range(-0.5, 0.5),
                Height::Level(horizon - depth),
                Extent::Units(vec2(dice.range(60.0, 260.0), dice.range(2.0, 5.0))),
            ),
            Base(Color::rgba(0.16, 0.26, 0.42, dice.range(0.05, 0.16))),
            Lit(0.07),
            Ripple {
                speed: dice.range(-0.012, 0.012),
            },
            line,
            Transform2D::default(),
        ));
    }

    // The wash a break throws over everything, drawn last and over the top.
    let mut wash = Sprite::new(art.white).with_z(18.0);
    wash.anchor = Vec2::splat(0.5);
    commands.spawn((
        SkyWash,
        Placed::new(0.0, Height::Level(0.0), Extent::Field(vec2(1.02, 1.02))),
        wash,
        Transform2D::default(),
    ));
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
        hint.translation = vec2(0.0, -field.0.y / 2.0 + 26.0);
    }
}

/// Put every piece of scenery where the current field says it goes.
fn lay_out_scene(
    mut pieces: Query<(&Placed, Option<&Ripple>, &mut Transform2D, &mut Sprite)>,
    field: Res<Field>,
    elapsed: Res<Elapsed>,
) {
    let horizon = game::horizon(field.0);
    for (placed, ripple, mut transform, mut sprite) in &mut pieces {
        let mut x = placed.x;
        if let Some(ripple) = ripple {
            // Wrapped rather than reset, so a line leaving one side has already arrived at the
            // other and no line is ever seen to appear.
            x = (x + ripple.speed * elapsed.0 + 0.5).rem_euclid(1.0) - 0.5;
        }
        let y = match placed.y {
            Height::Above(units) => horizon + units,
            Height::Level(fraction) => fraction * field.0.y,
        };
        transform.translation = vec2(x * field.0.x, y) + placed.nudge;
        sprite.custom_size = Some(match placed.size {
            Extent::Units(size) => size,
            Extent::Field(fraction) => fraction * field.0,
        });
    }
}

/// Color the scenery, and hand it its share of the last break's light.
fn paint_scene(
    mut pieces: Query<(&Base, &Lit, Option<&Twinkle>, &mut Sprite)>,
    flash: Res<Flash>,
    palette: Res<Palette>,
    elapsed: Res<Elapsed>,
) {
    let tint = blended(&palette, flash.color);
    let level = flash.level.min(1.0);
    for (base, lit, twinkle, mut sprite) in &mut pieces {
        let share = level * lit.0;
        let mut color = Color::rgba(
            base.0.r + tint[0] * share,
            base.0.g + tint[1] * share,
            base.0.b + tint[2] * share,
            base.0.a,
        );
        if let Some(twinkle) = twinkle {
            let phase = (twinkle.seed + elapsed.0 * twinkle.rate) * TAU;
            color.a = twinkle.alpha * (0.45 + 0.55 * (0.5 + 0.5 * phase.sin()));
        }
        sprite.color = color;
    }
}

/// The wash over the whole picture when something goes off.
fn wash_sky(
    mut washes: Query<&mut Sprite, With<SkyWash>>,
    flash: Res<Flash>,
    palette: Res<Palette>,
) {
    let tint = blended(&palette, flash.color);
    // Toward white as it gets stronger: a big break does not look like colored glass over the
    // frame, it looks like the light source has moved into the room.
    let level = flash.level.min(1.0);
    let toward_white = (level * 0.5).min(0.6);
    for mut sprite in &mut washes {
        sprite.color = Color::rgba(
            tint[0] + (1.0 - tint[0]) * toward_white,
            tint[1] + (1.0 - tint[1]) * toward_white,
            tint[2] + (1.0 - tint[2]) * toward_white,
            SKY_WASH * level,
        );
    }
}

/// Give everything the simulation has made its sprite.
fn dress(
    mut commands: Commands,
    art: Option<Res<Art>>,
    sparks: Query<Entity, (With<Spark>, Without<Sprite>)>,
    shells: Query<Entity, (With<Shell>, Without<Sprite>)>,
    smoke: Query<Entity, (With<Smoke>, Without<Sprite>)>,
) {
    let Some(art) = art else { return };
    for entity in &sparks {
        commands
            .entity(entity)
            .try_insert(Sprite::new(art.spark).with_z(0.0));
    }
    for entity in &shells {
        commands
            .entity(entity)
            .try_insert(Sprite::new(art.flare).with_z(2.0));
    }
    for entity in &smoke {
        commands
            .entity(entity)
            .try_insert(Sprite::new(art.smoke).with_z(-20.0));
    }
}

/// Color and size every burning star.
///
/// Three things multiply into a star's brightness and none of them snaps: how far through its
/// burn it is, how close it is to the water, and its own blink. Stars therefore arrive, fall
/// and go out without a hard edge anywhere.
fn light_sparks(
    mut sparks: Query<(&Spark, &Velocity, &Transform2D, &mut Sprite)>,
    field: Res<Field>,
    palette: Res<Palette>,
    elapsed: Res<Elapsed>,
) {
    for (spark, velocity, transform, mut sprite) in &mut sparks {
        let color = blended(&palette, spark.color);
        let mut alpha =
            SPARK_ALPHA * spark.presence() * game::water_fade(transform.translation.y, field.0);
        if spark.twinkle > 0.0 {
            let phase = (spark.seed + elapsed.0 * spark.twinkle) * TAU;
            alpha *= 0.12 + 0.88 * (0.5 + 0.5 * phase.sin()).powf(1.4);
        }
        let pace = (velocity.0.length() / STRETCH_SPEED).clamp(0.0, 1.0);
        let length = spark.size * (SPARK_STRETCH.0 + SPARK_STRETCH.1 * pace);
        sprite.custom_size = Some(vec2(length, spark.size));
        sprite.color = Color::rgba(color[0], color[1], color[2], alpha);
    }
}

/// The shells on their way up: hot, small, and stretched by their own speed.
fn light_shells(mut shells: Query<(&Velocity, &mut Sprite), With<Shell>>) {
    for (velocity, mut sprite) in &mut shells {
        let pace = (velocity.0.length() / 460.0).clamp(0.0, 1.0);
        sprite.custom_size = Some(vec2(9.0 + 16.0 * pace, 7.0));
        sprite.color = Color::rgba(1.0, 0.82, 0.5, SHELL_ALPHA);
    }
}

/// Smoke: barely there on its own, and lit from inside by whatever breaks next.
fn light_smoke(mut puffs: Query<(&Smoke, &mut Sprite)>, flash: Res<Flash>, palette: Res<Palette>) {
    let tint = blended(&palette, flash.color);
    let level = flash.level.min(1.0);
    for (puff, mut sprite) in &mut puffs {
        let fraction = (puff.age / puff.life.max(1e-3)).clamp(0.0, 1.0);
        let rising = (fraction / 0.12).clamp(0.0, 1.0);
        let falling = 1.0 - fraction;
        let own = blended(&palette, puff.color);
        // Fresh smoke still carries the color of the break that made it, and cools to grey.
        let warmth = (1.0 - fraction).powf(2.0) * 0.22;
        sprite.custom_size = Some(Vec2::splat(puff.size * (1.0 + 1.6 * fraction)));
        sprite.color = Color::rgba(
            0.020 + own[0] * warmth + tint[0] * level * 0.22,
            0.023 + own[1] * warmth + tint[1] * level * 0.22,
            0.032 + own[2] * warmth + tint[2] * level * 0.22,
            SMOKE_ALPHA * rising * falling,
        );
    }
}

/// Put a reflection on the water under every break, and let the ones already there wobble and
/// fade.
///
/// A column rather than a mirrored copy: real water at this distance does not return an image,
/// it returns a smeared vertical wash that shakes. The wash is also the only thing that ties a
/// break to a place on the shore, which is what stops the sky and the water reading as two
/// separate pictures.
#[expect(
    clippy::too_many_arguments,
    reason = "the reflection needs the water, the light and the clock"
)]
fn shimmer(
    mut commands: Commands,
    mut blooms: EventReader<Bloom>,
    mut columns: Query<(Entity, &mut Shimmer, &mut Transform2D, &mut Sprite)>,
    art: Option<Res<Art>>,
    field: Res<Field>,
    palette: Res<Palette>,
    paused: Res<Paused>,
    elapsed: Res<Elapsed>,
    time: Res<Time>,
) {
    let Some(art) = art else { return };
    let horizon = game::horizon(field.0);

    for bloom in blooms.read() {
        let reach = ((bloom.at.y - horizon) * 0.55).max(40.0);
        // Two columns: a narrow bright one for the break itself, a wide dim one for the light
        // it throws around. Together they read as a single soft glare with a hot middle.
        for (width, height, power, phase) in [
            (34.0 * bloom.power, reach, bloom.power, 0.0),
            (150.0 * bloom.power, reach * 0.7, bloom.power * 0.5, 1.7),
        ] {
            let mut sprite = Sprite::new(art.glow).with_z(-35.0);
            sprite.anchor = Vec2::splat(0.5);
            commands.spawn((
                Shimmer {
                    age: 0.0,
                    color: bloom.color,
                    power,
                    width,
                    height,
                    phase: phase + bloom.at.x * 0.01,
                },
                sprite,
                Transform2D::from_translation(vec2(bloom.at.x, horizon - height * 0.5)),
            ));
        }
    }

    let step = if paused.0 { 0.0 } else { time.frame_delta };
    for (entity, mut column, mut transform, mut sprite) in &mut columns {
        column.age += step;
        if column.age >= SHIMMER_LIFE {
            commands.entity(entity).despawn();
            continue;
        }
        let left = 1.0 - column.age / SHIMMER_LIFE;
        let shake = (elapsed.0 * 2.3 + column.phase).sin() * 0.5 + (elapsed.0 * 3.7).sin() * 0.5;
        let color = blended(&palette, column.color);
        transform.translation.x += shake * 0.6;
        transform.translation.y = horizon - column.height * 0.5;
        sprite.custom_size = Some(vec2(
            column.width * (1.0 + 0.22 * shake),
            column.height * (1.0 + 0.05 * shake),
        ));
        sprite.color = Color::rgba(
            color[0],
            color[1],
            color[2],
            SHIMMER_ALPHA * column.power * left * left,
        );
    }
}

/// How long the ball of light at a break lasts, in seconds.
const FLARE_LIFE: f32 = 0.34;

/// The ball of light at the moment a shell opens.
///
/// The stars alone do not read as an explosion: they are already flying apart by the first
/// frame anyone sees. What sells the break is a short-lived ball of white at the middle of it,
/// gone before it can be looked at directly, which is also what the eye actually catches of a
/// real one.
fn flare_bursts(
    mut commands: Commands,
    mut blooms: EventReader<Bloom>,
    mut flares: Query<(Entity, &mut Burstlight, &mut Sprite)>,
    art: Option<Res<Art>>,
    palette: Res<Palette>,
    paused: Res<Paused>,
    time: Res<Time>,
) {
    let Some(art) = art else { return };
    for bloom in blooms.read() {
        let mut sprite = Sprite::new(art.glow).with_z(1.0);
        sprite.anchor = Vec2::splat(0.5);
        commands.spawn((
            Burstlight {
                age: 0.0,
                color: bloom.color,
                power: bloom.power,
            },
            sprite,
            Transform2D::from_translation(bloom.at),
        ));
    }

    let step = if paused.0 { 0.0 } else { time.frame_delta };
    for (entity, mut flare, mut sprite) in &mut flares {
        flare.age += step;
        if flare.age >= FLARE_LIFE {
            commands.entity(entity).despawn();
            continue;
        }
        let fraction = flare.age / FLARE_LIFE;
        let color = blended(&palette, flare.color);
        // White at the instant it opens, its own color as it goes: the middle of a break is
        // too hot to have a color, and only cools into one on the way out.
        let heat = (1.0 - fraction * 2.2).clamp(0.0, 1.0);
        sprite.custom_size = Some(Vec2::splat(
            (70.0 + 260.0 * fraction.powf(0.6)) * flare.power,
        ));
        sprite.color = Color::rgba(
            color[0] + (1.0 - color[0]) * heat,
            color[1] + (1.0 - color[1]) * heat,
            color[2] + (1.0 - color[2]) * heat,
            0.38 * flare.power * (1.0 - fraction).powf(1.6),
        );
    }
}

/// Turn the reports the simulation has let go into actual noise.
fn play_reports(
    mut reports: EventReader<Report>,
    mut audio: ResMut<Audio>,
    noise: Option<Res<Noise>>,
    sounds: Res<Assets<Sound>>,
    muted: Res<Muted>,
) {
    let Some(noise) = noise else { return };
    for report in reports.read() {
        if muted.0 {
            continue;
        }
        let (handle, gain) = match report.voice {
            Voice::Launch => (noise.launch, 0.45),
            Voice::Boom => (noise.boom, 1.0),
            Voice::Crack => (noise.crack, 0.9),
            Voice::Crackle => (noise.crackle, 0.6),
        };
        audio.play_with(
            &sounds,
            handle,
            PlayParams {
                volume: (report.volume * gain).clamp(0.0, 1.0),
                pitch: report.pitch,
                pan: report.pan,
            },
        );
    }
}

/// C crosses to the next palette, M turns the noise off and on. Debounced against the previous
/// frame, since a frame system can see one tick's edge twice.
fn look_controls(
    input: Res<Input>,
    mut palette: ResMut<Palette>,
    mut muted: ResMut<Muted>,
    mut audio: ResMut<Audio>,
    mut held: Local<(bool, bool)>,
) {
    let (palette_down, mute_down) = (input.pressed(Key::C), input.pressed(Key::M));
    if palette_down && !held.0 {
        palette.previous = palette.current;
        palette.current = (palette.current + 1) % PALETTES.len();
        palette.blend = 0.0;
    }
    if mute_down && !held.1 {
        muted.0 = !muted.0;
        audio.set_master_volume(if muted.0 { 0.0 } else { 0.85 });
    }
    *held = (palette_down, mute_down);
}

/// Cross the palette over, once a change has been asked for.
fn advance_palette(mut palette: ResMut<Palette>, time: Res<Time>) {
    if palette.blend < 1.0 {
        palette.blend = (palette.blend + time.frame_delta / PALETTE_CROSSFADE).min(1.0);
    }
}

/// What the hint says. The only text the show ever puts up, and only for a little while.
const HINT_LINES: &str = "click the sky to fire one    f finale    c colors    m sound\n\
                          space still    up / down pace    0 normal";

/// Keep the hint's words current. Separate from the fade so neither has to know about the
/// other's business.
fn hint_text(
    mut hints: Query<&mut Text, With<Hint>>,
    show: Res<Show>,
    census: Res<Census>,
    speed: Res<Speed>,
    paused: Res<Paused>,
    palette: Res<Palette>,
) {
    for mut hint in &mut hints {
        hint.value = format!(
            "{HINT_LINES}\n{}    {}    {} stars{}{}",
            show.act.name(),
            PALETTES[palette.current].1,
            census.sparks,
            if speed.0 == 1.0 {
                String::new()
            } else {
                format!("    {:.2}x", speed.0)
            },
            if paused.0 { "    still" } else { "" },
        );
    }
}

/// The hint fades away and comes back whenever a key is pressed, so the show spends nearly all
/// of its time with nothing on it but sky.
fn hint(
    mut hints: Query<&mut Text, With<Hint>>,
    input: Res<Input>,
    time: Res<Time>,
    mut shown: Local<f32>,
) {
    let touched = [
        Key::C,
        Key::M,
        Key::F,
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
    for mut hint in &mut hints {
        hint.color = Color::rgba(0.78, 0.82, 0.92, 0.34 * fade);
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Spectacle".into(),
        window_size: (game::DEFAULT_FIELD.x as u32, game::DEFAULT_FIELD.y as u32),
        // The top of the sky, so the gradient meets the edge of the window and the picture has
        // no border.
        clear_color: Color::rgb(SKY_HIGH[0], SKY_HIGH[1], SKY_HIGH[2]),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Palette::default())
    .insert_resource(Muted::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(lay_out_scene)
    .add_frame_system(look_controls)
    .add_frame_system(advance_palette)
    .add_frame_system(dress)
    .add_frame_system(light_sparks)
    .add_frame_system(light_shells)
    .add_frame_system(light_smoke)
    .add_frame_system(paint_scene)
    .add_frame_system(wash_sky)
    .add_frame_system(shimmer)
    .add_frame_system(flare_bursts)
    .add_frame_system(play_reports)
    .add_frame_system(hint_text)
    .add_frame_system(hint)
    .run();
}
