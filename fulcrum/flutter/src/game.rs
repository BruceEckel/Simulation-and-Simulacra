//! Flutter: a room full of moths, and the two dials that matter — how many there are, and how
//! fast it all runs.
//!
//! Three decisions carry the piece:
//!
//! **A moth's path depends on nothing but that moth.** Its wander is read out of a seeded
//! function of the clock ([`wobble`]) rather than accumulated from random draws, so no moth's
//! flight depends on where it sits in storage or on which moths happen to exist. That is what
//! lets the swarm grow to twenty thousand and back down to ten while the simulation stays
//! reproducible: adding a moth cannot disturb the one next to it.
//!
//! **Population is a target, not an event.** [`Flock::target`] says how many moths there should
//! be; [`muster`] moves the target and [`resize_swarm`] makes the world agree with it, spawning
//! the missing ordinals or despawning the surplus ones. Holding a key scales the target
//! geometrically, which is the only way one key can take you from ten moths to twenty thousand
//! and still let you pick out fifty.
//!
//! **Speed is a step, not a tick rate.** The engine's tick rate is fixed — that is the whole
//! determinism promise — so speed here is a multiplier on how far one tick advances
//! ([`Step::seconds`]). Everything reads that instead of `Time::fixed_delta`, wingbeats
//! included, so a moth at 4x flies four times as far and flaps four times as fast.
//!
//! Pure logic, no sprites, so it runs headless for the determinism test. The binary decides
//! what a moth looks like.

use fulcrum::prelude::*;
use std::f32::consts::{PI, TAU};

/// The room, in world units.
pub const ARENA: Vec2 = Vec2::new(1280.0, 800.0);

/// Moths at startup. Enough to look like a swarm, few enough to pick one out and follow it.
pub const START_MOTHS: u32 = 400;
/// The most the room will hold. Past this the frame rate, not the simulation, is the limit.
pub const MAX_MOTHS: u32 = 30_000;
/// How much the target population grows or shrinks per tick while a population key is held.
/// Geometric, so one key covers three orders of magnitude without becoming useless at the
/// bottom of the range: a tap is one moth, a held key doubles the swarm about every half
/// second.
pub const SWARM_RAMP: f32 = 1.035;

/// How much the speed multiplier changes per tick while a speed key is held.
pub const SPEED_RAMP: f32 = 1.02;
/// Slowest the simulation will run, as a multiple of real time.
pub const SPEED_MIN: f32 = 0.05;
/// Fastest the simulation will run.
pub const SPEED_MAX: f32 = 8.0;

/// Slowest and fastest a moth cruises, in units per second.
pub const CRUISE: (f32, f32) = (58.0, 132.0);
/// Smallest and largest wingspan, in world units.
pub const WINGSPAN: (f32, f32) = (13.0, 30.0);
/// How hard a moth wanders when nothing else is pulling on it, in radians per second.
pub const WANDER: f32 = 2.1;
/// How hard the lamp turns a moth that is right on top of it, in radians per second.
pub const LAMP_PULL: f32 = 3.6;
/// How far the lamp reaches, in world units. Beyond this a moth is on its own.
pub const LAMP_REACH: f32 = 440.0;
/// How far from a wall a moth starts turning back, in world units.
pub const MARGIN: f32 = 110.0;
/// How hard a wall turns a moth that has gone the whole margin into it, in radians per second.
pub const EDGE_PUSH: f32 = 5.0;

/// Frames in the wingbeat — one row of tiles in `moth.png`.
pub const WING_FRAMES: u32 = 8;
/// Wingbeats per second for a moth cruising at the slow end. Faster moths beat faster.
pub const WINGBEAT: f32 = 7.0;
/// How far a moth sways across its own path over a wingbeat, in units per second. Moths do not
/// fly in lines, and this is most of why these ones do not either.
pub const FLUTTER: f32 = 26.0;

/// One moth. Position and heading live in its [`Transform2D`]; everything here is what makes
/// this moth different from the moth beside it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Moth {
    /// Where this moth came in the swarm. The live moths are always exactly the ordinals
    /// `0..Flock::count`, which is what makes "take fifty away" a decision rather than a
    /// scramble over whatever the query happens to yield first.
    pub ordinal: u32,
    /// Fixed at spawn; seeds this moth's wander and nothing else.
    pub seed: u32,
    /// Cruise speed, in units per second at 1x.
    pub speed: f32,
    /// Wingspan in world units — the drawn size, and the reason big moths look slower.
    pub wingspan: f32,
    /// Wingbeats per second at 1x.
    pub beat: f32,
    /// Where this moth is in its wingbeat, in `0..1`.
    pub wing: f32,
    /// Which way it is flying, in radians. Kept beside the transform's rotation because the
    /// transform is the drawn pose and this is the intent.
    pub heading: f32,
    /// This moth's place in the palette, in `0..1`. The binary decides what that means.
    pub tone: f32,
}

/// How many moths there should be, and how many there are.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flock {
    /// How many moths the room is being asked for.
    pub target: u32,
    /// How many it currently holds. Equal to `target` after every [`resize_swarm`].
    pub count: u32,
    /// Set to throw the whole swarm away and draw a fresh one next tick.
    pub restock: bool,
}

impl Default for Flock {
    fn default() -> Self {
        Self {
            target: START_MOTHS,
            count: 0,
            restock: false,
        }
    }
}

/// Nothing moves while this is set.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Paused(pub bool);

/// How fast the room runs, as a multiple of real time.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Speed(pub f32);

impl Default for Speed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// How far this tick advances. Written once per tick by [`set_step`]; every other system reads
/// this and never [`Time::fixed_delta`], which is what makes the speed dial work.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Step {
    /// Seconds this tick, already scaled by speed. Zero while paused.
    pub seconds: f32,
    /// The speed multiplier, or zero while paused.
    pub scale: f32,
}

/// Simulated seconds since startup. Runs slow, fast or not at all with [`Speed`], because the
/// wander is read from it.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Clock(pub f32);

/// The light the moths are here for.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Lamp {
    /// Where it is, in world units. Follows the pointer.
    pub at: Vec2,
    /// Whether it is lit. Put it out and the swarm comes apart.
    pub on: bool,
}

impl Default for Lamp {
    fn default() -> Self {
        Self {
            at: Vec2::ZERO,
            on: true,
        }
    }
}

/// A smooth wander in about `-1..1`, seeded per moth and read from the clock.
///
/// Two sines with periods that do not divide each other, so a moth does not visibly repeat
/// itself. Reading rather than accumulating is the point: this is a pure function of
/// `(seed, clock)`, so a moth flies the same path whatever else is in the room and in whatever
/// order the swarm is stored — which is what keeps a simulation you can add ten thousand
/// entities to mid-flight reproducible.
pub fn wobble(seed: u32, clock: f32) -> f32 {
    let phase = (seed & 0xffff) as f32 / 65_536.0 * TAU;
    let offset = (seed >> 16) as f32 / 65_536.0 * TAU;
    let rate = 0.9 + (seed % 89) as f32 * 0.021;
    (clock * rate + phase).sin() * 0.62 + (clock * rate * 0.37 + offset).sin() * 0.38
}

/// The shortest turn from heading `from` to heading `to`, in radians.
pub fn shortest_turn(from: f32, to: f32) -> f32 {
    let mut turn = (to - from) % TAU;
    if turn > PI {
        turn -= TAU;
    } else if turn < -PI {
        turn += TAU;
    }
    turn
}

/// Which frame of the wingbeat a phase in `0..1` is showing.
pub fn wing_frame(wing: f32) -> u32 {
    ((wing.fract() + 1.0).fract() * WING_FRAMES as f32) as u32 % WING_FRAMES
}

/// Installs the room.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Flock::default());
        app.world_mut().insert_resource(Paused::default());
        app.world_mut().insert_resource(Speed::default());
        app.world_mut().insert_resource(Step::default());
        app.world_mut().insert_resource(Clock::default());
        app.world_mut().insert_resource(Lamp::default());
        app.add_systems(
            FixedUpdate,
            (
                pace,
                muster,
                set_step,
                follow_pointer,
                advance_clock,
                resize_swarm,
                fly,
            )
                .chain(),
        );
    }
}

/// Stillness, pace, and the lamp switch.
fn pace(
    mut speed: ResMut<Speed>,
    mut paused: ResMut<Paused>,
    mut lamp: ResMut<Lamp>,
    input: Res<Input>,
) {
    if input.just_pressed(Key::Space) {
        paused.0 = !paused.0;
    }
    if input.pressed(Key::Right) || input.pressed(Key::D) {
        speed.0 *= SPEED_RAMP;
    }
    if input.pressed(Key::Left) || input.pressed(Key::A) {
        speed.0 /= SPEED_RAMP;
    }
    if input.just_pressed(Key::Digit0) {
        speed.0 = 1.0;
    }
    speed.0 = speed.0.clamp(SPEED_MIN, SPEED_MAX);
    if input.just_pressed(Key::L) {
        lamp.on = !lamp.on;
    }
}

/// More moths, fewer moths, or a whole new set of them.
///
/// The target scales rather than steps, but never by less than one: at four hundred moths a
/// held key is worth fourteen a tick, at forty it is worth one, so the same key is useful at
/// both ends.
fn muster(mut flock: ResMut<Flock>, input: Res<Input>) {
    let current = flock.target as f32;
    let mut target = current;
    if input.pressed(Key::Up) || input.pressed(Key::W) {
        target = (current * SWARM_RAMP).max(current + 1.0);
    }
    if input.pressed(Key::Down) || input.pressed(Key::S) {
        target = (current / SWARM_RAMP).min(current - 1.0);
    }
    flock.target = target.round().clamp(0.0, MAX_MOTHS as f32) as u32;
    if input.just_pressed(Key::R) {
        flock.restock = true;
    }
}

/// Fix this tick's step. Pausing is a step of zero, which every system already handles.
fn set_step(mut step: ResMut<Step>, time: Res<Time>, speed: Res<Speed>, paused: Res<Paused>) {
    step.scale = if paused.0 { 0.0 } else { speed.0 };
    step.seconds = time.fixed_delta * step.scale;
}

/// The lamp goes where the pointer is.
fn follow_pointer(mut lamp: ResMut<Lamp>, input: Res<Input>) {
    let limit = ARENA / 2.0;
    lamp.at = input.mouse_world().clamp(-limit, limit);
}

/// Advance simulated time, at whatever pace was asked for.
fn advance_clock(mut clock: ResMut<Clock>, step: Res<Step>) {
    clock.0 += step.seconds;
}

/// Make the room hold as many moths as it was asked for.
///
/// Surplus moths leave by ordinal, so the swarm is always ordinals `0..count` and taking a
/// thousand away twice running takes away the same thousand both times.
fn resize_swarm(
    mut commands: Commands,
    mut flock: ResMut<Flock>,
    mut rng: ResMut<SimRng>,
    moths: Query<(Entity, &Moth)>,
) {
    if flock.restock {
        for (entity, _) in &moths {
            commands.entity(entity).despawn();
        }
        flock.count = 0;
        flock.restock = false;
    }
    if flock.target > flock.count {
        let limit = ARENA / 2.0 - Vec2::splat(MARGIN);
        for ordinal in flock.count..flock.target {
            let seed = rng.u32();
            let speed = rng.range_f32(CRUISE.0..CRUISE.1);
            let wingspan = rng.range_f32(WINGSPAN.0..WINGSPAN.1);
            let heading = rng.range_f32(0.0..TAU);
            let at = vec2(
                rng.range_f32(-limit.x..limit.x),
                rng.range_f32(-limit.y..limit.y),
            );
            // Small moths beat faster, the way small moths do.
            let beat = WINGBEAT * (WINGSPAN.1 / wingspan) * 0.7;
            commands.spawn((
                Moth {
                    ordinal,
                    seed,
                    speed,
                    wingspan,
                    beat,
                    wing: rng.unit_f32(),
                    heading,
                    tone: rng.unit_f32(),
                },
                Transform2D {
                    translation: at,
                    rotation: heading,
                    scale: Vec2::ONE,
                },
            ));
        }
    } else if flock.target < flock.count {
        for (entity, moth) in &moths {
            if moth.ordinal >= flock.target {
                commands.entity(entity).despawn();
            }
        }
    }
    flock.count = flock.target;
}

/// Fly every moth one step: where it wants to turn, how far that takes it, and where it is in
/// its wingbeat.
fn fly(
    mut moths: Query<(&mut Moth, &mut Transform2D)>,
    step: Res<Step>,
    clock: Res<Clock>,
    lamp: Res<Lamp>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let limit = ARENA / 2.0;
    for (mut moth, mut transform) in &mut moths {
        let at = transform.translation;
        let mut turn = wobble(moth.seed, clock.0) * WANDER;

        // The lamp. A moth turns toward it but never slows down for it, so instead of piling
        // up it overshoots and comes back round — which is the whole reason moths circle
        // lights rather than landing on them.
        if lamp.on {
            let toward = lamp.at - at;
            let reach = 1.0 - (toward.length() / LAMP_REACH).min(1.0);
            if reach > 0.0 {
                let want = shortest_turn(moth.heading, toward.to_angle());
                turn += want.clamp(-1.0, 1.0) * LAMP_PULL * reach * reach;
            }
        }

        // The walls, felt a margin before they arrive: how far into the margin a moth has
        // pushed, per axis, pointing back toward the middle.
        let inset = limit - Vec2::splat(MARGIN);
        let over = vec2(
            (at.x.abs() - inset.x).max(0.0) / MARGIN * -at.x.signum(),
            (at.y.abs() - inset.y).max(0.0) / MARGIN * -at.y.signum(),
        );
        if over != Vec2::ZERO {
            let want = shortest_turn(moth.heading, over.to_angle());
            turn += want.clamp(-1.0, 1.0) * EDGE_PUSH * over.length().min(1.0);
        }

        moth.heading += turn * step.seconds;
        moth.wing = (moth.wing + moth.beat * step.seconds).fract();

        // Moths do not hold a line: each one slides across its own path over the wingbeat.
        let facing = Vec2::from_angle(moth.heading);
        let sway = facing.perp() * (moth.wing * TAU).sin() * FLUTTER;
        transform.translation =
            (at + (facing * moth.speed + sway) * step.seconds).clamp(-limit, limit);
        transform.rotation = moth.heading;
    }
}
