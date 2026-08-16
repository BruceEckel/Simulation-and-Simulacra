//! Rally simulation: pong's shape without pong's game. Two paddles and one ball to start,
//! then the court keeps adding both, forever — the paddles subdividing their wall as they
//! multiply, the balls piling up until the court is traffic. Nobody is playing: every paddle
//! is driven by the same intercept-prediction routine, and the only thing the run produces is
//! statistics.
//!
//! Pure logic — no sprites, no audio — so it runs headless for the determinism test.
//!
//! **Unbounded population** is the constraint that shapes the rest of this file. There is no
//! ceiling on balls or paddles, so:
//!
//! - Sizes taper by a constant ratio per arrival rather than a ramp toward a known maximum;
//!   see [`ball_size`] and [`paddle_shape`].
//! - Collision is swept, not sampled, so an arbitrarily thin paddle still returns balls
//!   instead of letting them tunnel through between ticks.
//! - Nothing scans everything: balls and paddles meet through the wall's evenly divided bands
//!   ([`band_index`]), which keeps each tick linear in the population instead of quadratic.
//!
//! The court resizes with the window through [`COURT_COMMAND`] on the replayable command
//! channel, never by reading renderer state; see the note in `boids` for why that indirection
//! exists.

use fulcrum::prelude::*;

/// Court size at startup, and the area every resize preserves.
pub const DEFAULT_COURT: Vec2 = Vec2::new(1024.0, 768.0);
/// Aspect-ratio limits for a resize: a court much flatter than this stops being pong.
pub const ASPECT_LIMITS: (f32, f32) = (0.4, 3.2);
/// Name of the resize command on the replayable command channel.
pub const COURT_COMMAND: &str = "court";

/// Nothing shrinks below this. Not a population cap — a rendering one: a sprite well under a
/// pixel is indistinguishable from a missing sprite, and a court of invisible balls is a
/// worse simulation than a court of tiny ones.
pub const MIN_EXTENT: f32 = 1.0;

/// A lone ball's size. Balls are squares.
pub const BALL_SIZE_MAX: f32 = 22.0;
/// Every ball added multiplies the whole set's size by this. A ratio rather than a ramp,
/// because there is no final count to ramp toward — and because a constant ratio makes the
/// taper as visible between the first two balls as between the fiftieth and fifty-first.
pub const BALL_TAPER: f32 = 0.985;
/// Ball speed, units/second. Constant: every bounce re-normalizes to it.
pub const BALL_SPEED: f32 = 400.0;
/// A ball must keep at least this fraction of its speed horizontal, or a near-vertical one
/// would bounce between the top and bottom walls forever and never reach a paddle.
pub const MIN_HORIZONTAL: f32 = 0.35;
/// How much a hit off the end of a paddle bends the ball, as a fraction of speed.
pub const ENGLISH: f32 = 0.75;

/// Thickness (the short axis) of the opening pair of paddles.
pub const PADDLE_MAX_THICKNESS: f32 = 16.0;
/// Length of the opening pair of paddles.
pub const PADDLE_MAX_LENGTH: f32 = 130.0;
/// Every paddle added multiplies both dimensions of every paddle by this, so they stay in
/// proportion as the walls fill up.
pub const PADDLE_TAPER: f32 = 0.97;
/// Fraction of its band a paddle's length takes up, leaving a visible gap between neighbors.
/// This is a second, independent cap: a wall of `n` paddles gives each one `1/n` of the wall
/// no matter what the taper says.
pub const PADDLE_BAND_FILL: f32 = 0.8;
/// How far a paddle's center sits from its wall.
pub const PADDLE_INSET: f32 = 30.0;
/// Paddle speed before skill scaling, units/second.
pub const PADDLE_BASE_SPEED: f32 = 300.0;
/// The most warning a perfect-skill paddle acts on, in seconds. Lower skill reacts later.
pub const REACTION_WINDOW: f32 = 2.0;

/// Balls in play at startup.
pub const START_BALLS: u32 = 1;
/// Paddles at startup: one per side.
pub const START_PADDLES: u32 = 2;
/// A ball joins every this-many ticks of simulated time, indefinitely.
pub const BALL_EVERY: u64 = 300;
/// A paddle joins every this-many ticks of simulated time, indefinitely.
pub const PADDLE_EVERY: u64 = 540;

/// Ticks a spawn key must be held before it starts repeating.
pub const HOLD_DELAY: u32 = 18;
/// Ticks between arrivals once a held spawn key is repeating.
pub const HOLD_PERIOD: u32 = 4;

/// Slowest the court can run, as a multiple of real time.
pub const SPEED_MIN: f32 = 0.1;
/// Fastest the court can run. The ceiling is the integration, not the frame rate: at 8x a
/// ball crosses 53 units a tick, which the swept collision handles but the wall bounce
/// resolves ever more coarsely.
pub const SPEED_MAX: f32 = 8.0;
/// How much a held speed key multiplies (or divides) the rate each tick — about 3.3x per
/// second of holding.
pub const SPEED_RAMP: f32 = 1.02;

/// Which wall a paddle defends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// The -X wall.
    Left,
    /// The +X wall.
    Right,
}

impl Side {
    /// -1 for the left wall, +1 for the right.
    pub fn sign(self) -> f32 {
        match self {
            Side::Left => -1.0,
            Side::Right => 1.0,
        }
    }

    /// The wall a ball with this horizontal velocity is heading for.
    pub fn toward(velocity_x: f32) -> Self {
        if velocity_x < 0.0 {
            Side::Left
        } else {
            Side::Right
        }
    }
}

/// A paddle. Its geometry isn't stored: [`paddle_shape`] derives it from the slot and the
/// current population, so adding a paddle re-flows the whole wall without touching anyone.
#[derive(Component)]
pub struct Paddle {
    /// Which wall it defends.
    pub side: Side,
    /// Its index along that wall, from the bottom. Also its place in the wall's spectrum.
    pub slot: u32,
    /// 0..1. Scales both how fast it moves and how early it commits to an intercept.
    pub skill: f32,
}

/// A ball. `index` is its spawn order, which is all the view needs to spread the whole set
/// across the color spectrum.
#[derive(Component)]
pub struct Ball {
    /// Spawn order, `0..balls`. Stable for the ball's whole life.
    pub index: u32,
}

/// Simulation velocity, units/second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// The court, in world units. Simulation state, changed only by [`COURT_COMMAND`].
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Court(pub Vec2);

impl Default for Court {
    fn default() -> Self {
        Self(DEFAULT_COURT)
    }
}

/// Who is in play. Kept as a resource so spawning never has to count entities.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Census {
    /// Balls in play.
    pub balls: u32,
    /// Paddles on the left wall.
    pub left: u32,
    /// Paddles on the right wall.
    pub right: u32,
}

impl Census {
    /// Paddles on both walls.
    pub fn paddles(&self) -> u32 {
        self.left + self.right
    }

    /// Paddles on one wall.
    pub fn on(&self, side: Side) -> u32 {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }
}

/// What the run has produced. The closest thing to a score in a simulation nobody plays.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stats {
    /// Ticks elapsed since the run started (paused ticks don't count).
    pub ticks: u64,
    /// Balls returned by a paddle.
    pub saves: u64,
    /// Balls that got past everyone and were served again.
    pub misses: u64,
}

/// Space freezes the court.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Paused(pub bool);

/// How fast the court runs, as a multiple of real time. Simulation state like everything
/// else: it comes from tick-sampled input, so a replay reproduces the speed changes too.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Speed(pub f32);

impl Default for Speed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// How far this tick advances the simulation. Written once per tick by [`set_step`] and read
/// by everything that moves, so no two systems can disagree about how much time passed —
/// which matters for the swept collision, where the ball's previous position is reconstructed
/// from exactly this number.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Step {
    /// Simulated seconds this tick, already scaled by speed. Zero while paused.
    pub seconds: f32,
    /// Simulated ticks this tick: the speed multiplier, or zero while paused.
    pub scale: f32,
}

/// Fractional simulated ticks carried between real ticks when the speed isn't a whole number.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Clock(pub f32);

/// How long each spawn key has been held, for key repeat.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Holds {
    /// Ticks the ball key has been down.
    pub ball: u32,
    /// Ticks the paddle key has been down.
    pub paddle: u32,
}

/// The court a window of this pixel size should get: the window's aspect ratio at
/// [`DEFAULT_COURT`]'s area, so ball density stays put while the shape follows the window.
pub fn court_for_window(window: Vec2) -> Vec2 {
    let area = DEFAULT_COURT.x * DEFAULT_COURT.y;
    let aspect = (window.x / window.y).clamp(ASPECT_LIMITS.0, ASPECT_LIMITS.1);
    vec2(
        (area * aspect).sqrt().round(),
        (area / aspect).sqrt().round(),
    )
}

/// Encode a court size for [`COURT_COMMAND`]: whole units, so it round-trips exactly through
/// a replay.
pub fn court_payload(size: Vec2) -> String {
    format!("{} {}", size.x as i32, size.y as i32)
}

/// Decode a [`court_payload`]. `None` for anything malformed or degenerate.
pub fn parse_court(payload: &str) -> Option<Vec2> {
    let (width, height) = payload.split_once(' ')?;
    let size = vec2(
        width.trim().parse::<i32>().ok()? as f32,
        height.trim().parse::<i32>().ok()? as f32,
    );
    (size.x >= 1.0 && size.y >= 1.0).then_some(size)
}

/// `most` tapered by `ratio` once per arrival past `from`, floored at [`MIN_EXTENT`]. The
/// population has no ceiling, so this has no end point to interpolate toward: each arrival
/// simply costs the same fraction as the last.
pub fn taper(most: f32, ratio: f32, count: u32, from: u32) -> f32 {
    let arrivals = count.saturating_sub(from);
    (most * ratio.powi(arrivals as i32)).max(MIN_EXTENT)
}

/// How big a ball is with `balls` of them in play.
pub fn ball_size(balls: u32) -> f32 {
    taper(BALL_SIZE_MAX, BALL_TAPER, balls, START_BALLS)
}

/// Where a paddle may be and how big it is, given who else is on the court.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaddleShape {
    /// Center of the stretch of wall it owns.
    pub center: f32,
    /// Half its length, along the wall.
    pub half_length: f32,
    /// Half its thickness, across the wall.
    pub half_thickness: f32,
    /// Lowest and highest center it may slide to without leaving its own stretch.
    pub travel: (f32, f32),
}

/// A paddle's geometry. Two independent things shrink it, and the smaller wins: the
/// population as a whole (more paddles anywhere, smaller paddles everywhere) and its own
/// wall's crowding, since `n` paddles split that wall into `n` bands and none may outgrow its
/// share. The second is what makes a crowded wall read as a picket fence; the first is what
/// keeps paddles shrinking even on a wall that isn't the one filling up.
pub fn paddle_shape(court: Vec2, census: Census, side: Side, slot: u32) -> PaddleShape {
    let on_side = census.on(side).max(1);
    let band = court.y / on_side as f32;
    let center = -court.y / 2.0 + band * (slot as f32 + 0.5);
    let crowd = census.paddles();
    let length = (band * PADDLE_BAND_FILL)
        .min(taper(PADDLE_MAX_LENGTH, PADDLE_TAPER, crowd, START_PADDLES))
        .max(MIN_EXTENT);
    let half_length = length / 2.0;
    let half_thickness = taper(PADDLE_MAX_THICKNESS, PADDLE_TAPER, crowd, START_PADDLES) / 2.0;
    PaddleShape {
        center,
        half_length,
        half_thickness,
        travel: (
            center - band / 2.0 + half_length,
            center + band / 2.0 - half_length,
        ),
    }
}

/// Which of a wall's `count` bands a height falls in. This is the index that lets a ball find
/// the handful of paddles that could possibly return it without consulting the other
/// hundreds — and since every paddle's body stays inside its own band, checking the band a
/// ball crosses at plus its two neighbors is exhaustive.
pub fn band_index(court: Vec2, count: u32, y: f32) -> u32 {
    let count = count.max(1);
    let band = court.y / count as f32;
    let raw = ((y + court.y / 2.0) / band).floor();
    (raw.max(0.0) as u32).min(count - 1)
}

/// A paddle's distance from the court's center line.
pub fn paddle_x(court: Vec2, side: Side) -> f32 {
    side.sign() * (court.x / 2.0 - PADDLE_INSET)
}

/// Installs the rally simulation.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Court::default());
        app.world_mut().insert_resource(Census::default());
        app.world_mut().insert_resource(Stats::default());
        app.world_mut().insert_resource(Paused::default());
        app.world_mut().insert_resource(Speed::default());
        app.world_mut().insert_resource(Step::default());
        app.world_mut().insert_resource(Clock::default());
        app.world_mut().insert_resource(Holds::default());
        app.add_systems(Startup, open_court);
        app.add_systems(
            FixedUpdate,
            (
                apply_court,
                pace,
                set_step,
                population_controls,
                restart,
                grow,
                drive_paddles,
                move_balls,
                return_balls,
                serve_escapes,
            )
                .chain(),
        );
    }
}

/// Set up the opening position: one paddle per wall, one ball. Public so the binary can order
/// its sprite-attachment after it.
pub fn open_court(mut commands: Commands, mut rng: ResMut<SimRng>, mut census: ResMut<Census>) {
    deal(&mut commands, &mut rng, &mut census);
}

/// The opening position, used at startup and on restart.
fn deal(commands: &mut Commands, rng: &mut SimRng, census: &mut Census) {
    for _ in 0..START_BALLS {
        add_ball(commands, rng, census);
    }
    for index in 0..START_PADDLES {
        let side = if index % 2 == 0 {
            Side::Left
        } else {
            Side::Right
        };
        add_paddle(commands, rng, census, side);
    }
}

/// Serve a new ball from the center.
fn add_ball(commands: &mut Commands, rng: &mut SimRng, census: &mut Census) {
    let velocity = serve(rng);
    commands.spawn((
        Ball {
            index: census.balls,
        },
        Transform2D::default(),
        Velocity(velocity),
    ));
    census.balls += 1;
}

/// Put a paddle at the end of a wall. Everyone already on that wall gets a narrower band as a
/// result, which [`paddle_shape`] handles without touching them.
fn add_paddle(commands: &mut Commands, rng: &mut SimRng, census: &mut Census, side: Side) {
    let slot = census.on(side);
    let skill = rng.range_f32(0.55..1.0);
    // Spawned at the origin: `drive_paddles` runs later this same tick and puts it on its
    // wall, inside its band, before anything is drawn.
    commands.spawn((Paddle { side, slot, skill }, Transform2D::default()));
    match side {
        Side::Left => census.left += 1,
        Side::Right => census.right += 1,
    }
}

/// A serve: mostly horizontal, either direction, with a little vertical spread.
fn serve(rng: &mut SimRng) -> Vec2 {
    let angle = rng.range_f32(-0.45..0.45);
    let direction = if rng.chance(0.5) { 1.0 } else { -1.0 };
    vec2(direction * angle.cos(), angle.sin()) * BALL_SPEED
}

/// Hold a ball at [`BALL_SPEED`] and keep enough of it horizontal to stay in the rally.
fn normalize_ball(velocity: Vec2) -> Vec2 {
    let direction = velocity.try_normalize().unwrap_or(Vec2::X);
    let horizontal = direction.x.abs().max(MIN_HORIZONTAL);
    let vertical = (1.0 - horizontal * horizontal).max(0.0).sqrt();
    vec2(
        direction.x.signum() * horizontal,
        if direction.y < 0.0 {
            -vertical
        } else {
            vertical
        },
    ) * BALL_SPEED
}

/// Where a ball crossing to `target_x` will be vertically, accounting for wall bounces.
fn predict_y(position: Vec2, velocity: Vec2, target_x: f32, court: Vec2, ball: f32) -> f32 {
    let time = (target_x - position.x) / velocity.x;
    fold(position.y + velocity.y * time, court.y / 2.0 - ball / 2.0)
}

/// Reflect a coordinate back and forth inside `-half..half`, the way a bouncing ball folds.
fn fold(value: f32, half: f32) -> f32 {
    if half <= 0.0 {
        return 0.0;
    }
    let period = 4.0 * half;
    let phase = (value + half).rem_euclid(period);
    if phase <= 2.0 * half {
        phase - half
    } else {
        3.0 * half - phase
    }
}

/// Move the walls when a resize command arrives, and bring everyone inside the new court.
fn apply_court(
    mut court: ResMut<Court>,
    mut orders: EventReader<CommandEvent>,
    census: Res<Census>,
    mut balls: Query<&mut Transform2D, With<Ball>>,
) {
    let mut resized = false;
    for order in orders.read() {
        if order.name != COURT_COMMAND {
            continue;
        }
        if let Some(size) = parse_court(&order.payload) {
            court.0 = size;
            resized = true;
        }
    }
    if !resized {
        return;
    }
    let limit = court.0 / 2.0 - Vec2::splat(ball_size(census.balls));
    for mut ball in &mut balls {
        ball.translation = ball.translation.clamp(-limit, limit);
    }
}

/// Freeze the court, or change how fast it runs. Holding a speed key ramps continuously
/// rather than stepping, so you can dial in a rate by ear instead of hunting for a preset.
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

/// Fix this tick's simulated step. Pausing is just a step of zero, which is why nothing
/// downstream has to know what "paused" means.
fn set_step(mut step: ResMut<Step>, time: Res<Time>, speed: Res<Speed>, paused: Res<Paused>) {
    step.scale = if paused.0 { 0.0 } else { speed.0 };
    step.seconds = time.fixed_delta * step.scale;
}

/// Key repeat: fires the tick a key goes down, then again every [`HOLD_PERIOD`] ticks once it
/// has been held for [`HOLD_DELAY`]. A tap adds exactly one; a hold pours them in.
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

/// Poke the court: B adds a ball, P adds a paddle, and holding either keeps them coming.
/// Neither has a limit to bump against, and both work while paused.
fn population_controls(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    mut holds: ResMut<Holds>,
    input: Res<Input>,
) {
    if repeating(&mut holds.ball, input.pressed(Key::B)) {
        add_ball(&mut commands, &mut rng, &mut census);
    }
    if repeating(&mut holds.paddle, input.pressed(Key::P)) {
        let side = thinner_side(&census);
        add_paddle(&mut commands, &mut rng, &mut census, side);
    }
}

/// Everything on the court.
type Occupants<'w, 's> = Query<'w, 's, Entity, Or<(With<Ball>, With<Paddle>)>>;

/// R clears the court and deals the opening position again, statistics and all.
fn restart(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    mut stats: ResMut<Stats>,
    mut clock: ResMut<Clock>,
    input: Res<Input>,
    everyone: Occupants,
) {
    if !input.just_pressed(Key::R) {
        return;
    }
    for entity in &everyone {
        commands.entity(entity).despawn();
    }
    *census = Census::default();
    *stats = Stats::default();
    *clock = Clock::default();
    deal(&mut commands, &mut rng, &mut census);
}

/// The wall with fewer paddles, so the two sides stay in step.
fn thinner_side(census: &Census) -> Side {
    if census.left <= census.right {
        Side::Left
    } else {
        Side::Right
    }
}

/// The schedule that makes this a simulation rather than a match: population grows on the
/// clock, forever, with no involvement from anything that happens on the court.
///
/// The clock runs on simulated time, so the schedule speeds up and slows down with everything
/// else — a court at 4x fills four times as fast, and the elapsed time on the readout still
/// matches what the balls are doing.
fn grow(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    mut stats: ResMut<Stats>,
    mut clock: ResMut<Clock>,
    step: Res<Step>,
) {
    clock.0 += step.scale;
    while clock.0 >= 1.0 {
        clock.0 -= 1.0;
        stats.ticks += 1;
        if stats.ticks.is_multiple_of(BALL_EVERY) {
            add_ball(&mut commands, &mut rng, &mut census);
        }
        if stats.ticks.is_multiple_of(PADDLE_EVERY) {
            let side = thinner_side(&census);
            add_paddle(&mut commands, &mut rng, &mut census, side);
        }
    }
}

/// Every ball, read-only: what the paddles aim at.
type BallSnapshot<'w, 's> =
    Query<'w, 's, (&'static Transform2D, &'static Velocity), (With<Ball>, Without<Paddle>)>;

/// Every paddle plays the same way: watch your own band, and when a ball is predicted to
/// cross it inside your reaction window, slide to meet it. Skill scales both the speed and
/// the warning, so a crowded wall still leaks.
///
/// The intercepts are bucketed by band in one pass over the balls, so this costs
/// `balls + paddles` per tick rather than `balls × paddles` — which is what lets the
/// population grow without a ceiling.
fn drive_paddles(
    mut paddles: Query<(&Paddle, &mut Transform2D)>,
    balls: BallSnapshot,
    court: Res<Court>,
    census: Res<Census>,
    step: Res<Step>,
) {
    let ball_extent = ball_size(census.balls);
    // (arrival, predicted y) of the soonest ball heading for each band of each wall.
    let mut incoming: [Vec<Option<(f32, f32)>>; 2] = [
        vec![None; census.left.max(1) as usize],
        vec![None; census.right.max(1) as usize],
    ];
    for (ball, velocity) in &balls {
        let side = Side::toward(velocity.0.x);
        let count = census.on(side);
        if count == 0 {
            continue;
        }
        let wall_x = paddle_x(court.0, side);
        let arrival = (wall_x - ball.translation.x) / velocity.0.x;
        if arrival < 0.0 {
            continue;
        }
        let y = predict_y(ball.translation, velocity.0, wall_x, court.0, ball_extent);
        let bucket = &mut incoming[side as usize][band_index(court.0, count, y) as usize];
        if bucket.is_none_or(|(best, _)| arrival < best) {
            *bucket = Some((arrival, y));
        }
    }

    for (paddle, mut transform) in &mut paddles {
        let shape = paddle_shape(court.0, *census, paddle.side, paddle.slot);
        transform.translation.x = paddle_x(court.0, paddle.side); // a resize moves the wall
        let threat = incoming[paddle.side as usize]
            .get(paddle.slot as usize)
            .copied()
            .flatten();
        // Idle paddles drift back to the middle of their band rather than freezing wherever
        // the last rally left them.
        let target = match threat {
            Some((arrival, y)) if arrival <= REACTION_WINDOW * paddle.skill => y,
            _ => shape.center,
        };
        let target = target.clamp(shape.travel.0, shape.travel.1);
        let speed = PADDLE_BASE_SPEED * (0.7 + 0.6 * paddle.skill);
        let reach = (speed * step.seconds).min((target - transform.translation.y).abs());
        transform.translation.y += (target - transform.translation.y).signum() * reach;
        transform.translation.y = transform
            .translation
            .y
            .clamp(shape.travel.0, shape.travel.1);
    }
}

/// Balls fly and bounce off the top and bottom walls. The side walls are open — that's what
/// the paddles are for.
fn move_balls(
    mut balls: Query<(&mut Transform2D, &mut Velocity), With<Ball>>,
    court: Res<Court>,
    census: Res<Census>,
    step: Res<Step>,
) {
    let wall = court.0.y / 2.0 - ball_size(census.balls) / 2.0;
    for (mut transform, mut velocity) in &mut balls {
        transform.translation += velocity.0 * step.seconds;
        if transform.translation.y >= wall {
            transform.translation.y = wall;
            velocity.0.y = -velocity.0.y.abs();
        } else if transform.translation.y <= -wall {
            transform.translation.y = -wall;
            velocity.0.y = velocity.0.y.abs();
        }
    }
}

/// Paddle hits: reflect, add english from where along the paddle it landed, and re-normalize
/// so speed never creeps.
///
/// The test is **swept**, not an overlap check at the tick boundary. With no ceiling on the
/// population, paddles get thinner than a ball travels in one tick (6.7 units), and a
/// sampled test would let balls tunnel straight through — the return rate would quietly
/// collapse as the court filled. Instead each ball asks whether the segment it just traversed
/// crossed a paddle's face.
fn return_balls(
    mut balls: Query<(&mut Transform2D, &mut Velocity), With<Ball>>,
    paddles: Query<(&Paddle, &Transform2D), Without<Ball>>,
    court: Res<Court>,
    census: Res<Census>,
    step: Res<Step>,
    mut stats: ResMut<Stats>,
) {
    if step.seconds <= 0.0 {
        return; // paused: nothing moved, so nothing can have been struck
    }
    // Paddle heights by slot, so a ball can look up only the band it crossed.
    let mut wall: [Vec<f32>; 2] = [
        vec![f32::NAN; census.left as usize],
        vec![f32::NAN; census.right as usize],
    ];
    for (paddle, transform) in &paddles {
        if let Some(slot) = wall[paddle.side as usize].get_mut(paddle.slot as usize) {
            *slot = transform.translation.y;
        }
    }

    let ball = ball_size(census.balls);
    for (mut transform, mut velocity) in &mut balls {
        let side = Side::toward(velocity.0.x);
        let count = census.on(side);
        if count == 0 {
            continue;
        }
        let shape = paddle_shape(court.0, *census, side, 0);
        let reach = vec2(shape.half_thickness + ball / 2.0, ball / 2.0);
        // The face a ball meets first, and where it was before this tick's move.
        let face = paddle_x(court.0, side) - side.sign() * reach.x;
        let here = transform.translation;
        let before = here - velocity.0 * step.seconds;

        // Did the segment cross the face, or is the ball sitting on it right now?
        let crossed = (before.x - face) * side.sign() < 0.0 && (here.x - face) * side.sign() >= 0.0;
        let contact_y = if crossed {
            let travel = here.x - before.x;
            let fraction = if travel.abs() > f32::EPSILON {
                ((face - before.x) / travel).clamp(0.0, 1.0)
            } else {
                0.0
            };
            before.y + (here.y - before.y) * fraction
        } else if (here.x - face) * side.sign() >= 0.0
            && (here.x - paddle_x(court.0, side)) * side.sign() <= 0.0
        {
            // Sitting inside the paddle without having crossed its face this tick: a ball the
            // paddle slid into, or one a resize dropped there. Past the paddle's center line
            // it is a miss, not a hit — nothing gets returned from behind.
            here.y
        } else {
            continue;
        };

        // Only the crossed band and its neighbors can hold a paddle in reach: every paddle's
        // body stays inside its own band.
        let center_band = band_index(court.0, count, contact_y) as i64;
        let mut hit = None;
        for slot in (center_band - 1).max(0)..=(center_band + 1).min(count as i64 - 1) {
            let paddle_y = wall[side as usize][slot as usize];
            let shape = paddle_shape(court.0, *census, side, slot as u32);
            if (contact_y - paddle_y).abs() <= shape.half_length + reach.y {
                hit = Some((paddle_y, shape));
                break;
            }
        }
        let Some((paddle_y, shape)) = hit else {
            continue;
        };

        let offset = (contact_y - paddle_y) / (shape.half_length + reach.y);
        velocity.0 = normalize_ball(vec2(
            -side.sign() * velocity.0.x.abs(),
            velocity.0.y + offset * ENGLISH * BALL_SPEED,
        ));
        // Put it back on the face it struck, so the next tick starts clear of the paddle.
        transform.translation = vec2(face - side.sign() * 0.5, contact_y);
        stats.saves += 1;
    }
}

/// A ball that got past everyone is served again from the center — the population is set by
/// the schedule, so nothing is ever actually lost.
fn serve_escapes(
    mut balls: Query<(&mut Transform2D, &mut Velocity), With<Ball>>,
    court: Res<Court>,
    census: Res<Census>,
    step: Res<Step>,
    mut stats: ResMut<Stats>,
    mut rng: ResMut<SimRng>,
) {
    if step.seconds <= 0.0 {
        return; // paused
    }
    let out = court.0.x / 2.0 + ball_size(census.balls);
    for (mut transform, mut velocity) in &mut balls {
        if transform.translation.x.abs() <= out {
            continue;
        }
        transform.translation = Vec2::ZERO;
        velocity.0 = serve(&mut rng);
        stats.misses += 1;
    }
}
