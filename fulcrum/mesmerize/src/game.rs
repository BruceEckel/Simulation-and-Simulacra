//! Mesmerize: a few thousand motes of light carried by a slow, invisible current, breathing.
//!
//! Three decisions do nearly all of the work, and each is load-bearing rather than decorative:
//!
//! **The current is a curl field.** Velocity is the curl of a small sum of sine waves, which
//! makes it divergence-free: it has no sources and no sinks. A field built the obvious way —
//! pick an angle per point — grows permanent attractors, and within a minute every mote is
//! piled into them and the piece is dead. See [`flow`].
//!
//! This does *not* mean the motes stay evenly spread, and they had better not: because they
//! carry inertia they lag the current, get thrown out of the vortex cores, and gather along
//! the strands between them. That gathering is where all the visible structure comes from —
//! the filaments, the dark centers, the sense of depth. What the curl buys is that the
//! gathering is never permanent: the strands keep moving, dissolving and reforming, instead
//! of becoming pits that everything falls into and stays.
//!
//! **It breathes at five and a half breaths a minute.** [`BREATH_PERIOD`] is eleven seconds,
//! with a shorter draw and a longer release, which is the pace slow-breathing practice aims
//! for. The whole field swells and settles on that cycle: it brightens and spreads on the
//! draw, dims and gathers on the release. Watching something breathe at a rate you can follow
//! is the part that might actually reach your nervous system. There is a ring you can bring
//! in to breathe along with deliberately, but it is off unless asked for — the light carries
//! the rhythm on its own, and an object in the middle of the picture is a thing to look at
//! rather than a thing to rest in.
//!
//! **Nothing ever ends abruptly.** Motes fade in, fade out, and dissolve near the boundary
//! rather than crossing it; the current itself drifts on its own slow clock so the pattern
//! never repeats. There are no collisions, no goals, and no failure states — there is nothing
//! in here to be startled by.
//!
//! Pure logic — no sprites, no audio — so it runs headless for the determinism test.
//!
//! The field resizes with the window through [`FIELD_COMMAND`] on the replayable command
//! channel, never by reading renderer state; see the note in `boids` for why.

use fulcrum::prelude::*;
use std::f32::consts::TAU;

/// Field size at startup, and the area every resize preserves.
pub const DEFAULT_FIELD: Vec2 = Vec2::new(1024.0, 768.0);
/// Aspect-ratio limits for a resize.
pub const ASPECT_LIMITS: (f32, f32) = (0.4, 3.2);
/// Name of the resize command on the replayable command channel.
pub const FIELD_COMMAND: &str = "field";

/// Motes at startup. Density is most of the beauty here: a few thousand separate dots read as
/// scattered confetti, while several thousand overlapping ones read as a single moving body
/// of light.
pub const START_MOTES: u32 = 5600;
/// The most the field will hold. Not an aesthetic limit — a promise that the tick stays quick.
pub const MAX_MOTES: u32 = 24_000;
/// Motes added or removed per repeat while a population key is held.
pub const HOLD_BATCH: u32 = 90;
/// Ticks a held key must be down before it starts repeating.
pub const HOLD_DELAY: u32 = 10;
/// Ticks between arrivals once a held key is repeating.
pub const HOLD_PERIOD: u32 = 2;

/// How fast the current carries a mote, in units per second. Slow on purpose: fast motion is
/// stimulating, and the whole piece is aimed the other way.
pub const FLOW_SPEED: f32 = 46.0;
/// How quickly a mote's velocity converges on the current, per second. Low values give the
/// motion its lag and its silkiness; high values make motes snap to the field and look rigid.
pub const FLOW_INERTIA: f32 = 2.2;
// How fast the current itself evolves is set per wave in `WAVES`, in radians per second: the
// leading swirl turns over in about seventy seconds, so the pattern you are watching is never
// quite the pattern you were watching, and never obviously changing either.

/// Shortest and longest a mote lives, in seconds.
pub const LIFETIME: (f32, f32) = (16.0, 40.0);
/// Fraction of its life a mote spends fading in, and again fading out.
pub const FADE: f32 = 0.22;
/// How far from the boundary a mote begins to dissolve, so nothing is ever seen to leave.
pub const EDGE_FADE: f32 = 90.0;

/// One full breath, in seconds. Eleven seconds is five and a half breaths a minute, inside the
/// band that slow-breathing practice settles on.
pub const BREATH_PERIOD: f32 = 11.0;
/// Fraction of the breath spent drawing in. The shorter half; the release is the longer one.
pub const BREATH_INHALE: f32 = 0.42;
/// How much the breath quickens the current at its peak.
pub const BREATH_FLOW: f32 = 0.15;

/// How far the pointer's wake reaches.
pub const STIR_RADIUS: f32 = 210.0;
/// How hard a moving pointer turns the current around itself.
pub const STIR_STRENGTH: f32 = 130.0;
/// How quickly the pointer's wake fades once it stops moving, per second.
pub const STIR_DECAY: f32 = 1.7;
/// Pointer travel per second that counts as a full-strength stir.
pub const STIR_FULL_SPEED: f32 = 600.0;

/// Slowest the piece can run, as a multiple of real time.
pub const SPEED_MIN: f32 = 0.15;
/// Fastest it can run.
pub const SPEED_MAX: f32 = 4.0;
/// How much a held speed key multiplies the rate each tick.
pub const SPEED_RAMP: f32 = 1.02;

/// The waves the current is built from: `(cycles across x, cycles across y, weight, drift)`.
///
/// Integer cycle counts make the field exactly periodic across the box, so its structure meets
/// itself at the edges instead of ending. The weights fall away so the large, slow swirl leads
/// and the small ones only texture it.
pub const WAVES: [(f32, f32, f32, f32); 4] = [
    (1.0, 1.0, 1.00, 0.09),
    (-1.0, 2.0, 0.55, -0.13),
    (2.0, -2.0, 0.30, 0.17),
    (3.0, 3.0, 0.16, -0.23),
];

/// One mote of light.
#[derive(Component)]
pub struct Mote {
    /// Where this mote sits in the palette, `0..1`. Fixed for its lifetime, so a mote keeps
    /// its color while the palette itself drifts around it.
    pub stream: f32,
    /// Seconds lived.
    pub age: f32,
    /// Seconds it will live.
    pub life: f32,
}

impl Mote {
    /// How present a mote is, `0..1`: it arrives from nothing and leaves the same way.
    pub fn presence(&self) -> f32 {
        let fraction = (self.age / self.life.max(1e-3)).clamp(0.0, 1.0);
        let rising = (fraction / FADE).clamp(0.0, 1.0);
        let falling = ((1.0 - fraction) / FADE).clamp(0.0, 1.0);
        // Smoothstep both ends: a linear fade still has a visible corner where it starts.
        let ease = |t: f32| t * t * (3.0 - 2.0 * t);
        ease(rising) * ease(falling)
    }
}

/// Simulation velocity, units/second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// The box the current fills. Simulation state, changed only by [`FIELD_COMMAND`].
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Field(pub Vec2);

impl Default for Field {
    fn default() -> Self {
        Self(DEFAULT_FIELD)
    }
}

/// Where the breath is. Its own clock, so the piece keeps breathing at the same rate whatever
/// the simulation speed is doing.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Breath {
    /// Seconds into the current breath.
    pub seconds: f32,
    /// `0` at the bottom of the release, `1` at the top of the draw.
    pub phase: f32,
    /// Total seconds elapsed, which is also what the current drifts on.
    pub elapsed: f32,
}

/// The pointer's wake: where it is and how much of a turn it is still giving the current.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Stir {
    /// Where the wake is centered.
    pub at: Vec2,
    /// How strong it is, `0..1`, decaying once the pointer stops.
    pub strength: f32,
    /// Where the pointer was last tick.
    pub was: Vec2,
    /// Whether a pointer position has been seen yet, so the first frame isn't a jump from
    /// the origin.
    pub started: bool,
}

/// How many motes are in the field.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Census(pub u32);

/// Nothing moves while this is set.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Paused(pub bool);

/// How fast the piece runs, as a multiple of real time.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Speed(pub f32);

impl Default for Speed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// How far this tick advances. Written once per tick by [`set_step`].
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Step {
    /// Seconds this tick, already scaled by speed. Zero while paused.
    pub seconds: f32,
    /// The speed multiplier, or zero while paused.
    pub scale: f32,
}

/// How long each held key has been down, for key repeat.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Holds {
    /// Ticks the add key has been down.
    pub more: u32,
    /// Ticks the remove key has been down.
    pub fewer: u32,
}

/// The field a window of this pixel size should get: the window's aspect at [`DEFAULT_FIELD`]'s
/// area, so the density of light stays put while the shape follows the window.
pub fn field_for_window(window: Vec2) -> Vec2 {
    let area = DEFAULT_FIELD.x * DEFAULT_FIELD.y;
    let aspect = (window.x / window.y).clamp(ASPECT_LIMITS.0, ASPECT_LIMITS.1);
    vec2(
        (area * aspect).sqrt().round(),
        (area / aspect).sqrt().round(),
    )
}

/// Encode a field size for [`FIELD_COMMAND`]: whole units, so it round-trips exactly.
pub fn field_payload(size: Vec2) -> String {
    format!("{} {}", size.x as i32, size.y as i32)
}

/// Decode a [`field_payload`]. `None` for anything malformed or degenerate.
pub fn parse_field(payload: &str) -> Option<Vec2> {
    let (width, height) = payload.split_once(' ')?;
    let size = vec2(
        width.trim().parse::<i32>().ok()? as f32,
        height.trim().parse::<i32>().ok()? as f32,
    );
    (size.x >= 1.0 && size.y >= 1.0).then_some(size)
}

/// The current at a point, in units per second.
///
/// Each wave contributes `(k.y, -k.x)/|k| · w · cos(k·p + drift·t)`, which is the curl of a
/// sine wave and therefore divergence-free. A sum of curls is still a curl, so the whole field
/// is: motes are carried around forever and never collected anywhere. That single property is
/// what lets this run for an hour and still look like something.
pub fn flow(point: Vec2, field: Vec2, time: f32) -> Vec2 {
    let mut current = Vec2::ZERO;
    for (cycles_x, cycles_y, weight, drift) in WAVES {
        let wave = vec2(TAU * cycles_x / field.x, TAU * cycles_y / field.y);
        let length = wave.length().max(1e-6);
        let phase = wave.dot(point) + drift * time;
        current += vec2(wave.y, -wave.x) / length * (weight * phase.cos());
    }
    current * FLOW_SPEED
}

/// Where the breath is at `seconds` into its cycle: `0` at the bottom of the release, `1` at
/// the top of the draw.
///
/// Asymmetric — the draw is the shorter half — because an even sine feels mechanical, while a
/// long release is the half that does the calming. Smoothstepped at both turns so there is no
/// moment where the motion changes direction sharply.
pub fn breath_phase(seconds: f32) -> f32 {
    let cycle = (seconds / BREATH_PERIOD).rem_euclid(1.0);
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    if cycle < BREATH_INHALE {
        ease(cycle / BREATH_INHALE)
    } else {
        1.0 - ease((cycle - BREATH_INHALE) / (1.0 - BREATH_INHALE))
    }
}

/// How visible a mote is at this position, `0..1`. Motes dissolve as they approach the
/// boundary, so none is ever seen to cross it — the field simply ends in nothing.
pub fn edge_presence(point: Vec2, field: Vec2) -> f32 {
    let inside = field / 2.0 - point.abs();
    let nearest = inside.x.min(inside.y);
    let fraction = (nearest / EDGE_FADE).clamp(0.0, 1.0);
    fraction * fraction * (3.0 - 2.0 * fraction)
}

/// Installs the piece.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Field::default());
        app.world_mut().insert_resource(Breath::default());
        app.world_mut().insert_resource(Stir::default());
        app.world_mut().insert_resource(Census::default());
        app.world_mut().insert_resource(Paused::default());
        app.world_mut().insert_resource(Speed::default());
        app.world_mut().insert_resource(Step::default());
        app.world_mut().insert_resource(Holds::default());
        app.add_systems(Startup, fill_field);
        app.add_systems(
            FixedUpdate,
            (
                apply_field,
                pace,
                set_step,
                breathe,
                stir_from_pointer,
                population_controls,
                reseed,
                drift,
            )
                .chain(),
        );
    }
}

/// Fill the field. Public so the binary can order its sprite-attachment after it.
pub fn fill_field(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    field: Res<Field>,
) {
    add_motes(&mut commands, &mut rng, &mut census, field.0, START_MOTES);
}

/// Put `count` motes into the field at random positions, at random points in their lives so
/// they don't all fade together.
fn add_motes(
    commands: &mut Commands,
    rng: &mut SimRng,
    census: &mut Census,
    field: Vec2,
    count: u32,
) {
    let limit = field / 2.0;
    for _ in 0..count.min(MAX_MOTES.saturating_sub(census.0)) {
        let position = vec2(
            rng.range_f32(-limit.x..limit.x),
            rng.range_f32(-limit.y..limit.y),
        );
        let life = rng.range_f32(LIFETIME.0..LIFETIME.1);
        commands.spawn((
            Mote {
                stream: rng.range_f32(0.0..1.0),
                age: rng.range_f32(0.0..life),
                life,
            },
            Transform2D::from_translation(position),
            Velocity(flow(position, field, 0.0)),
        ));
        census.0 += 1;
    }
}

/// Send a mote back out into the field as a new one.
fn renew(
    mote: &mut Mote,
    transform: &mut Transform2D,
    velocity: &mut Velocity,
    rng: &mut SimRng,
    field: Vec2,
    time: f32,
) {
    let limit = field / 2.0;
    let position = vec2(
        rng.range_f32(-limit.x..limit.x),
        rng.range_f32(-limit.y..limit.y),
    );
    mote.life = rng.range_f32(LIFETIME.0..LIFETIME.1);
    mote.age = 0.0;
    mote.stream = rng.range_f32(0.0..1.0);
    transform.translation = position;
    velocity.0 = flow(position, field, time);
}

/// Move the boundary when a resize command arrives, and bring the motes inside it.
fn apply_field(
    mut field: ResMut<Field>,
    mut orders: EventReader<CommandEvent>,
    mut motes: Query<&mut Transform2D, With<Mote>>,
) {
    let mut resized = false;
    for order in orders.read() {
        if order.name != FIELD_COMMAND {
            continue;
        }
        if let Some(size) = parse_field(&order.payload) {
            field.0 = size;
            resized = true;
        }
    }
    if !resized {
        return;
    }
    let limit = field.0 / 2.0;
    for mut mote in &mut motes {
        mote.translation = mote.translation.clamp(-limit, limit);
    }
}

/// Stillness and pace. There is no "restart" here on purpose — the piece has no state worth
/// resetting, only a field you can reseed.
fn pace(mut paused: ResMut<Paused>, mut speed: ResMut<Speed>, input: Res<Input>) {
    if input.just_pressed(Key::Space) {
        paused.0 = !paused.0;
    }
    if input.pressed(Key::Up) {
        speed.0 *= SPEED_RAMP;
    }
    if input.pressed(Key::Down) {
        speed.0 /= SPEED_RAMP;
    }
    if input.just_pressed(Key::Digit0) {
        speed.0 = 1.0;
    }
    speed.0 = speed.0.clamp(SPEED_MIN, SPEED_MAX);
}

/// Fix this tick's step. Pausing is a step of zero.
fn set_step(mut step: ResMut<Step>, time: Res<Time>, speed: Res<Speed>, paused: Res<Paused>) {
    step.scale = if paused.0 { 0.0 } else { speed.0 };
    step.seconds = time.fixed_delta * step.scale;
}

/// Advance the breath.
///
/// On real time rather than simulated time: the breath is an invitation to breathe along with
/// it, and an invitation that changes its rate when you speed the current up is no use to
/// anybody. Speed changes what the light does, never how fast it breathes.
fn breathe(mut breath: ResMut<Breath>, time: Res<Time>, paused: Res<Paused>) {
    if paused.0 {
        return;
    }
    breath.seconds = (breath.seconds + time.fixed_delta).rem_euclid(BREATH_PERIOD);
    breath.phase = breath_phase(breath.seconds);
    breath.elapsed += time.fixed_delta;
}

/// Turn pointer movement into a wake in the current.
///
/// Strength comes from how fast the pointer is travelling, not merely from where it is, so a
/// resting hand leaves the field alone and a moving one draws through it. The wake then fades
/// on its own rather than stopping when the hand does.
fn stir_from_pointer(mut stir: ResMut<Stir>, input: Res<Input>, step: Res<Step>) {
    if step.seconds <= 0.0 {
        return;
    }
    let pointer = input.mouse_world();
    if !stir.started {
        stir.was = pointer;
        stir.started = true;
    }
    let travelled = (pointer - stir.was).length() / step.seconds;
    stir.was = pointer;
    stir.at = pointer;
    let fresh = (travelled / STIR_FULL_SPEED).clamp(0.0, 1.0);
    let faded = stir.strength * (-STIR_DECAY * step.seconds).exp();
    stir.strength = faded.max(fresh);
}

/// Key repeat: fires the tick a key goes down, then again every [`HOLD_PERIOD`] ticks once it
/// has been held for [`HOLD_DELAY`].
fn repeating(held: &mut u32, down: bool) -> bool {
    if !down {
        *held = 0;
        return false;
    }
    let fire =
        *held == 0 || (*held >= HOLD_DELAY && (*held - HOLD_DELAY).is_multiple_of(HOLD_PERIOD));
    *held += 1;
    fire
}

/// Hold N for more light, M for less.
fn population_controls(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    mut holds: ResMut<Holds>,
    field: Res<Field>,
    input: Res<Input>,
    motes: Query<Entity, With<Mote>>,
) {
    if repeating(&mut holds.more, input.pressed(Key::N)) {
        add_motes(&mut commands, &mut rng, &mut census, field.0, HOLD_BATCH);
    }
    if repeating(&mut holds.fewer, input.pressed(Key::M)) {
        for entity in motes.iter().take(HOLD_BATCH as usize) {
            commands.entity(entity).despawn();
            census.0 = census.0.saturating_sub(1);
        }
    }
}

/// R scatters the motes again without disturbing the current they are riding.
fn reseed(
    mut motes: Query<(&mut Mote, &mut Transform2D, &mut Velocity)>,
    mut rng: ResMut<SimRng>,
    field: Res<Field>,
    breath: Res<Breath>,
    input: Res<Input>,
) {
    if !input.just_pressed(Key::R) {
        return;
    }
    for (mut mote, mut transform, mut velocity) in &mut motes {
        renew(
            &mut mote,
            &mut transform,
            &mut velocity,
            &mut rng,
            field.0,
            breath.elapsed,
        );
    }
}

/// Carry every mote along the current, and send the ones whose time is up back out as new.
fn drift(
    mut motes: Query<(&mut Mote, &mut Transform2D, &mut Velocity)>,
    field: Res<Field>,
    breath: Res<Breath>,
    stir: Res<Stir>,
    step: Res<Step>,
    mut rng: ResMut<SimRng>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    let time = breath.elapsed;
    let quickening = 1.0 + BREATH_FLOW * (breath.phase * 2.0 - 1.0);
    // Exponential approach, so the lag is the same at any tick rate or speed setting.
    let blend = 1.0 - (-FLOW_INERTIA * dt).exp();
    let limit = field.0 / 2.0;

    for (mut mote, mut transform, mut velocity) in &mut motes {
        mote.age += dt;
        if mote.age >= mote.life {
            renew(
                &mut mote,
                &mut transform,
                &mut velocity,
                &mut rng,
                field.0,
                time,
            );
            continue;
        }

        let position = transform.translation;
        let mut target = flow(position, field.0, time) * quickening;

        // The pointer's wake: a turn around it, strongest at its center, gone by its edge.
        if stir.strength > 0.001 {
            let offset = position - stir.at;
            let distance = offset.length();
            if distance < STIR_RADIUS && distance > 1e-3 {
                let falloff = 1.0 - distance / STIR_RADIUS;
                let around = vec2(-offset.y, offset.x) / distance;
                target += around * STIR_STRENGTH * stir.strength * falloff * falloff;
            }
        }

        let carried = velocity.0 + (target - velocity.0) * blend;
        velocity.0 = carried;
        // Point the mote along its travel. The view draws each one stretched along this
        // heading, which is what turns a field of dots into a field of strokes.
        transform.rotation = carried.to_angle();
        let next = position + carried * dt;
        // A mote that reaches the boundary has already faded to nothing by the time it gets
        // there, so it is simply sent back out as a new one rather than being turned around.
        if next.x.abs() > limit.x || next.y.abs() > limit.y {
            renew(
                &mut mote,
                &mut transform,
                &mut velocity,
                &mut rng,
                field.0,
                time,
            );
        } else {
            transform.translation = next;
        }
    }
}
