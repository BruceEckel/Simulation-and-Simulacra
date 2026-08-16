//! Popped: hot-air balloons full of cheerful animals, and a mouse pointer.
//!
//! The whole piece is a setup and a turn, and nearly every decision here is about timing one or
//! the other:
//!
//! **The setup has to be worth ruining.** Balloons rise slowly, sway on a breeze, and the
//! animals in the baskets wave at the pointer when it comes near ([`Animal::wave`]). They are
//! pleased to see you. Nothing rewards popping a balloon, and nothing asks you to; the piece
//! simply floats along being nice until you do something about it.
//!
//! **The turn needs a beat before it.** A popped balloon does not drop anybody. Every animal
//! gets [`BEAT`] seconds of [`Mood::Beat`] first, hanging in the air with an exclamation mark
//! over its head, looking at what just happened and then at you. The gag is entirely in that
//! pause: take it out and the animals merely fall, which is not funny.
//!
//! **Nobody is hurt, and the piece is careful about it.** Animals bounce, sit seeing stars,
//! then get up, dust themselves off, shake a fist in your direction and walk off
//! ([`Mood::Trudging`]). One in [`GRACEFUL_ODDS`] lands on its feet and takes a bow instead, and
//! one in [`CHUTE_ODDS`] is carrying a parachute, screams for a second, remembers it, and
//! descends smugly. The comedy is in the indignity, and indignity requires everybody to be fine.
//!
//! **The best jokes are the ones nobody wrote.** A falling animal pops any balloon it passes
//! through ([`chain_reactions`]), which is how a single click occasionally empties half the sky
//! and is funnier than anything that could be scripted.
//!
//! Pure logic, no sprites and no audio, so it runs headless for the determinism test. The binary
//! draws the animals and turns [`Noise`] into screaming.

use fulcrum::prelude::*;
use std::f32::consts::TAU;

/// The sky, in world units. A fixed arena: the window is only ever a view of it.
pub const ARENA: Vec2 = Vec2::new(1280.0, 800.0);
/// Where the ground is, and where anybody who falls ends up.
pub const GROUND: f32 = -300.0;

/// The most balloons in the sky at once.
pub const MAX_BALLOONS: u32 = 16;
/// Shortest and longest wait between one balloon and the next, in seconds.
pub const LAUNCH_GAP: (f32, f32) = (1.4, 3.2);
/// How much quicker they come after ten minutes of this, as a multiple.
pub const LAUNCH_HURRY: f32 = 0.55;
/// Seconds of play over which that hurrying happens.
pub const HURRY_OVER: f32 = 600.0;

/// Smallest and largest balloon, as a radius in world units.
pub const BALLOON_SIZE: (f32, f32) = (44.0, 74.0);
/// Slowest and fastest climb, in units per second. Gentle: the whole point is that they are in
/// no hurry whatsoever.
pub const CLIMB: (f32, f32) = (26.0, 46.0);
/// How far a balloon wanders sideways, in units per second.
pub const SWAY: f32 = 22.0;
/// How long the ropes are, as a multiple of the balloon's radius.
pub const ROPE: f32 = 1.35;
/// How far the basket lags behind the balloon when the wind pushes it, in seconds of travel.
pub const BASKET_SWING: f32 = 0.22;

/// How hard the breeze blows at its strongest, in units per second.
pub const BREEZE: f32 = 26.0;

/// Seconds an animal hangs in the air after its balloon goes, before gravity is allowed to
/// notice. The single most important number in the piece.
pub const BEAT: (f32, f32) = (0.42, 0.66);
/// Downward pull, in units per second squared. Cartoon gravity: slower than the real thing,
/// because a fall you can watch is funnier than a fall that is over.
pub const GRAVITY: f32 = 720.0;
/// How fast a falling animal can end up going, in units per second.
pub const TERMINAL: f32 = 540.0;
/// How much speed a bounce gives back.
pub const BOUNCE: f32 = 0.44;
/// Below this speed a landing is a landing rather than a bounce, in units per second.
pub const BOUNCE_FLOOR: f32 = 170.0;
/// The most times anybody bounces.
pub const MAX_BOUNCES: u32 = 2;
/// Seconds between one scream and the next breath.
pub const SCREAM_EVERY: f32 = 1.15;

/// One animal in this many is carrying a parachute.
pub const CHUTE_ODDS: f32 = 0.14;
/// Seconds of falling before somebody remembers they packed one.
pub const CHUTE_DELAY: (f32, f32) = (0.5, 1.1);
/// How fast a parachute comes down, in units per second.
pub const CHUTE_SPEED: f32 = 74.0;
/// One animal in this many lands on its feet and takes a bow.
pub const GRACEFUL_ODDS: f32 = 0.12;

/// Seconds spent sitting on the ground seeing stars.
pub const DAZED: (f32, f32) = (1.6, 2.8);
/// Seconds spent taking a bow.
pub const BOW: f32 = 1.6;
/// How fast an animal walks off, in units per second.
pub const TRUDGE: f32 = 78.0;

/// How near the pointer has to be before an animal waves at it, in units.
pub const NOTICE: f32 = 170.0;
/// How much of a click's slop is forgiven, as a multiple of the balloon's radius.
pub const CLICK_SLOP: f32 = 1.06;

/// Seconds a burst balloon skin flies around before it is done.
pub const SCRAP_LIFE: (f32, f32) = (1.1, 1.8);
/// How hard the air coming out of it throws it about.
pub const SCRAP_THRUST: f32 = 900.0;
/// How quickly that dies away, per second.
pub const SCRAP_DECAY: f32 = 1.9;
/// How fast a scrap changes its mind about which way it is going, in radians per second.
pub const SCRAP_WOBBLE: f32 = 15.0;

/// Slowest the piece can run, as a multiple of real time.
pub const SPEED_MIN: f32 = 0.15;
/// Fastest it can run.
pub const SPEED_MAX: f32 = 3.0;
/// How much a held speed key multiplies the rate each tick.
pub const SPEED_RAMP: f32 = 1.02;

/// How many coats of paint the binary has to offer. Colour by number, as everywhere else: the
/// simulation picks which one, and never what it looks like.
pub const COATS: u8 = 8;

/// Who is in the basket.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Species {
    /// Round, calm, slow to react.
    Bear,
    /// Ears you can see from the ground.
    Rabbit,
    /// Pointed ears and no dignity to lose.
    Cat,
    /// Small, round, delighted by everything.
    Pig,
    /// Bulging eyes, bounces best.
    Frog,
}

impl Species {
    /// All of them, for picking one.
    pub const ALL: [Species; 5] = [
        Species::Bear,
        Species::Rabbit,
        Species::Cat,
        Species::Pig,
        Species::Frog,
    ];

    /// How high this animal screams, as a playback rate. Big animals are funnier low.
    pub fn voice(self) -> f32 {
        match self {
            Species::Bear => 0.72,
            Species::Rabbit => 1.28,
            Species::Cat => 1.12,
            Species::Pig => 0.94,
            Species::Frog => 1.4,
        }
    }

    /// How well this animal bounces, as a multiple of [`BOUNCE`].
    pub fn springiness(self) -> f32 {
        match self {
            Species::Frog => 1.5,
            Species::Rabbit => 1.25,
            Species::Bear => 0.75,
            _ => 1.0,
        }
    }
}

/// What an animal is doing about its situation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    /// Enjoying the ride.
    Riding,
    /// Has noticed. Has not yet fallen.
    Beat,
    /// Falling, and making that clear.
    Falling,
    /// Descending under a parachute, with enormous dignity.
    Chuting,
    /// Sitting on the ground seeing stars.
    Dazed,
    /// Landed on its feet, and taking a moment about it.
    Bowing,
    /// Walking off, shaking a fist.
    Trudging,
}

impl Mood {
    /// Whether gravity is currently allowed to have an opinion.
    pub fn falls(self) -> bool {
        matches!(self, Mood::Falling)
    }

    /// Whether this animal is on the ground and sorting itself out.
    pub fn grounded(self) -> bool {
        matches!(self, Mood::Dazed | Mood::Bowing | Mood::Trudging)
    }
}

/// A balloon, rising.
#[derive(Component)]
pub struct Balloon {
    /// How big it is.
    pub radius: f32,
    /// Which coat of paint.
    pub coat: u8,
    /// How fast it climbs, in units per second.
    pub climb: f32,
    /// Where it is in its own sideways wander.
    pub sway: f32,
    /// How fast it wanders.
    pub sway_rate: f32,
}

/// The basket under a balloon, or falling without one.
#[derive(Component)]
pub struct Basket {
    /// The balloon holding it up, or `None` once there is nothing holding it up.
    pub holder: Option<Entity>,
    /// How far below the balloon it hangs.
    pub drop: f32,
    /// How wide it is.
    pub width: f32,
    /// Which way it is leaning, in radians.
    pub lean: f32,
}

/// One animal.
#[derive(Component)]
pub struct Animal {
    /// What it is.
    pub species: Species,
    /// Which coat of paint.
    pub coat: u8,
    /// How big it is, around 1.
    pub size: f32,
    /// What it is doing.
    pub mood: Mood,
    /// Seconds left in whatever that is.
    pub timer: f32,
    /// The basket it is riding in, while it is riding in one.
    pub seat: Option<Entity>,
    /// Where in the basket it sits.
    pub perch: Vec2,
    /// Where it is in its flailing.
    pub flail: f32,
    /// How much it is waving, `0..1`.
    pub wave: f32,
    /// How many times it has bounced.
    pub bounces: u32,
    /// Seconds until the next scream.
    pub scream_in: f32,
    /// Whether it packed a parachute, and how long until it remembers.
    pub chute: Option<f32>,
    /// Whether it is going to land on its feet.
    pub graceful: bool,
    /// Which way it will walk off.
    pub facing: f32,
    /// A fixed number in `0..1`, for the differences between one animal and the next.
    pub seed: f32,
}

impl Animal {
    /// How hard it is screaming, `0..1`. Used for the size of the mouth and the volume of the
    /// noise.
    pub fn terror(&self) -> f32 {
        match self.mood {
            Mood::Falling => 1.0,
            Mood::Beat => 0.35,
            _ => 0.0,
        }
    }
}

/// What is left of a popped balloon, going round the sky like a released party balloon.
#[derive(Component)]
pub struct Scrap {
    /// Which coat of paint it was.
    pub coat: u8,
    /// How big it was.
    pub radius: f32,
    /// Seconds lived.
    pub age: f32,
    /// Seconds it will last.
    pub life: f32,
    /// Which way the air is coming out.
    pub heading: f32,
    /// How much of it is left.
    pub thrust: f32,
}

/// Simulation velocity, units per second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// A noise the binary should make.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Voice {
    /// A balloon going.
    Pop,
    /// An animal, on the way down.
    Scream,
    /// An animal, arriving.
    Bonk,
    /// An animal, arriving and leaving again.
    Boing,
    /// A balloon skin flying round the sky.
    Raspberry,
    /// A balloon that got away, which is a good outcome and sounds like one.
    Chime,
}

/// One noise, at the moment it happens.
#[derive(Event, Clone, Copy, PartialEq, Debug)]
pub struct Noise {
    /// Which noise.
    pub voice: Voice,
    /// How loud, `0..1`.
    pub volume: f32,
    /// Where it is between the ears, `-1..1`.
    pub pan: f32,
    /// Playback rate.
    pub pitch: f32,
}

/// A puff of dust, for the binary to draw.
#[derive(Event, Clone, Copy, PartialEq, Debug)]
pub struct Puff {
    /// Where.
    pub at: Vec2,
    /// How big.
    pub size: f32,
}

/// Balloons that are about to stop being balloons, gathered during the tick and dealt with at
/// the end of it so that popping is one thing that happens in one place.
#[derive(Resource, Default, Clone, Debug)]
pub struct Departures {
    /// Balloons that have been popped, and what popped them.
    pub popped: Vec<(Entity, Blame)>,
    /// Balloons that made it off the top of the sky.
    pub escaped: Vec<Entity>,
}

/// Who is responsible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blame {
    /// You are.
    Pointer,
    /// A falling animal went through it, which is nobody's fault except still yours.
    Falling,
}

/// The running total, which is also the punchline.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tally {
    /// Balloons you have popped yourself.
    pub popped: u32,
    /// Balloons popped by somebody who was already falling.
    pub chained: u32,
    /// Balloons that got away.
    pub escaped: u32,
    /// Animals who have landed and walked it off.
    pub landed: u32,
    /// Animals who came down under a parachute.
    pub chuted: u32,
    /// Animals who stuck the landing.
    pub graceful: u32,
}

/// How many things are in the sky.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Census {
    /// Balloons.
    pub balloons: u32,
    /// Animals, riding or otherwise.
    pub animals: u32,
}

/// Seconds until the next balloon.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Launcher(pub f32);

impl Default for Launcher {
    fn default() -> Self {
        // One is already on its way up when you arrive.
        Self(0.6)
    }
}

/// Total seconds of this that have happened.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Elapsed(pub f32);

/// The breeze, in units per second, positive to the right.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Wind(pub f32);

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

/// Where a noise made at `at` sits between the ears.
pub fn stereo(at: Vec2) -> f32 {
    (at.x / (ARENA.x * 0.5)).clamp(-1.0, 1.0) * 0.8
}

/// Where the basket of a balloon hangs.
pub fn basket_at(balloon: Vec2, drop: f32, lean: f32) -> Vec2 {
    balloon + vec2(lean.sin(), -lean.cos()) * drop
}

/// Installs the sky.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Departures::default());
        app.world_mut().insert_resource(Tally::default());
        app.world_mut().insert_resource(Census::default());
        app.world_mut().insert_resource(Launcher::default());
        app.world_mut().insert_resource(Elapsed::default());
        app.world_mut().insert_resource(Wind::default());
        app.world_mut().insert_resource(Paused::default());
        app.world_mut().insert_resource(Speed::default());
        app.world_mut().insert_resource(Step::default());
        app.register_event::<Noise>();
        app.register_event::<Puff>();
        app.add_systems(
            FixedUpdate,
            (
                pace,
                set_step,
                advance_clock,
                launch_balloons,
                fly_balloons,
                carry_baskets,
                pop_by_pointer,
                chain_reactions,
                settle_departures,
                animal_life,
                fly_scraps,
            )
                .chain(),
        );
    }
}

/// Stillness and pace.
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

/// Advance the clock and turn the breeze.
fn advance_clock(mut elapsed: ResMut<Elapsed>, mut wind: ResMut<Wind>, step: Res<Step>) {
    if step.seconds <= 0.0 {
        return;
    }
    elapsed.0 += step.seconds;
    let slow = (elapsed.0 * 0.09).sin();
    let slower = (elapsed.0 * 0.031 + 2.1).sin();
    wind.0 = BREEZE * (slow * 0.65 + slower * 0.35);
}

/// Send up another balloon, with somebody in it.
fn launch_balloons(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut launcher: ResMut<Launcher>,
    mut census: ResMut<Census>,
    elapsed: Res<Elapsed>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    launcher.0 -= step.seconds;
    if launcher.0 > 0.0 || census.balloons >= MAX_BALLOONS {
        return;
    }
    // They come a little quicker as the day goes on, and then stop getting quicker.
    let hurry = 1.0 - (1.0 - LAUNCH_HURRY) * (elapsed.0 / HURRY_OVER).clamp(0.0, 1.0);
    launcher.0 = rng.range_f32(LAUNCH_GAP.0..LAUNCH_GAP.1) * hurry;

    let radius = rng.range_f32(BALLOON_SIZE.0..BALLOON_SIZE.1);
    let at = vec2(
        rng.range_f32(-ARENA.x * 0.42..ARENA.x * 0.42),
        -ARENA.y * 0.5 - radius * 2.0,
    );
    let balloon = commands
        .spawn((
            Balloon {
                radius,
                coat: rng.range_i32(0..COATS as i32) as u8,
                climb: rng.range_f32(CLIMB.0..CLIMB.1),
                sway: rng.range_f32(0.0..TAU),
                sway_rate: rng.range_f32(0.25..0.6),
            },
            Transform2D::from_translation(at),
            Velocity(vec2(0.0, rng.range_f32(CLIMB.0..CLIMB.1))),
        ))
        .id();
    census.balloons += 1;

    let drop = radius * ROPE + 42.0;
    let width = radius * 0.95;
    let basket = commands
        .spawn((
            Basket {
                holder: Some(balloon),
                drop,
                width,
                lean: 0.0,
            },
            Transform2D::from_translation(at - vec2(0.0, drop)),
            Velocity(Vec2::ZERO),
        ))
        .id();

    // One passenger usually, sometimes a pair, occasionally a whole outing.
    let riders = if rng.chance(0.55) {
        1
    } else if rng.chance(0.75) {
        2
    } else {
        3
    };
    for index in 0..riders {
        let spread = if riders == 1 {
            0.0
        } else {
            (index as f32 / (riders - 1) as f32 - 0.5) * width * 1.1
        };
        let species = Species::ALL[rng.range_i32(0..Species::ALL.len() as i32) as usize];
        commands.spawn((
            Animal {
                species,
                coat: rng.range_i32(0..COATS as i32) as u8,
                size: rng.range_f32(0.85..1.2) * if riders > 2 { 0.85 } else { 1.0 },
                mood: Mood::Riding,
                timer: 0.0,
                seat: Some(basket),
                perch: vec2(spread, 12.0),
                flail: rng.range_f32(0.0..TAU),
                wave: 0.0,
                bounces: 0,
                scream_in: 0.0,
                chute: rng
                    .chance(CHUTE_ODDS)
                    .then(|| rng.range_f32(CHUTE_DELAY.0..CHUTE_DELAY.1)),
                graceful: rng.chance(GRACEFUL_ODDS),
                facing: if rng.chance(0.5) { 1.0 } else { -1.0 },
                seed: rng.unit_f32(),
            },
            Transform2D::from_translation(at - vec2(0.0, drop) + vec2(spread, 12.0)),
            Velocity(Vec2::ZERO),
        ));
        census.animals += 1;
    }
}

/// Carry the balloons up.
fn fly_balloons(
    mut balloons: Query<(&mut Balloon, &mut Transform2D, &mut Velocity)>,
    wind: Res<Wind>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    for (mut balloon, mut transform, mut velocity) in &mut balloons {
        balloon.sway += balloon.sway_rate * dt;
        // A balloon has no engine and no rudder: it goes up, and it goes wherever the air is
        // going, which is what makes it such a peaceful thing to watch.
        velocity.0 = vec2(
            wind.0 + balloon.sway.sin() * SWAY,
            balloon.climb + (balloon.sway * 0.7).cos() * 3.0,
        );
        transform.translation += velocity.0 * dt;
    }
}

/// Hang the baskets under their balloons, and let the loose ones fall.
#[allow(clippy::type_complexity)] // standard ECS system shape
fn carry_baskets(
    mut baskets: Query<(&mut Basket, &mut Transform2D, &mut Velocity)>,
    balloons: Query<(&Transform2D, &Velocity), (With<Balloon>, Without<Basket>)>,
    mut departures: ResMut<Departures>,
    wind: Res<Wind>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    let ceiling = ARENA.y * 0.5;
    for (mut basket, mut transform, mut velocity) in &mut baskets {
        match basket.holder.and_then(|holder| {
            balloons
                .get(holder)
                .ok()
                .map(|(transform, velocity)| (transform, velocity, holder))
        }) {
            Some((above, drift, holder)) => {
                // The basket trails behind the balloon's sideways travel, which is the pendulum
                // swing that makes a rig read as hanging rather than as one drawn object.
                let wanted = (-drift.0.x * BASKET_SWING / basket.drop.max(1.0)).clamp(-0.4, 0.4);
                basket.lean += (wanted - basket.lean) * (1.0 - (-3.0 * dt).exp());
                let at = basket_at(above.translation, basket.drop, basket.lean);
                velocity.0 = (at - transform.translation) / dt.max(1e-4);
                transform.translation = at;
                // The rig has got away once the basket is over the top, which is the moment
                // the last of it leaves the screen.
                if at.y > ceiling + 40.0 {
                    departures.escaped.push(holder);
                }
            }
            None => {
                velocity.0.y -= GRAVITY * dt;
                velocity.0.x += wind.0 * 0.4 * dt;
                basket.lean += velocity.0.x * 0.0004;
                transform.translation += velocity.0 * dt;
            }
        }
    }
}

/// Click a balloon, pop a balloon.
fn pop_by_pointer(
    balloons: Query<(Entity, &Balloon, &Transform2D)>,
    mut departures: ResMut<Departures>,
    input: Res<Input>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 || !input.mouse_just_pressed(MouseButton::Left) {
        return;
    }
    let pointer = input.mouse_world();
    // The nearest one under the pointer, so overlapping balloons pop in the order you would
    // expect rather than in whatever order the world happens to be stored in.
    let mut best: Option<(Entity, f32)> = None;
    for (entity, balloon, transform) in &balloons {
        let reach = balloon.radius * CLICK_SLOP;
        let distance = (pointer - transform.translation).length();
        if distance <= reach && best.is_none_or(|(_, nearest)| distance < nearest) {
            best = Some((entity, distance));
        }
    }
    if let Some((entity, _)) = best {
        departures.popped.push((entity, Blame::Pointer));
    }
}

/// Somebody on the way down goes through somebody else's balloon.
fn chain_reactions(
    fallers: Query<(&Animal, &Transform2D)>,
    balloons: Query<(Entity, &Balloon, &Transform2D)>,
    mut departures: ResMut<Departures>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    for (animal, falling) in &fallers {
        if !matches!(animal.mood, Mood::Falling | Mood::Chuting) {
            continue;
        }
        for (entity, balloon, transform) in &balloons {
            let reach = balloon.radius + 14.0 * animal.size;
            if (falling.translation - transform.translation).length_squared() < reach * reach
                && !departures.popped.iter().any(|(had, _)| *had == entity)
            {
                departures.popped.push((entity, Blame::Falling));
            }
        }
    }
}

/// Deal with every balloon that has stopped being one.
#[expect(clippy::too_many_arguments, reason = "a pop touches the whole sky")]
fn settle_departures(
    mut commands: Commands,
    mut departures: ResMut<Departures>,
    mut rng: ResMut<SimRng>,
    mut tally: ResMut<Tally>,
    mut census: ResMut<Census>,
    mut noises: EventWriter<Noise>,
    balloons: Query<(&Balloon, &Transform2D)>,
    mut baskets: Query<(Entity, &mut Basket, &Transform2D)>,
    mut animals: Query<(Entity, &mut Animal)>,
) {
    let popped = std::mem::take(&mut departures.popped);
    let escaped = std::mem::take(&mut departures.escaped);

    for (entity, blame) in popped {
        let Ok((balloon, transform)) = balloons.get(entity) else {
            continue;
        };
        let at = transform.translation;
        noises.write(Noise {
            voice: Voice::Pop,
            volume: 0.85,
            pan: stereo(at),
            // Small balloons pop higher, which is the only sound cue for a balloon's size.
            pitch: (1.5 - balloon.radius / 90.0).clamp(0.7, 1.5),
        });
        noises.write(Noise {
            voice: Voice::Raspberry,
            volume: 0.5,
            pan: stereo(at),
            pitch: rng.range_f32(0.85..1.2),
        });
        commands.spawn((
            Scrap {
                coat: balloon.coat,
                radius: balloon.radius,
                age: 0.0,
                life: rng.range_f32(SCRAP_LIFE.0..SCRAP_LIFE.1),
                heading: rng.range_f32(0.0..TAU),
                thrust: 1.0,
            },
            Transform2D::from_translation(at),
            Velocity(Vec2::ZERO),
        ));
        commands.entity(entity).despawn();
        census.balloons = census.balloons.saturating_sub(1);
        match blame {
            Blame::Pointer => tally.popped += 1,
            Blame::Falling => tally.chained += 1,
        }

        // Cut the basket loose, and give everybody in it their moment.
        for (basket_entity, mut basket, _) in &mut baskets {
            if basket.holder != Some(entity) {
                continue;
            }
            basket.holder = None;
            for (_, mut animal) in &mut animals {
                if animal.seat != Some(basket_entity) {
                    continue;
                }
                animal.seat = None;
                animal.mood = Mood::Beat;
                animal.timer = rng.range_f32(BEAT.0..BEAT.1);
            }
        }
    }

    for entity in escaped {
        if balloons.get(entity).is_err() {
            continue;
        }
        noises.write(Noise {
            voice: Voice::Chime,
            volume: 0.35,
            pan: 0.0,
            pitch: rng.range_f32(0.95..1.35),
        });
        commands.entity(entity).despawn();
        census.balloons = census.balloons.saturating_sub(1);
        tally.escaped += 1;
        for (basket_entity, basket, _) in &baskets {
            if basket.holder != Some(entity) {
                continue;
            }
            commands.entity(basket_entity).despawn();
            for (animal_entity, animal) in &animals {
                if animal.seat == Some(basket_entity) {
                    commands.entity(animal_entity).despawn();
                    census.animals = census.animals.saturating_sub(1);
                }
            }
        }
    }
}

/// Everything an animal does, from waving at you to walking off.
#[expect(clippy::too_many_arguments, reason = "one animal has a whole life")]
fn animal_life(
    mut commands: Commands,
    mut animals: Query<(Entity, &mut Animal, &mut Transform2D, &mut Velocity)>,
    baskets: Query<(&Basket, &Transform2D), Without<Animal>>,
    mut rng: ResMut<SimRng>,
    mut tally: ResMut<Tally>,
    mut census: ResMut<Census>,
    mut noises: EventWriter<Noise>,
    mut puffs: EventWriter<Puff>,
    wind: Res<Wind>,
    input: Res<Input>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    let pointer = input.mouse_world();

    for (entity, mut animal, mut transform, mut velocity) in &mut animals {
        animal.timer -= dt;
        let at = transform.translation;

        match animal.mood {
            Mood::Riding => {
                let Some((basket, seat)) = animal
                    .seat
                    .and_then(|seat| baskets.get(seat).ok())
                    .map(|(basket, transform)| (basket, transform.translation))
                else {
                    continue;
                };
                let (sin, cos) = basket.lean.sin_cos();
                let perch = vec2(
                    animal.perch.x * cos - animal.perch.y * sin,
                    animal.perch.x * sin + animal.perch.y * cos,
                );
                // A small bob, so nobody is ever completely still.
                let bob = (animal.flail).sin() * 1.4;
                animal.flail += dt * 2.2;
                transform.translation = seat + perch + vec2(0.0, bob);
                velocity.0 = Vec2::ZERO;

                // They are pleased to see you. This is load-bearing.
                let near = (pointer - transform.translation).length() < NOTICE;
                let wanted = if near { 1.0 } else { 0.0 };
                animal.wave += (wanted - animal.wave) * (1.0 - (-6.0 * dt).exp());
            }
            Mood::Beat => {
                // Hanging in the air, having noticed. Gravity is not consulted.
                velocity.0 = Vec2::ZERO;
                animal.wave *= 1.0 - (1.0 - (-4.0 * dt).exp());
                if animal.timer <= 0.0 {
                    animal.mood = Mood::Falling;
                    animal.scream_in = 0.0;
                    velocity.0 = vec2(rng.range_f32(-40.0..40.0), 0.0);
                }
            }
            Mood::Falling => {
                velocity.0.y = (velocity.0.y - GRAVITY * dt).max(-TERMINAL);
                velocity.0.x += wind.0 * 0.5 * dt;
                transform.translation += velocity.0 * dt;
                animal.flail += dt * 26.0;

                animal.scream_in -= dt;
                if animal.scream_in <= 0.0 {
                    animal.scream_in = SCREAM_EVERY;
                    noises.write(Noise {
                        voice: Voice::Scream,
                        volume: 0.8,
                        pan: stereo(transform.translation),
                        pitch: animal.species.voice() * (1.25 - 0.35 * animal.size),
                    });
                }

                // The parachute, remembered mid-scream, which is the best possible moment.
                // The timer went negative when the beat ran out, so how long this has been
                // going on is simply how far past zero it is.
                if let Some(delay) = animal.chute
                    && animal.timer <= -delay
                {
                    animal.mood = Mood::Chuting;
                    animal.chute = None;
                    animal.timer = 0.0;
                    tally.chuted += 1;
                    noises.write(Noise {
                        voice: Voice::Boing,
                        volume: 0.4,
                        pan: stereo(transform.translation),
                        pitch: 1.3,
                    });
                }

                if transform.translation.y <= GROUND {
                    let hit = velocity.0.y.abs();
                    transform.translation.y = GROUND;
                    puffs.write(Puff {
                        at: vec2(transform.translation.x, GROUND),
                        size: (hit / 260.0).clamp(0.5, 2.0) * animal.size,
                    });
                    let springy = BOUNCE * animal.species.springiness();
                    if hit > BOUNCE_FLOOR && animal.bounces < MAX_BOUNCES {
                        animal.bounces += 1;
                        velocity.0.y = hit * springy;
                        velocity.0.x *= 0.65;
                        noises.write(Noise {
                            voice: Voice::Boing,
                            volume: 0.6,
                            pan: stereo(transform.translation),
                            pitch: (1.4 - 0.3 * animal.size) * animal.species.voice().max(0.8),
                        });
                    } else {
                        land(
                            &mut animal,
                            &mut velocity,
                            transform.translation,
                            &mut rng,
                            &mut tally,
                            &mut noises,
                        );
                    }
                }
            }
            Mood::Chuting => {
                // Down at a walking pace, with the air of somebody who planned this.
                let wanted = vec2(wind.0 * 0.9, -CHUTE_SPEED);
                let carried = velocity.0;
                velocity.0 += (wanted - carried) * (1.0 - (-4.0 * dt).exp());
                transform.translation += velocity.0 * dt;
                animal.flail += dt * 1.5;
                if transform.translation.y <= GROUND {
                    transform.translation.y = GROUND;
                    velocity.0 = Vec2::ZERO;
                    animal.mood = Mood::Trudging;
                    animal.timer = 0.0;
                    tally.landed += 1;
                }
            }
            Mood::Dazed => {
                velocity.0 = Vec2::ZERO;
                animal.flail += dt * 3.0;
                if animal.timer <= 0.0 {
                    animal.mood = Mood::Trudging;
                }
            }
            Mood::Bowing => {
                velocity.0 = Vec2::ZERO;
                animal.flail += dt * 2.0;
                if animal.timer <= 0.0 {
                    animal.mood = Mood::Trudging;
                }
            }
            Mood::Trudging => {
                velocity.0 = vec2(animal.facing * TRUDGE, 0.0);
                transform.translation += velocity.0 * dt;
                transform.translation.y = GROUND;
                animal.flail += dt * 9.0;
                if transform.translation.x.abs() > ARENA.x * 0.5 + 60.0 {
                    commands.entity(entity).despawn();
                    census.animals = census.animals.saturating_sub(1);
                }
            }
        }

        // Anybody who somehow ends up a long way outside the sky is quietly excused.
        if at.y < GROUND - 400.0 || at.x.abs() > ARENA.x * 0.5 + 400.0 {
            commands.entity(entity).despawn();
            census.animals = census.animals.saturating_sub(1);
        }
    }
}

/// Arriving: either seeing stars, or standing up as if that was the plan all along.
fn land(
    animal: &mut Animal,
    velocity: &mut Velocity,
    at: Vec2,
    rng: &mut SimRng,
    tally: &mut Tally,
    noises: &mut EventWriter<Noise>,
) {
    velocity.0 = Vec2::ZERO;
    tally.landed += 1;
    if animal.graceful {
        animal.mood = Mood::Bowing;
        animal.timer = BOW;
        tally.graceful += 1;
    } else {
        animal.mood = Mood::Dazed;
        animal.timer = rng.range_f32(DAZED.0..DAZED.1);
    }
    noises.write(Noise {
        voice: Voice::Bonk,
        volume: 0.7,
        pan: stereo(at),
        pitch: (1.3 - 0.4 * animal.size).clamp(0.7, 1.4),
    });
}

/// The skin of a popped balloon, going round the sky until the air runs out.
fn fly_scraps(
    mut commands: Commands,
    mut scraps: Query<(Entity, &mut Scrap, &mut Transform2D, &mut Velocity)>,
    mut rng: ResMut<SimRng>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    for (entity, mut scrap, mut transform, mut velocity) in &mut scraps {
        scrap.age += dt;
        if scrap.age >= scrap.life {
            commands.entity(entity).despawn();
            continue;
        }
        // A released balloon does not fly anywhere in particular, at speed.
        scrap.heading += rng.range_f32(-SCRAP_WOBBLE..SCRAP_WOBBLE) * dt;
        scrap.thrust *= (-SCRAP_DECAY * dt).exp();
        let push = vec2(scrap.heading.cos(), scrap.heading.sin()) * SCRAP_THRUST * scrap.thrust;
        let carried = velocity.0;
        velocity.0 += (push - carried) * (1.0 - (-6.0 * dt).exp());
        velocity.0.y -= GRAVITY * 0.35 * dt;
        transform.translation += velocity.0 * dt;
        transform.rotation = velocity.0.to_angle();
    }
}
