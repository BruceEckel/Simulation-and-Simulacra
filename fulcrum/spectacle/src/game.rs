//! Spectacle: a fireworks show over dark water, put on for no reason but the watching.
//!
//! Four decisions carry the piece, and each one is doing work rather than decoration:
//!
//! **Shells burst at their own apex.** A shell is given a height it should reach, and the
//! launch speed follows from gravity ([`launch_speed`]); it breaks when its climb runs out.
//! Nothing counts down a fuse to a scripted moment, so the rise, the hang, and the break are
//! one physical arc. Slow the show down and the arc stays honest instead of stretching a timer.
//!
//! **Drag is how the air is modelled, and the wind rides in on it.** Every star chases the
//! velocity of the air around it at a rate set by its own drag ([`Spark::drag`]), so a light
//! peony star is pulled into the breeze within a second while a heavy willow star ignores it
//! and falls. One line gets both the hang of a small star and the droop of a big one, and the
//! wind comes free, because the air it chases is already drifting.
//!
//! **The bang arrives late.** Sound is scheduled, not played: a break puts its report on a
//! queue with a delay of distance over [`SOUND_SPEED`], and the queue lets it go when the wave
//! would have reached the shore ([`deliver_reports`]). A flash a screen away lands about a
//! second before its boom, which is the single cue that says "far away and very large", and it
//! costs one queue.
//!
//! **A show has shape.** [`Act`] runs a fixed round of moods, from a sparse overture through a
//! cascade to a hush and then everything at once, and repeats. Fireworks fired at a constant
//! rate stop registering after a minute; the hush before the finale is what makes the finale
//! land, so the quiet passages are load-bearing.
//!
//! Pure logic, no sprites and no audio, so it runs headless for the determinism test. The
//! binary paints it and turns [`Report`] into noise.
//!
//! The field resizes with the window through [`FIELD_COMMAND`] on the replayable command
//! channel, never by reading renderer state; see the note in `boids` for why.

use fulcrum::prelude::*;
use std::f32::consts::TAU;

/// Field size at startup, and the area every resize preserves.
pub const DEFAULT_FIELD: Vec2 = Vec2::new(1280.0, 720.0);
/// Aspect-ratio limits for a resize.
pub const ASPECT_LIMITS: (f32, f32) = (0.5, 3.4);
/// Name of the resize command on the replayable command channel.
pub const FIELD_COMMAND: &str = "field";

/// Share of the field's height given to water, measured up from the bottom. The horizon sits
/// low, because the sky is where everything happens and the water is only there to hold the
/// reflection.
pub const WATER_SHARE: f32 = 0.22;

/// Downward pull, in units per second squared. Not real gravity: real gravity at this scale
/// makes stars rain straight down, and the whole look of a break is stars hanging.
pub const GRAVITY: f32 = 260.0;
/// How fast sound crosses the field, in units per second. Slow enough that the delay across a
/// screen is a full beat, which is the point.
pub const SOUND_SPEED: f32 = 620.0;
/// How quickly the air's push dies with distance, for the volume of a report.
pub const HEARING_RANGE: f32 = 760.0;

/// The most stars in the sky at once. Not an aesthetic limit, a promise that the tick stays
/// quick even mid-finale.
pub const MAX_SPARKS: u32 = 32_000;
/// The most shells in the air at once.
pub const MAX_SHELLS: u32 = 240;

/// Fraction of the field's height a shell may aim for, lowest and highest.
pub const APEX_RANGE: (f32, f32) = (0.42, 0.78);
/// How far from the middle a launch may be, as a fraction of the field's width.
pub const LAUNCH_SPREAD: f32 = 0.44;
/// Climb speed at which a shell gives up and breaks. Slightly above zero, so it breaks just
/// short of the top while still drifting, which is what keeps a break from looking stamped on.
pub const BURST_CLIMB: f32 = 26.0;
/// Longest a shell may burn before it breaks no matter what, in seconds.
pub const MAX_FUSE: f32 = 6.0;
/// Seconds between sparks in a climbing shell's tail.
pub const SHELL_TRAIL: f32 = 0.018;

/// Strongest the breeze gets, in units per second.
pub const WIND_PEAK: f32 = 38.0;

/// How fast the light of a break dies away, per second.
pub const FLASH_DECAY: f32 = 5.5;

/// Seconds a held mouse button waits between shells.
pub const HAND_COOLDOWN: f32 = 0.22;

/// Slowest the show can run, as a multiple of real time.
pub const SPEED_MIN: f32 = 0.15;
/// Fastest it can run.
pub const SPEED_MAX: f32 = 3.0;
/// How much a held speed key multiplies the rate each tick.
pub const SPEED_RAMP: f32 = 1.02;

/// How many color slots a palette has to fill. The simulation picks slots by role, never by
/// color, so a palette can restyle the entire show without the shells knowing.
pub const COLOR_SLOTS: u8 = 8;
/// Color slots by role.
pub const RED: u8 = 0;
/// See [`RED`].
pub const ORANGE: u8 = 1;
/// See [`RED`].
pub const GOLD: u8 = 2;
/// See [`RED`].
pub const GREEN: u8 = 3;
/// See [`RED`].
pub const CYAN: u8 = 4;
/// See [`RED`].
pub const BLUE: u8 = 5;
/// See [`RED`].
pub const MAGENTA: u8 = 6;
/// See [`RED`].
pub const WHITE: u8 = 7;

/// What a shell does when it breaks.
///
/// These are the real families, and they differ in physics rather than in decoration: a willow
/// is a peony with heavy stars and low drag, a chrysanthemum is a peony whose stars trail. Once
/// the star has the right weight, the shape follows on its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Break {
    /// A clean sphere of stars.
    Peony,
    /// A sphere whose every star draws a tail behind it.
    Chrysanthemum,
    /// Heavy, slow stars that hang and then droop most of the way to the water.
    Willow,
    /// A circle of stars seen at an angle, so it reads as a tilted hoop.
    Ring,
    /// A few thick fronds thrown out from a common center.
    Palm,
    /// Stars that fly out, pause, and split into small crosses.
    Crossette,
    /// A hard white flash and a crack, with almost nothing left over.
    Salute,
    /// A slow cloud of stars that blink out of step with each other.
    Strobe,
    /// Not a break at all: a fan of stars fired straight off the water.
    Mine,
}

/// A shell on its way up.
#[derive(Component)]
pub struct Shell {
    /// What it will do at the top.
    pub kind: Break,
    /// The two color slots its stars draw from.
    pub colors: (u8, u8),
    /// Size of the break to come, around 1.0.
    pub power: f32,
    /// Seconds until the next spark of its tail.
    pub trail: f32,
    /// Seconds before it breaks whatever else happens.
    pub fuse: f32,
}

/// One burning star.
#[derive(Component)]
pub struct Spark {
    /// Seconds lived.
    pub age: f32,
    /// Seconds it will burn.
    pub life: f32,
    /// Which palette slot it draws its color from.
    pub color: u8,
    /// How big it is, in world units.
    pub size: f32,
    /// How quickly it gives itself up to the air, per second. High drag is a small light star,
    /// low drag is a heavy one.
    pub drag: f32,
    /// How hard gravity holds it, as a multiple of [`GRAVITY`].
    pub weight: f32,
    /// Blinks per second, or zero for a steady star.
    pub twinkle: f32,
    /// A fixed number in `0..1`, so stars that blink do not blink together.
    pub seed: f32,
    /// Seconds between sparks of its own tail, or zero for a star that leaves none.
    pub trail_every: f32,
    /// Seconds until the next one.
    pub trail_in: f32,
    /// Seconds until it splits, when [`Spark::split_into`] is set.
    pub split_in: f32,
    /// How many stars it splits into, or zero.
    pub split_into: u8,
}

impl Spark {
    /// A plain star: light, draggy, no tail, no split.
    pub fn new(color: u8, life: f32, size: f32) -> Self {
        Self {
            age: 0.0,
            life,
            color,
            size,
            drag: 1.7,
            weight: 0.35,
            twinkle: 0.0,
            seed: 0.0,
            trail_every: 0.0,
            trail_in: 0.0,
            split_in: 0.0,
            split_into: 0,
        }
    }

    /// How bright it is, `0..1`. A quick attack and a long decay, which is how burning metal
    /// behaves and, more to the point, is what keeps a sky full of stars from flickering.
    pub fn presence(&self) -> f32 {
        let fraction = (self.age / self.life.max(1e-3)).clamp(0.0, 1.0);
        let attack = (fraction / 0.06).clamp(0.0, 1.0);
        let decay = (1.0 - fraction).powf(1.6);
        attack * decay
    }
}

/// A puff left behind by a break, drifting on the wind.
#[derive(Component)]
pub struct Smoke {
    /// Seconds lived.
    pub age: f32,
    /// Seconds it will last.
    pub life: f32,
    /// How wide it starts, in world units.
    pub size: f32,
    /// The slot of the break that made it, so a fresh puff still carries its color.
    pub color: u8,
}

/// Simulation velocity, units per second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// The box the show fills. Simulation state, changed only by [`FIELD_COMMAND`].
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Field(pub Vec2);

impl Default for Field {
    fn default() -> Self {
        Self(DEFAULT_FIELD)
    }
}

/// Which passage of the show is running.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    /// Single shells with room around them, so the eye learns the scale.
    Overture,
    /// More of them, closer together, bigger.
    Rise,
    /// Overlapping breaks, one on top of the last.
    Cascade,
    /// Almost nothing: a willow or two, and the smoke clearing.
    Hush,
    /// Everything at once, until it stops.
    Finale,
}

impl Act {
    /// How long this passage runs, in seconds.
    pub fn duration(self) -> f32 {
        match self {
            Act::Overture => 26.0,
            Act::Rise => 40.0,
            Act::Cascade => 34.0,
            Act::Hush => 16.0,
            Act::Finale => 22.0,
        }
    }

    /// Shortest and longest wait between launches, in seconds.
    pub fn gap(self) -> (f32, f32) {
        match self {
            Act::Overture => (1.7, 3.1),
            Act::Rise => (0.85, 1.7),
            Act::Cascade => (0.42, 0.9),
            Act::Hush => (2.6, 4.6),
            Act::Finale => (0.11, 0.3),
        }
    }

    /// Fewest and most shells in one launch.
    pub fn salvo(self) -> (i32, i32) {
        match self {
            Act::Overture => (1, 2),
            Act::Rise => (1, 3),
            Act::Cascade => (1, 4),
            Act::Hush => (1, 2),
            Act::Finale => (2, 5),
        }
    }

    /// Smallest and largest break this passage asks for.
    pub fn power(self) -> (f32, f32) {
        match self {
            Act::Overture => (0.72, 1.0),
            Act::Rise => (0.85, 1.15),
            Act::Cascade => (0.8, 1.2),
            Act::Hush => (0.9, 1.3),
            Act::Finale => (0.7, 1.35),
        }
    }

    /// What this passage fires. Repeats in the list are weight: the overture is mostly peonies
    /// because a first impression should be the plainest possible version of the thing.
    pub fn repertoire(self) -> &'static [Break] {
        match self {
            Act::Overture => &[
                Break::Peony,
                Break::Peony,
                Break::Peony,
                Break::Chrysanthemum,
                Break::Ring,
                Break::Willow,
            ],
            Act::Rise => &[
                Break::Peony,
                Break::Peony,
                Break::Chrysanthemum,
                Break::Chrysanthemum,
                Break::Ring,
                Break::Palm,
                Break::Crossette,
                Break::Strobe,
                Break::Willow,
            ],
            Act::Cascade => &[
                Break::Peony,
                Break::Chrysanthemum,
                Break::Crossette,
                Break::Ring,
                Break::Palm,
                Break::Salute,
                Break::Strobe,
                Break::Mine,
            ],
            Act::Hush => &[
                Break::Willow,
                Break::Willow,
                Break::Willow,
                Break::Chrysanthemum,
                Break::Ring,
            ],
            Act::Finale => &[
                Break::Peony,
                Break::Chrysanthemum,
                Break::Crossette,
                Break::Palm,
                Break::Salute,
                Break::Ring,
                Break::Mine,
                Break::Strobe,
                Break::Willow,
            ],
        }
    }

    /// The passage after this one. The round is fixed, so the show has a shape you can learn
    /// without ever being able to predict a single shell.
    pub fn next(self) -> Act {
        match self {
            Act::Overture => Act::Rise,
            Act::Rise => Act::Cascade,
            Act::Cascade => Act::Hush,
            Act::Hush => Act::Finale,
            Act::Finale => Act::Overture,
        }
    }

    /// What to call it on screen.
    pub fn name(self) -> &'static str {
        match self {
            Act::Overture => "overture",
            Act::Rise => "rise",
            Act::Cascade => "cascade",
            Act::Hush => "hush",
            Act::Finale => "finale",
        }
    }
}

/// Where the show has got to.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Show {
    /// The passage running now.
    pub act: Act,
    /// Seconds into it.
    pub elapsed: f32,
    /// Seconds until the next launch.
    pub next: f32,
    /// Shells fired since the show began.
    pub fired: u32,
}

impl Default for Show {
    fn default() -> Self {
        Self {
            act: Act::Overture,
            elapsed: 0.0,
            // A beat of dark before the first shell, so the first one is an event.
            next: 1.6,
            fired: 0,
        }
    }
}

/// Total seconds the show has been running, which is also the clock the breeze turns on.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Elapsed(pub f32);

/// The breeze, in units per second, positive to the right.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Wind(pub f32);

/// The light of the most recent break, dying away.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Flash {
    /// How much light is left, `0..~1.4`.
    pub level: f32,
    /// The slot it was lit in.
    pub color: u8,
    /// Where it came from.
    pub at: Vec2,
}

/// A voice in the show's noise.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Voice {
    /// The thump of a mortar and the hiss of a shell leaving it.
    Launch,
    /// The body of a break.
    Boom,
    /// The hard crack of a salute.
    Crack,
    /// The rattle of small stars going off after one.
    Crackle,
}

/// A sound, at the moment it reaches the shore. Written by the simulation, played by the
/// binary.
#[derive(Event, Clone, Copy, PartialEq, Debug)]
pub struct Report {
    /// Which voice.
    pub voice: Voice,
    /// How loud, `0..1`, already faded for distance.
    pub volume: f32,
    /// Where it sits between the ears, `-1..1`.
    pub pan: f32,
    /// Playback rate. Big shells speak low.
    pub pitch: f32,
}

/// A break, at the moment it happens. The binary uses these for the reflection on the water;
/// the noise arrives separately and later.
#[derive(Event, Clone, Copy, PartialEq, Debug)]
pub struct Bloom {
    /// Where it broke.
    pub at: Vec2,
    /// Its first color slot.
    pub color: u8,
    /// How big it was, around 1.0.
    pub power: f32,
}

/// Reports still crossing the water, each with the seconds left before it lands.
#[derive(Resource, Default, Clone, Debug)]
pub struct Pending(pub Vec<(f32, Report)>);

/// What is in the sky.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Census {
    /// Stars burning.
    pub sparks: u32,
    /// Shells climbing.
    pub shells: u32,
}

/// Nothing moves while this is set.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Paused(pub bool);

/// How fast the show runs, as a multiple of real time.
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

/// Seconds until a held mouse button may fire again.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Hand(pub f32);

/// Height of the water line in world units.
pub fn horizon(field: Vec2) -> f32 {
    field.y * (WATER_SHARE - 0.5)
}

/// The field a window of this pixel size should get: the window's aspect at [`DEFAULT_FIELD`]'s
/// area, so a break is the same size on any window shape.
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

/// The climb a shell needs to reach `height` above where it started.
///
/// Straight out of `v^2 = 2gh`, which is why a shell aimed at the top of the frame takes about
/// two seconds to get there and hangs at the top for a moment: the physics does the timing, so
/// there is no fuse to keep in step with anything.
pub fn launch_speed(height: f32) -> f32 {
    (2.0 * GRAVITY * height.max(0.0)).sqrt()
}

/// Seconds for a sound made at `at` to reach the near shore.
pub fn travel_delay(at: Vec2, field: Vec2) -> f32 {
    let ear = vec2(0.0, -field.y * 0.5);
    (at - ear).length() / SOUND_SPEED
}

/// How much of a sound made at `at` is left by the time it lands, `0..1`.
pub fn travel_volume(at: Vec2, field: Vec2) -> f32 {
    let ear = vec2(0.0, -field.y * 0.5);
    1.0 / (1.0 + (at - ear).length() / HEARING_RANGE)
}

/// Where a sound made at `at` sits between the ears.
pub fn travel_pan(at: Vec2, field: Vec2) -> f32 {
    (at.x / (field.x * 0.5).max(1.0)).clamp(-1.0, 1.0) * 0.85
}

/// How much of a star is left as it nears the water, `0..1`. Stars are put out by the water
/// rather than colliding with it, so nothing is ever seen to hit anything.
pub fn water_fade(y: f32, field: Vec2) -> f32 {
    let above = (y - horizon(field)) / 70.0;
    let fraction = above.clamp(0.0, 1.0);
    fraction * fraction * (3.0 - 2.0 * fraction)
}

/// Installs the show.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Field::default());
        app.world_mut().insert_resource(Show::default());
        app.world_mut().insert_resource(Elapsed::default());
        app.world_mut().insert_resource(Wind::default());
        app.world_mut().insert_resource(Flash::default());
        app.world_mut().insert_resource(Pending::default());
        app.world_mut().insert_resource(Census::default());
        app.world_mut().insert_resource(Paused::default());
        app.world_mut().insert_resource(Speed::default());
        app.world_mut().insert_resource(Step::default());
        app.world_mut().insert_resource(Hand::default());
        app.register_event::<Report>();
        app.register_event::<Bloom>();
        app.add_systems(
            FixedUpdate,
            (
                apply_field,
                pace,
                set_step,
                advance_clock,
                direct_show,
                launch_by_hand,
                fly_shells,
                move_sparks,
                drift_smoke,
                fade_flash,
                deliver_reports,
            )
                .chain(),
        );
    }
}

/// Move the boundary when a resize command arrives.
///
/// Nothing in the sky is dragged inside the new box: stars are ballistic, and a resize that
/// teleported them would break the one thing the eye is following. They simply leave, and the
/// next shells use the new field.
fn apply_field(mut field: ResMut<Field>, mut orders: EventReader<CommandEvent>) {
    for order in orders.read() {
        if order.name != FIELD_COMMAND {
            continue;
        }
        if let Some(size) = parse_field(&order.payload) {
            field.0 = size;
        }
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

/// Advance the show's clock and turn the breeze.
///
/// Two slow waves at unrelated rates, so the wind wanders instead of swinging: stars from the
/// same shell drift together, and stars from a shell a minute later drift somewhere else.
fn advance_clock(mut elapsed: ResMut<Elapsed>, mut wind: ResMut<Wind>, step: Res<Step>) {
    if step.seconds <= 0.0 {
        return;
    }
    elapsed.0 += step.seconds;
    let slow = (elapsed.0 * 0.043).sin();
    let slower = (elapsed.0 * 0.017 + 1.3).sin();
    wind.0 = WIND_PEAK * (slow * 0.7 + slower * 0.3);
}

/// Run the programme: advance the passage, and fire what it calls for.
#[expect(
    clippy::too_many_arguments,
    reason = "the director needs the whole stage"
)]
fn direct_show(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut show: ResMut<Show>,
    mut census: ResMut<Census>,
    mut pending: ResMut<Pending>,
    mut blooms: EventWriter<Bloom>,
    mut flash: ResMut<Flash>,
    field: Res<Field>,
    step: Res<Step>,
    input: Res<Input>,
) {
    // The finale is one key away at any moment, because sometimes you do not want to wait
    // three minutes for the good part.
    if input.just_pressed(Key::F) && show.act != Act::Finale {
        show.act = Act::Finale;
        show.elapsed = 0.0;
        show.next = 0.0;
    }
    if step.seconds <= 0.0 {
        return;
    }
    show.elapsed += step.seconds;
    if show.elapsed >= show.act.duration() {
        show.act = show.act.next();
        show.elapsed = 0.0;
        show.next = show.act.gap().0;
    }

    show.next -= step.seconds;
    if show.next > 0.0 {
        return;
    }
    let (short, long) = show.act.gap();
    show.next = rng.range_f32(short..long);

    let (fewest, most) = show.act.salvo();
    let shells = rng.range_i32(fewest..most + 1);
    // A salvo is spread across the width rather than stacked in one place, so a busy passage
    // reads as a line of fire along the shore instead of a pile in the middle.
    for index in 0..shells {
        let lane = if shells <= 1 {
            rng.range_f32(-1.0..1.0)
        } else {
            let step = 2.0 / (shells - 1) as f32;
            -1.0 + step * index as f32 + rng.range_f32(-0.16..0.16)
        };
        let x = lane.clamp(-1.0, 1.0) * field.0.x * LAUNCH_SPREAD;
        let (weakest, strongest) = show.act.power();
        let power = rng.range_f32(weakest..strongest);
        let list = show.act.repertoire();
        let kind = list[rng.range_i32(0..list.len() as i32) as usize];
        let apex = rng.range_f32(APEX_RANGE.0..APEX_RANGE.1) * field.0.y;
        fire(
            &mut commands,
            &mut rng,
            &mut census,
            &mut pending,
            &mut blooms,
            &mut flash,
            field.0,
            x,
            apex,
            kind,
            power,
        );
        show.fired += 1;
    }
}

/// A shell fired at the pointer, bursting where it was clicked.
#[expect(
    clippy::too_many_arguments,
    reason = "one launch needs the whole stage"
)]
fn launch_by_hand(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    mut pending: ResMut<Pending>,
    mut blooms: EventWriter<Bloom>,
    mut flash: ResMut<Flash>,
    mut hand: ResMut<Hand>,
    field: Res<Field>,
    step: Res<Step>,
    input: Res<Input>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    hand.0 = (hand.0 - step.seconds).max(0.0);
    if !input.mouse_pressed(MouseButton::Left) || hand.0 > 0.0 {
        return;
    }
    let target = input.mouse_world();
    let water = horizon(field.0);
    if target.y <= water + 40.0 {
        return;
    }
    hand.0 = HAND_COOLDOWN;
    // Aimed, not scripted: the apex is wherever the pointer was, and the launch speed follows.
    // A shell you place yourself lands in the same physics as every other one.
    let list = Act::Rise.repertoire();
    let kind = list[rng.range_i32(0..list.len() as i32) as usize];
    let power = rng.range_f32(0.85..1.2);
    fire(
        &mut commands,
        &mut rng,
        &mut census,
        &mut pending,
        &mut blooms,
        &mut flash,
        field.0,
        target.x.clamp(-field.0.x * 0.48, field.0.x * 0.48),
        target.y - water,
        kind,
        power,
    );
}

/// Pick the two color slots a break draws from.
///
/// Willows are gold and salutes are white because that is what they are made of, not because
/// it looks nice. Everything else takes one slot, and half the time a second, since a two-color
/// break is the cheapest way to make a plain sphere interesting.
fn choose_colors(rng: &mut SimRng, kind: Break) -> (u8, u8) {
    match kind {
        Break::Willow => (GOLD, GOLD),
        Break::Salute => (WHITE, WHITE),
        _ => {
            let first = rng.range_i32(0..COLOR_SLOTS as i32) as u8;
            if rng.chance(0.5) {
                (first, first)
            } else {
                let mut second = rng.range_i32(0..COLOR_SLOTS as i32) as u8;
                if second == first {
                    second = (first + 1 + rng.range_i32(0..3) as u8) % COLOR_SLOTS;
                }
                (first, second)
            }
        }
    }
}

/// Send one shell up, or, for a mine, fire it straight off the water.
#[expect(
    clippy::too_many_arguments,
    reason = "one launch needs the whole stage"
)]
pub fn fire(
    commands: &mut Commands,
    rng: &mut SimRng,
    census: &mut Census,
    pending: &mut Pending,
    blooms: &mut EventWriter<Bloom>,
    flash: &mut Flash,
    field: Vec2,
    x: f32,
    apex: f32,
    kind: Break,
    power: f32,
) {
    let colors = choose_colors(rng, kind);
    let from = vec2(x, horizon(field) + 6.0);

    if kind == Break::Mine {
        // A mine has no flight: it is the mortar itself, opened at the water line.
        schedule(
            pending,
            field,
            from,
            0.0,
            Voice::Boom,
            0.85 * power,
            1.15 - 0.2 * power,
        );
        burst(
            commands,
            rng,
            census,
            pending,
            blooms,
            flash,
            field,
            from,
            Vec2::ZERO,
            kind,
            colors,
            power,
        );
        return;
    }

    if census.shells >= MAX_SHELLS {
        return;
    }
    census.shells += 1;
    let climb = launch_speed(apex);
    let lean = rng.range_f32(-0.12..0.12);
    commands.spawn((
        Shell {
            kind,
            colors,
            power,
            trail: 0.0,
            fuse: MAX_FUSE,
        },
        Transform2D::from_translation(from),
        Velocity(vec2(climb * lean, climb)),
    ));
    schedule(
        pending,
        field,
        from,
        0.0,
        Voice::Launch,
        0.4,
        rng.range_f32(0.92..1.1),
    );
}

/// Put a sound on the queue, to be let go when its wave would reach the shore.
fn schedule(
    pending: &mut Pending,
    field: Vec2,
    at: Vec2,
    after: f32,
    voice: Voice,
    volume: f32,
    pitch: f32,
) {
    pending.0.push((
        travel_delay(at, field) + after,
        Report {
            voice,
            volume: (volume * travel_volume(at, field)).clamp(0.0, 1.0),
            pan: travel_pan(at, field),
            pitch,
        },
    ));
}

/// Put one star in the sky, if there is room for it.
fn add_spark(
    commands: &mut Commands,
    census: &mut Census,
    at: Vec2,
    velocity: Vec2,
    spark: Spark,
) -> bool {
    if census.sparks >= MAX_SPARKS {
        return false;
    }
    census.sparks += 1;
    commands.spawn((spark, Transform2D::from_translation(at), Velocity(velocity)));
    true
}

/// A unit vector at a random angle.
fn any_direction(rng: &mut SimRng) -> Vec2 {
    let angle = rng.range_f32(0.0..TAU);
    vec2(angle.cos(), angle.sin())
}

/// Break a shell open.
///
/// Every family here is the same loop over stars with different weights, drags and lifetimes.
/// That is not a shortcut, it is the physics of the real thing: a willow and a peony leave the
/// same mortar, and what makes them different is what the stars are made of.
#[expect(clippy::too_many_arguments, reason = "a break needs the whole stage")]
pub fn burst(
    commands: &mut Commands,
    rng: &mut SimRng,
    census: &mut Census,
    pending: &mut Pending,
    blooms: &mut EventWriter<Bloom>,
    flash: &mut Flash,
    field: Vec2,
    at: Vec2,
    inherited: Vec2,
    kind: Break,
    colors: (u8, u8),
    power: f32,
) {
    // A break keeps the shell's drift, so a windy shell throws a lopsided sphere. Only a
    // fraction of it: stars leave the casing far faster than the casing was moving.
    let carried = inherited * 0.35;
    let pick = |rng: &mut SimRng| {
        if rng.chance(0.5) { colors.0 } else { colors.1 }
    };

    match kind {
        Break::Peony | Break::Chrysanthemum | Break::Strobe => {
            let trailing = kind == Break::Chrysanthemum;
            let blinking = kind == Break::Strobe;
            let count = (if blinking { 110.0 } else { 165.0 } * power) as i32;
            let reach = if blinking { 205.0 } else { 300.0 } * power;
            for _ in 0..count {
                let color = pick(rng);
                let speed = reach * rng.range_f32(0.78..1.0);
                let life = if blinking {
                    rng.range_f32(2.6..3.6)
                } else if trailing {
                    rng.range_f32(2.1..3.0)
                } else {
                    rng.range_f32(1.5..2.3)
                };
                let spark = Spark {
                    drag: if trailing { 1.3 } else { 1.8 },
                    weight: if trailing { 0.55 } else { 0.35 },
                    twinkle: if blinking {
                        rng.range_f32(4.0..7.0)
                    } else {
                        0.0
                    },
                    seed: rng.unit_f32(),
                    trail_every: if trailing { 0.05 } else { 0.0 },
                    ..Spark::new(color, life, 7.4)
                };
                if !add_spark(
                    commands,
                    census,
                    at,
                    carried + any_direction(rng) * speed,
                    spark,
                ) {
                    break;
                }
            }
        }
        Break::Willow => {
            // Heavy stars, low drag: they ignore the air, hang, and then fall in long strands
            // that reach most of the way to the water. The whole family is those two numbers.
            let count = (96.0 * power) as i32;
            for _ in 0..count {
                let speed = 196.0 * power * rng.range_f32(0.7..1.0);
                let spark = Spark {
                    drag: 0.8,
                    weight: 0.62,
                    seed: rng.unit_f32(),
                    trail_every: 0.075,
                    ..Spark::new(pick(rng), rng.range_f32(3.6..5.2), 7.4)
                };
                if !add_spark(
                    commands,
                    census,
                    at,
                    carried + any_direction(rng) * speed,
                    spark,
                ) {
                    break;
                }
            }
        }
        Break::Ring => {
            // A real ring is a circle in three dimensions, and what sells it on a flat screen
            // is seeing it edge-on: the circle is squashed on one axis and then rolled, so it
            // reads as a hoop hanging at an angle rather than as a drawn O.
            let count = (124.0 * power) as i32;
            let squash = rng.range_f32(0.22..0.8);
            let roll = rng.range_f32(0.0..TAU);
            let (sin, cos) = roll.sin_cos();
            let reach = 252.0 * power;
            for index in 0..count {
                let angle = TAU * index as f32 / count.max(1) as f32;
                let flat = vec2(angle.cos(), angle.sin() * squash);
                let turned = vec2(flat.x * cos - flat.y * sin, flat.x * sin + flat.y * cos);
                let speed = reach * rng.range_f32(0.94..1.03);
                let spark = Spark {
                    drag: 1.5,
                    seed: rng.unit_f32(),
                    ..Spark::new(pick(rng), rng.range_f32(1.8..2.3), 6.6)
                };
                if !add_spark(commands, census, at, carried + turned * speed, spark) {
                    break;
                }
            }
        }
        Break::Palm => {
            // A palm is a handful of thick fronds, not a sphere: a heavy head that trails hard,
            // with lighter stars strung out behind it along the same line.
            let fronds = (10.0 * power) as i32;
            let head_speed = 310.0 * power;
            for index in 0..fronds.max(3) {
                let angle = TAU * index as f32 / fronds.max(3) as f32 + rng.range_f32(-0.2..0.2);
                let direction = vec2(angle.cos(), angle.sin());
                let color = pick(rng);
                let head = Spark {
                    drag: 0.9,
                    weight: 0.6,
                    seed: rng.unit_f32(),
                    trail_every: 0.03,
                    ..Spark::new(color, rng.range_f32(2.4..3.2), 10.5)
                };
                if !add_spark(commands, census, at, carried + direction * head_speed, head) {
                    break;
                }
                for _ in 0..7 {
                    let follow = direction * head_speed * rng.range_f32(0.4..0.95);
                    let spread = vec2(-direction.y, direction.x) * rng.range_f32(-18.0..18.0);
                    let spark = Spark {
                        drag: 1.1,
                        weight: 0.5,
                        seed: rng.unit_f32(),
                        trail_every: 0.06,
                        ..Spark::new(color, rng.range_f32(1.8..2.6), 7.0)
                    };
                    if !add_spark(commands, census, at, carried + follow + spread, spark) {
                        break;
                    }
                }
            }
        }
        Break::Crossette => {
            let count = (22.0 * power) as i32;
            let wait = rng.range_f32(0.5..0.7);
            for _ in 0..count {
                let speed = 214.0 * power * rng.range_f32(0.9..1.05);
                let spark = Spark {
                    drag: 1.1,
                    weight: 0.4,
                    seed: rng.unit_f32(),
                    trail_every: 0.05,
                    split_in: wait,
                    split_into: 4,
                    ..Spark::new(pick(rng), wait + rng.range_f32(0.05..0.2), 8.6)
                };
                if !add_spark(
                    commands,
                    census,
                    at,
                    carried + any_direction(rng) * speed,
                    spark,
                ) {
                    break;
                }
            }
            // The rattle of the splits, timed for when they happen rather than for the break.
            schedule(
                pending,
                field,
                at,
                wait,
                Voice::Crackle,
                0.5 * power,
                rng.range_f32(0.95..1.1),
            );
        }
        Break::Salute => {
            // Almost nothing to look at and everything to hear: a hard flash, a scatter of
            // white grit, and a crack. A show needs punctuation as much as it needs sentences.
            let count = (64.0 * power) as i32;
            for _ in 0..count {
                let speed = rng.range_f32(90.0..420.0) * power;
                let spark = Spark {
                    drag: 2.7,
                    weight: 0.3,
                    twinkle: rng.range_f32(18.0..30.0),
                    seed: rng.unit_f32(),
                    ..Spark::new(WHITE, rng.range_f32(0.45..0.95), 5.0)
                };
                if !add_spark(
                    commands,
                    census,
                    at,
                    carried + any_direction(rng) * speed,
                    spark,
                ) {
                    break;
                }
            }
        }
        Break::Mine => {
            // Fired off the water in a fan, so it fills the bottom of the frame where nothing
            // else goes.
            let count = (96.0 * power) as i32;
            for _ in 0..count {
                let lean = rng.range_f32(-0.62..0.62);
                let direction = vec2(lean.sin(), lean.cos());
                let speed = rng.range_f32(430.0..660.0) * power;
                let spark = Spark {
                    drag: 0.6,
                    weight: 0.85,
                    seed: rng.unit_f32(),
                    trail_every: 0.05,
                    ..Spark::new(pick(rng), rng.range_f32(1.9..2.9), 7.6)
                };
                if !add_spark(commands, census, at, direction * speed, spark) {
                    break;
                }
            }
        }
    }

    // Smoke, which is what makes the next break look like it is happening inside weather.
    let puffs = if kind == Break::Salute { 1 } else { 3 };
    for _ in 0..puffs {
        let drift = any_direction(rng) * rng.range_f32(8.0..40.0);
        commands.spawn((
            Smoke {
                age: 0.0,
                life: rng.range_f32(4.5..8.0),
                size: rng.range_f32(70.0..130.0) * power,
                color: colors.0,
            },
            Transform2D::from_translation(at + any_direction(rng) * rng.range_f32(0.0..40.0)),
            Velocity(drift),
        ));
    }

    let brightness = match kind {
        Break::Salute => 1.4,
        Break::Mine => 0.7,
        Break::Willow | Break::Strobe => 0.55,
        _ => 0.9,
    } * power;
    if brightness > flash.level {
        flash.level = brightness;
        flash.color = colors.0;
        flash.at = at;
    }
    blooms.write(Bloom {
        at,
        color: colors.0,
        power,
    });

    match kind {
        Break::Salute => {
            schedule(
                pending,
                field,
                at,
                0.0,
                Voice::Crack,
                1.0,
                rng.range_f32(0.95..1.15),
            );
            schedule(
                pending,
                field,
                at,
                0.12,
                Voice::Crackle,
                0.7,
                rng.range_f32(1.0..1.2),
            );
        }
        Break::Mine => {}
        _ => {
            // Bigger shells speak lower and louder, which is the other half of "far away and
            // very large" after the delay.
            schedule(
                pending,
                field,
                at,
                0.0,
                Voice::Boom,
                (0.55 + 0.45 * power).min(1.0),
                (1.25 - 0.35 * power).clamp(0.6, 1.3),
            );
            if kind == Break::Strobe || rng.chance(0.25) {
                schedule(
                    pending,
                    field,
                    at,
                    0.18,
                    Voice::Crackle,
                    0.35 * power,
                    rng.range_f32(0.95..1.15),
                );
            }
        }
    }
}

/// Carry the shells up, and break them at the top of the climb.
#[expect(clippy::too_many_arguments, reason = "flight ends in a break")]
fn fly_shells(
    mut commands: Commands,
    mut shells: Query<(Entity, &mut Shell, &mut Transform2D, &mut Velocity)>,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    mut pending: ResMut<Pending>,
    mut blooms: EventWriter<Bloom>,
    mut flash: ResMut<Flash>,
    field: Res<Field>,
    wind: Res<Wind>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    let ceiling = field.0.y * 0.5 - 24.0;

    for (entity, mut shell, mut transform, mut velocity) in &mut shells {
        // A shell is heavy and fast, so the breeze barely reaches it. Its stars are another
        // matter entirely.
        velocity.0.y -= GRAVITY * dt;
        velocity.0.x += wind.0 * 0.18 * dt;
        transform.translation += velocity.0 * dt;
        transform.rotation = velocity.0.to_angle();

        shell.trail -= dt;
        while shell.trail <= 0.0 {
            shell.trail += SHELL_TRAIL;
            let spark = Spark {
                drag: 2.4,
                weight: 0.4,
                twinkle: rng.range_f32(0.0..12.0),
                seed: rng.unit_f32(),
                ..Spark::new(GOLD, rng.range_f32(0.3..0.6), 5.0)
            };
            let scatter = any_direction(&mut rng) * rng.range_f32(0.0..26.0);
            add_spark(
                &mut commands,
                &mut census,
                transform.translation,
                velocity.0 * 0.12 + scatter,
                spark,
            );
        }

        shell.fuse -= dt;
        let spent = velocity.0.y <= BURST_CLIMB;
        let too_high = transform.translation.y >= ceiling;
        if spent || too_high || shell.fuse <= 0.0 {
            burst(
                &mut commands,
                &mut rng,
                &mut census,
                &mut pending,
                &mut blooms,
                &mut flash,
                field.0,
                transform.translation,
                velocity.0,
                shell.kind,
                shell.colors,
                shell.power,
            );
            commands.entity(entity).despawn();
            census.shells = census.shells.saturating_sub(1);
        }
    }
}

/// Everything a burning star does: fall, give itself to the air, trail, split, go out.
fn move_sparks(
    mut commands: Commands,
    mut sparks: Query<(Entity, &mut Spark, &mut Transform2D, &mut Velocity)>,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    field: Res<Field>,
    wind: Res<Wind>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    let air = vec2(wind.0, 0.0);
    let water = horizon(field.0);
    let edge = field.0.x * 0.5 + 140.0;
    let mut born: Vec<(Vec2, Vec2, Spark)> = Vec::new();

    for (entity, mut spark, mut transform, mut velocity) in &mut sparks {
        spark.age += dt;

        // Drag as an approach to the velocity of the air, not as a subtraction from speed.
        // Written this way it is exact at any step size, and it hands the star to the wind for
        // free: a light star matches the breeze in under a second, a heavy one never does.
        let surrender = 1.0 - (-spark.drag * dt).exp();
        let carried = velocity.0;
        velocity.0 += (air - carried) * surrender;
        velocity.0.y -= GRAVITY * spark.weight * dt;
        transform.translation += velocity.0 * dt;
        transform.rotation = velocity.0.to_angle();

        if spark.trail_every > 0.0 {
            spark.trail_in -= dt;
            while spark.trail_in <= 0.0 {
                spark.trail_in += spark.trail_every;
                let ember = Spark {
                    drag: 2.6,
                    weight: 0.3,
                    seed: rng.unit_f32(),
                    ..Spark::new(spark.color, rng.range_f32(0.35..0.7), spark.size * 0.62)
                };
                born.push((
                    transform.translation,
                    velocity.0 * 0.2 + any_direction(&mut rng) * rng.range_f32(0.0..16.0),
                    ember,
                ));
            }
        }

        if spark.split_into > 0 {
            spark.split_in -= dt;
            if spark.split_in <= 0.0 {
                let pieces = spark.split_into;
                let turn = rng.range_f32(0.0..TAU);
                for index in 0..pieces {
                    let angle = turn + TAU * index as f32 / pieces as f32;
                    let child = Spark {
                        drag: 1.6,
                        weight: 0.45,
                        seed: rng.unit_f32(),
                        ..Spark::new(spark.color, rng.range_f32(0.7..1.1), 6.6)
                    };
                    born.push((
                        transform.translation,
                        velocity.0 * 0.25 + vec2(angle.cos(), angle.sin()) * 152.0,
                        child,
                    ));
                }
                commands.entity(entity).despawn();
                census.sparks = census.sparks.saturating_sub(1);
                continue;
            }
        }

        let gone = spark.age >= spark.life
            || transform.translation.y <= water
            || transform.translation.x.abs() > edge
            || transform.translation.y > field.0.y * 0.5 + 200.0;
        if gone {
            commands.entity(entity).despawn();
            census.sparks = census.sparks.saturating_sub(1);
        }
    }

    for (at, velocity, spark) in born {
        add_spark(&mut commands, &mut census, at, velocity, spark);
    }
}

/// Smoke expands, drifts, and goes.
fn drift_smoke(
    mut commands: Commands,
    mut puffs: Query<(Entity, &mut Smoke, &mut Transform2D, &mut Velocity)>,
    wind: Res<Wind>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    let air = vec2(wind.0, 12.0);
    for (entity, mut puff, mut transform, mut velocity) in &mut puffs {
        puff.age += dt;
        let surrender = 1.0 - (-0.8 * dt).exp();
        let carried = velocity.0;
        velocity.0 += (air - carried) * surrender;
        transform.translation += velocity.0 * dt;
        if puff.age >= puff.life {
            commands.entity(entity).despawn();
        }
    }
}

/// The light of the last break dying away.
fn fade_flash(mut flash: ResMut<Flash>, step: Res<Step>) {
    if step.seconds <= 0.0 {
        return;
    }
    flash.level *= (-FLASH_DECAY * step.seconds).exp();
}

/// Let go of every report whose wave has arrived.
fn deliver_reports(
    mut pending: ResMut<Pending>,
    mut reports: EventWriter<Report>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    let mut landed: Vec<Report> = Vec::new();
    pending.0.retain_mut(|(left, report)| {
        *left -= dt;
        if *left <= 0.0 {
            landed.push(*report);
            false
        } else {
            true
        }
    });
    for report in landed {
        reports.write(report);
    }
}
