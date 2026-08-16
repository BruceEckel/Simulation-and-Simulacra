//! Lullaby: a field of light that cools until it stops, dims until it is gone, and goes on
//! breathing in the dark.
//!
//! This is not a piece to watch for as long as you like. It is a piece with an arc and an end.
//! Everything below is in the service of that arc, and four decisions do the work:
//!
//! **It is a cooling simulation, and the cooling is the whole story.** Every star is a damped
//! spring tied to a resting place of its own, shaken by a random force. The size of that force
//! is a temperature, and the temperature falls to exactly zero part way through the night. Early
//! on the shaking dominates and the field is a warm haze in constant motion; as the temperature
//! drops, each star's wandering tightens around its place until the sky is perfectly still. The
//! stillness is arrived at rather than imposed. Nothing is faded out while it is still moving,
//! because a moving thing you can no longer quite see is exactly what keeps a tired eye hunting.
//! See [`settle`].
//!
//! **The breath lengthens.** It begins at ten seconds, which is six breaths a minute, and
//! stretches to sixteen, which is under four. The draw stays about where it is and the release
//! takes all of the extra time: [`BREATH_INHALE`] falls from just under half the cycle to under
//! a third. A long release is the half that does the settling, and a rate that is *descending*
//! is worth more here than any particular rate held steady.
//!
//! **Nothing in here can change quickly.** [`Depth`] (how far into the night the piece has
//! travelled) never moves faster than [`DEPTH_RATE`], whatever you do to it. Restart it, shorten
//! the night, ask for time back: the schedule jumps, and the piece still takes two and a half
//! minutes to travel from one end to the other. There is no input, and no accident, that can
//! produce a sudden change in brightness or motion. [`nothing_changes_suddenly`] in the tests is
//! the real statement of it.
//!
//! **The picture is not the deliverable.** You cannot watch a screen with your eyes closed, and
//! a sleep aid you have to look at is a sleep aid you have to stay awake for. So the light is
//! spent first: it is gone by [`LIGHT_OUT`], well before the night is over. What continues is
//! the voice (see [`voice_level`]), breathing slower and lower into a dark room, until that too
//! is gone and the machine is simply a black rectangle making no sound.
//!
//! Pure logic. No sprites, no color, no audio, so it runs headless for the tests, and so a whole
//! twenty-five minute night takes about a second to simulate.
//!
//! The field resizes with the window through [`FIELD_COMMAND`] on the replayable command
//! channel, never by reading renderer state.

use fulcrum::prelude::*;
use std::f32::consts::TAU;

/// Field size at startup, and the area every resize preserves.
pub const DEFAULT_FIELD: Vec2 = Vec2::new(1024.0, 768.0);
/// Aspect-ratio limits for a resize.
pub const ASPECT_LIMITS: (f32, f32) = (0.4, 3.2);
/// Name of the resize command on the replayable command channel.
pub const FIELD_COMMAND: &str = "field";

/// How many stars. Enough that the haze at the beginning reads as one body of light rather than
/// as countable dots, and that the still sky at the end has something to it.
pub const STARS: u32 = 3200;

/// How long a night runs by default, in seconds. Twenty-five minutes: long enough that the last
/// stretch is spent in the dark with only the breath, short enough that the whole arc happens
/// while you are still in bed rather than at some point in the small hours.
pub const DEFAULT_NIGHT: f32 = 25.0 * 60.0;
/// The step the number keys move the night by.
pub const NIGHT_STEP: f32 = 5.0 * 60.0;
/// Shortest and longest night, which is `1` and `9` on the number row.
pub const NIGHT_LIMITS: (f32, f32) = (NIGHT_STEP, 9.0 * NIGHT_STEP);
/// Time handed back by a press of "still awake".
pub const REPRIEVE: f32 = 4.0 * 60.0;

/// The fastest [`Depth`] may travel, per second. The reciprocal is the floor on how long a full
/// journey from wide awake to dark can take: two and a half minutes, no matter what is asked of
/// it. This single number is what makes every control in the piece safe.
pub const DEPTH_RATE: f32 = 1.0 / 150.0;

/// The depth at which the shaking stops entirely and the sky is still.
pub const SETTLED: f32 = 0.62;
/// Root-mean-square speed of a star at the top of the night, in units per second. It falls to
/// zero at [`SETTLED`].
pub const JITTER_SPEED: f32 = 44.0;
/// How hard a star is drawn to its resting place, at the top of the night and at the bottom.
/// Weak to begin with, so a star wanders a long way from its place and the sky is a haze; strong
/// at the end, so it sits exactly on it.
pub const HOME_PULL: (f32, f32) = (0.35, 5.5);
/// How much the medium resists, over the same span. It thickens as the pull grows, which keeps
/// the settling overdamped: a star drifts home rather than swinging past it.
pub const DRAG: (f32, f32) = (1.6, 5.0);
/// Below this speed, and this distance from home, a star at zero temperature is simply put to
/// rest. A damped spring only approaches its rest point, and "almost still" is not the thing
/// being aimed at here. Both are far under a pixel at any sane window size.
pub const STILL_SPEED: f32 = 0.05;
pub const STILL_DISTANCE: f32 = 0.25;

/// One breath at the top of the night and at the bottom, in seconds: six a minute down to under
/// four.
pub const BREATH_PERIOD: (f32, f32) = (10.0, 16.0);
/// The share of the breath spent drawing in, over the same span. It shrinks, so in absolute
/// terms the draw stays near four and a half seconds throughout and the whole of the added time
/// goes into the release.
pub const BREATH_INHALE: (f32, f32) = (0.45, 0.30);
/// How much the breath swells a star's wandering. Small: the breath is meant to be felt in the
/// light rather than watched as an event.
pub const BREATH_SWELL: f32 = 0.10;

/// The depth at which the light starts to go, and the depth at which it is gone. Nothing is
/// drawn at all past [`LIGHT_OUT`], so the window is honestly black rather than nearly black.
pub const LIGHT_FULL: f32 = 0.34;
pub const LIGHT_OUT: f32 = 0.84;
/// The band of depths within which individual stars begin to go out, and how long each takes.
/// Staggering them means the sky thins unevenly, the way a sky actually clouds over, instead of
/// every star dimming in step like a lamp on a dimmer.
///
/// The last star has to be out by [`LIGHT_OUT`], where the ceiling reaches zero, so that the end
/// of the light is the last few stars going by themselves rather than the ceiling taking whatever
/// is left. Both are smooth, so the difference is not dramatic; it is the difference between the
/// sky emptying and the sky being switched off.
pub const STAR_DIM: (f32, f32) = (0.30, 0.68);
pub const STAR_FADE: f32 = 0.14;

/// Seconds the voice takes to come up at the start. Nobody wants a noise to begin at full
/// strength in a dark room.
pub const VOICE_IN: f32 = 45.0;
/// The depth at which the voice starts to go. Well past [`LIGHT_OUT`]: the last stretch of the
/// night is sound alone, in the dark, which is the part that is actually of any use once your
/// eyes are shut.
pub const VOICE_HOLD: f32 = 0.86;

/// How the resting places are laid out: a soft band across the sky, tilted, with a scattering
/// everywhere else. A perfectly even sky is a texture; a sky with a band in it is a place.
pub const BAND_TILT: f32 = 0.42;
pub const BAND_WIDTH: f32 = 0.16;
pub const BAND_FLOOR: f32 = 0.30;

/// One star.
#[derive(Component)]
pub struct Star {
    /// Where this star comes to rest. Chosen once, at the start of the night.
    pub home: Vec2,
    /// Where it sits in the palette, `0..1`. Fixed, so a star keeps its color all night.
    pub warmth: f32,
    /// How big it is, `0..1`, weighted so most are small and a few are not.
    pub size: f32,
    /// A multiplier on this star's pull home, so the sky does not settle all at once.
    pub lag: f32,
    /// The depth at which this star begins to go out.
    pub dim_at: f32,
}

/// Simulation velocity, units per second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// The box the sky fills. Simulation state, changed only by [`FIELD_COMMAND`].
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Field(pub Vec2);

impl Default for Field {
    fn default() -> Self {
        Self(DEFAULT_FIELD)
    }
}

/// How long this night is asked to be, in seconds.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Night(pub f32);

impl Default for Night {
    fn default() -> Self {
        Self(DEFAULT_NIGHT)
    }
}

/// How far into the night the piece has actually travelled: `0` wide awake, `1` dark and silent.
///
/// [`Depth::wanted`] is where the schedule says it should be and can jump about as freely as the
/// controls allow. [`Depth::now`] is where it is, and it only ever walks; see [`DEPTH_RATE`].
/// Everything visible and audible is a function of `now`, which is why nothing in the piece can
/// change abruptly.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Depth {
    /// Seconds since the night began.
    pub elapsed: f32,
    /// Where the schedule says the piece should be.
    pub wanted: f32,
    /// Where it is.
    pub now: f32,
}

/// Where the breath is.
///
/// Held as a position in the cycle rather than as seconds, so that lengthening the breath changes
/// only how fast the position advances. Kept as seconds it would jump the moment the period
/// changed, and the piece spends the entire night changing the period.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Breath {
    /// Position in the current breath, `0..1`. `0` is the bottom of the release.
    pub cycle: f32,
    /// `0` empty, `1` full. What the light follows.
    pub phase: f32,
    /// How long this breath is, in seconds.
    pub period: f32,
    /// The share of it spent drawing in.
    pub inhale: f32,
    /// How many draws and how many releases have begun since the night started.
    ///
    /// Counters rather than a flag set on the tick it happens. The voice answers a *change* in
    /// these, which means it cannot answer one breath twice or miss one whatever order the
    /// systems happen to run in. A flag would have to be read after it was written and before it
    /// was cleared, and getting that wrong is worth either a doubled breath at the start or a
    /// silent one in the middle, both of which are precisely the sort of small wrong event this
    /// piece cannot afford.
    pub draws: u32,
    pub releases: u32,
}

impl Default for Breath {
    fn default() -> Self {
        Self {
            cycle: 0.0,
            phase: 0.0,
            period: BREATH_PERIOD.0,
            inhale: BREATH_INHALE.0,
            draws: 0,
            releases: 0,
        }
    }
}

/// Whether the first tick has been taken, so the opening draw fires exactly once.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Began(pub bool);

/// Smoothstep. Every fade, ramp and turn in the piece goes through this: a linear fade has a
/// visible corner at each end, and a corner is a small event.
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Eased travel from `1` at `from` to `0` at `to`, and held flat outside them.
pub fn ramp_down(x: f32, from: f32, to: f32) -> f32 {
    if to <= from {
        return if x >= to { 0.0 } else { 1.0 };
    }
    1.0 - ease((x - from) / (to - from))
}

/// Eased travel from `0` at `from` to `1` at `to`.
pub fn ramp_up(x: f32, from: f32, to: f32) -> f32 {
    if to <= from {
        return if x >= to { 1.0 } else { 0.0 };
    }
    ease((x - from) / (to - from))
}

/// Blend between two ends by an eased depth.
pub fn over_night(depth: f32, ends: (f32, f32)) -> f32 {
    ends.0 + (ends.1 - ends.0) * ease(depth)
}

/// Where the schedule says the night has got to.
pub fn wanted_depth(elapsed: f32, night: f32) -> f32 {
    (elapsed / night.max(1.0)).clamp(0.0, 1.0)
}

/// Walk `current` toward `target` at no more than `rate` per second.
///
/// The one place a jump can be introduced into this piece is a control, and every control writes
/// the schedule rather than the state. This is the valve between them.
pub fn approach(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    let step = rate * dt;
    if target > current {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

/// Root-mean-square speed of a star at this depth. Squared, this is the temperature.
///
/// Falls as the square of the distance left to [`SETTLED`] rather than linearly, so most of the
/// cooling happens early and the last part of it is a long, slow arrival at nothing.
pub fn jitter(depth: f32) -> f32 {
    let left = ((SETTLED - depth) / SETTLED).clamp(0.0, 1.0);
    JITTER_SPEED * left * left
}

/// How hard stars are drawn home at this depth.
pub fn home_pull(depth: f32) -> f32 {
    over_night(depth, HOME_PULL)
}

/// How much the medium resists at this depth.
pub fn drag(depth: f32) -> f32 {
    over_night(depth, DRAG)
}

/// How long a breath is at this depth.
pub fn breath_period(depth: f32) -> f32 {
    over_night(depth, BREATH_PERIOD)
}

/// The share of a breath spent drawing in, at this depth.
pub fn inhale_fraction(depth: f32) -> f32 {
    over_night(depth, BREATH_INHALE)
}

/// Where the breath is, from a position in the cycle: `0` empty, `1` full.
///
/// Smoothstepped at both turns, so there is no moment at which the direction of the motion
/// changes sharply and no instant to be caught by.
pub fn breath_phase(cycle: f32, inhale: f32) -> f32 {
    let cycle = cycle.rem_euclid(1.0);
    let inhale = inhale.clamp(0.05, 0.95);
    if cycle < inhale {
        ease(cycle / inhale)
    } else {
        1.0 - ease((cycle - inhale) / (1.0 - inhale))
    }
}

/// The ceiling on how bright anything may be, at this depth. Exactly zero from [`LIGHT_OUT`] on.
pub fn luminance(depth: f32) -> f32 {
    ramp_down(depth, LIGHT_FULL, LIGHT_OUT)
}

/// How present one star is: it goes out at its own depth, within the band the sky thins across.
pub fn star_presence(dim_at: f32, depth: f32) -> f32 {
    ramp_down(depth, dim_at, dim_at + STAR_FADE)
}

/// How loud the voice is: up over the first minute, held through the dark, and gone by the end.
pub fn voice_level(depth: f32, elapsed: f32) -> f32 {
    ramp_up(elapsed, 0.0, VOICE_IN) * ramp_down(depth, VOICE_HOLD, 1.0)
}

/// How thick the sky is at a point, `0..1`: a tilted band with a scattering everywhere else.
pub fn sky_density(point: Vec2, field: Vec2) -> f32 {
    let unit = vec2(point.x / field.x.max(1.0), point.y / field.y.max(1.0));
    let across = unit.y - unit.x * BAND_TILT;
    let band = (-(across / BAND_WIDTH) * (across / BAND_WIDTH) * 0.5).exp();
    BAND_FLOOR + (1.0 - BAND_FLOOR) * band
}

/// The field a window of this pixel size should get: the window's aspect at [`DEFAULT_FIELD`]'s
/// area, so the density of the sky stays put while its shape follows the window.
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

/// Two independent standard normals, by Box-Muller.
///
/// The kick has to be gaussian rather than merely random: the whole behaviour of a star, the size
/// of the cloud it wanders in and the way that cloud shrinks as the night cools, is the standard
/// result for a spring shaken by gaussian noise. Shake it with anything else and the numbers in
/// this file stop predicting what you see.
fn gaussian(rng: &mut SimRng) -> Vec2 {
    let unit = rng.range_f32(1e-7..1.0);
    let angle = rng.range_f32(0.0..TAU);
    let radius = (-2.0 * unit.ln()).sqrt();
    vec2(radius * angle.cos(), radius * angle.sin())
}

/// Installs the piece.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Field::default());
        app.world_mut().insert_resource(Night::default());
        app.world_mut().insert_resource(Depth::default());
        app.world_mut().insert_resource(Breath::default());
        app.world_mut().insert_resource(Began::default());
        app.add_systems(Startup, fill_sky);
        app.add_systems(
            FixedUpdate,
            (apply_field, controls, descend, breathe, settle).chain(),
        );
    }
}

/// Hang the sky. Public so the binary can order its sprite-attachment after it.
pub fn fill_sky(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    field: Res<Field>,
    depth: Res<Depth>,
) {
    for _ in 0..STARS {
        let home = pick_home(&mut rng, field.0);
        // Start each star already scattered around its place by roughly as far as it would
        // wander there anyway, so the opening is a haze and not a constellation coming apart.
        let spread = jitter(depth.now) / home_pull(depth.now).sqrt();
        commands.spawn((
            Star {
                home,
                warmth: rng.range_f32(0.0..1.0),
                // Cubed, so most stars are small and a handful are not. An evenly sized field
                // reads as a texture; an unevenly sized one reads as distance.
                size: rng.range_f32(0.0..1.0).powi(3),
                lag: rng.range_f32(0.7..1.4),
                dim_at: rng.range_f32(STAR_DIM.0..STAR_DIM.1),
            },
            Transform2D::from_translation(home + gaussian(&mut rng) * spread),
            Velocity(gaussian(&mut rng) * jitter(depth.now)),
        ));
    }
}

/// A resting place, drawn from [`sky_density`] by rejection.
///
/// Bounded rather than looping until it succeeds: an unbounded loop here would make the number of
/// RNG draws depend on luck, and every draw after it would shift. A handful of stars land against
/// the density on the last try, which is invisible and deterministic.
fn pick_home(rng: &mut SimRng, field: Vec2) -> Vec2 {
    let limit = field / 2.0;
    let mut point = Vec2::ZERO;
    for _ in 0..16 {
        point = vec2(
            rng.range_f32(-limit.x..limit.x),
            rng.range_f32(-limit.y..limit.y),
        );
        if rng.range_f32(0.0..1.0) < sky_density(point, field) {
            break;
        }
    }
    point
}

/// Move the boundary when a resize arrives, and take the whole sky with it.
///
/// Stretched rather than clipped: the resting places are the picture, and a picture that loses
/// its edges when the window narrows is a picture that punishes you for touching the window.
fn apply_field(
    mut field: ResMut<Field>,
    mut orders: EventReader<CommandEvent>,
    mut stars: Query<(&mut Star, &mut Transform2D)>,
) {
    let mut wanted = None;
    for order in orders.read() {
        if order.name != FIELD_COMMAND {
            continue;
        }
        if let Some(size) = parse_field(&order.payload) {
            wanted = Some(size);
        }
    }
    let Some(size) = wanted else { return };
    let scale = size / field.0;
    field.0 = size;
    for (mut star, mut transform) in &mut stars {
        star.home *= scale;
        transform.translation *= scale;
    }
}

/// The four things you can say to it, none of which can hurry it.
fn controls(
    mut depth: ResMut<Depth>,
    mut night: ResMut<Night>,
    mut rng: ResMut<SimRng>,
    mut stars: Query<&mut Star>,
    field: Res<Field>,
    input: Res<Input>,
) {
    const LENGTHS: [(Key, f32); 9] = [
        (Key::Digit1, 1.0),
        (Key::Digit2, 2.0),
        (Key::Digit3, 3.0),
        (Key::Digit4, 4.0),
        (Key::Digit5, 5.0),
        (Key::Digit6, 6.0),
        (Key::Digit7, 7.0),
        (Key::Digit8, 8.0),
        (Key::Digit9, 9.0),
    ];
    for (key, steps) in LENGTHS {
        if input.just_pressed(key) {
            night.0 = (steps * NIGHT_STEP).clamp(NIGHT_LIMITS.0, NIGHT_LIMITS.1);
        }
    }
    // Still awake: hand back some of the night. Only the schedule moves; what you see and hear
    // walks back at its own pace over the next couple of minutes.
    if input.just_pressed(Key::Space) {
        depth.elapsed = (depth.elapsed - REPRIEVE).max(0.0);
    }
    // Begin again, with a different sky. The stars are already springs, so they simply drift to
    // their new places over the following seconds; there is nothing here that cuts.
    if input.just_pressed(Key::R) {
        depth.elapsed = 0.0;
        for mut star in &mut stars {
            star.home = pick_home(&mut rng, field.0);
            star.warmth = rng.range_f32(0.0..1.0);
            star.size = rng.range_f32(0.0..1.0).powi(3);
            star.lag = rng.range_f32(0.7..1.4);
            star.dim_at = rng.range_f32(STAR_DIM.0..STAR_DIM.1);
        }
    }
}

/// Advance the night, and let the piece walk toward wherever the schedule has got to.
fn descend(mut depth: ResMut<Depth>, night: Res<Night>, time: Res<Time>) {
    depth.elapsed += time.fixed_delta;
    depth.wanted = wanted_depth(depth.elapsed, night.0);
    depth.now = approach(depth.now, depth.wanted, DEPTH_RATE, time.fixed_delta);
}

/// Advance the breath, at whatever length the current depth asks for.
fn breathe(
    mut breath: ResMut<Breath>,
    mut began: ResMut<Began>,
    depth: Res<Depth>,
    time: Res<Time>,
) {
    breath.period = breath_period(depth.now);
    breath.inhale = inhale_fraction(depth.now);

    let before = breath.cycle;
    let advanced = before + time.fixed_delta / breath.period.max(1e-3);
    let wrapped = advanced >= 1.0;
    breath.cycle = advanced.rem_euclid(1.0);
    // The first tick of the night opens a draw, so the piece starts by breathing in rather than
    // by waiting out a silent breath.
    if wrapped || !began.0 {
        breath.draws += 1;
    }
    if !wrapped && before < breath.inhale && breath.cycle >= breath.inhale {
        breath.releases += 1;
    }
    breath.phase = breath_phase(breath.cycle, breath.inhale);
    began.0 = true;
}

/// Carry every star: pulled toward its place, resisted by the medium, and shaken by whatever
/// temperature is left.
///
/// This is Langevin motion, integrated semi-implicitly. The kick is scaled by
/// `sqrt(2 · drag · temperature · dt)`, which is what keeps the root-mean-square speed sitting at
/// [`jitter`] regardless of the tick rate. Get that scaling wrong and the piece runs hotter or
/// colder on a different machine.
///
/// Because the pull is always positive, a star can only ever wander a bounded distance from its
/// place, so nothing needs to be caught at a boundary, wrapped, or turned around. There is no
/// edge case in this loop at all, which is the reason it can be trusted to run unattended in a
/// dark room for half an hour.
fn settle(
    mut stars: Query<(&Star, &mut Transform2D, &mut Velocity)>,
    mut rng: ResMut<SimRng>,
    depth: Res<Depth>,
    breath: Res<Breath>,
    time: Res<Time>,
) {
    let dt = time.fixed_delta;
    let drag = drag(depth.now);
    // The breath loosens the sky a little as it fills and gathers it as it empties. It works on
    // the pull rather than on the light, so what you see breathing is the sky itself.
    let pull = home_pull(depth.now) * (1.0 - BREATH_SWELL * breath.phase);
    let temperature = jitter(depth.now) * jitter(depth.now);
    let kick = (2.0 * drag * temperature * dt).sqrt();

    for (star, mut transform, mut velocity) in &mut stars {
        let offset = transform.translation - star.home;
        let mut carried = velocity.0 + (-offset * (pull * star.lag) - velocity.0 * drag) * dt;
        if kick > 0.0 {
            carried += gaussian(&mut rng) * kick;
        }
        // At zero temperature this is a damped spring, which only ever approaches its rest
        // point. Once a star is within a fraction of a pixel of home and barely moving, it is
        // put to rest, so the end of the night is genuine stillness rather than a very small
        // amount of motion left running all night.
        if kick <= 0.0 && carried.length() < STILL_SPEED && offset.length() < STILL_DISTANCE {
            velocity.0 = Vec2::ZERO;
            transform.translation = star.home;
            continue;
        }
        velocity.0 = carried;
        transform.translation += carried * dt;
    }
}
