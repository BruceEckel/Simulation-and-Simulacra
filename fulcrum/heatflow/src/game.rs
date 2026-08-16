//! Statistical heat flow: a two-dimensional hard-disk gas between two thermal walls.
//!
//! The left and right walls are fixed-temperature reservoirs. An atom striking one is
//! re-emitted with a velocity drawn from the flux-weighted Maxwell distribution at that
//! wall's temperature — it forgets the energy it arrived with entirely. Atoms also collide
//! elastically with each other, which is what lets energy diffuse through the gas instead of
//! each atom shuttling independently between the surfaces. Set the two walls to different
//! temperatures and a conduction gradient establishes itself; the [`Profile`] resource is the
//! measurement of it, and [`Flux`] is the energy crossing each surface.
//!
//! Pure logic — no sprites, no audio — so it runs headless for the determinism test.
//!
//! **Units.** Atom mass is 1 and [`BOLTZMANN`] carries the whole unit system: it converts a
//! temperature into an energy, so `v_rms = sqrt(2 k T / m)`. In two dimensions a gas in
//! equilibrium at `T` has mean kinetic energy `k T` per atom (two degrees of freedom), which
//! is what [`temperature_of`] inverts. Atoms *leaving* a wall average `1.5 k T` rather than
//! `k T`, because fast atoms cross the surface more often than slow ones; that bias is
//! physical, and the bulk gas still settles at exactly the wall temperature.
//!
//! The court resizes with the window through [`COURT_COMMAND`] on the replayable command
//! channel, never by reading renderer state; see the note in `boids` for why.

use fulcrum::prelude::*;
use std::f32::consts::TAU;

/// Court size at startup, and the area every resize preserves.
pub const DEFAULT_COURT: Vec2 = Vec2::new(1024.0, 768.0);
/// Aspect-ratio limits for a resize.
pub const ASPECT_LIMITS: (f32, f32) = (0.4, 3.2);
/// Name of the resize command on the replayable command channel.
pub const COURT_COMMAND: &str = "court";

/// Boltzmann's constant in simulation units. It sets the whole speed scale: an atom in
/// equilibrium at temperature `T` has `v_rms = sqrt(2 k T / m)`, so at `T = 300` and `k = 45`
/// that is about 164 units per second.
pub const BOLTZMANN: f32 = 45.0;
/// Every atom weighs the same. Equal masses make an elastic collision a clean exchange of the
/// velocity components along the line of centers.
pub const ATOM_MASS: f32 = 1.0;
/// Atom radius. Physical, so it does *not* shrink as atoms are added — the collision cross
/// section is what sets the mean free path, and shrinking it would quietly change the physics
/// every time you added an atom.
pub const ATOM_RADIUS: f32 = 3.0;

/// Coldest a wall can be set to: absolute zero, which is a real setting rather than a guarded
/// one. A 0 K surface emits atoms with no velocity at all, so it absorbs everything that
/// reaches it and gives nothing back — atoms come to rest against it until a neighbour knocks
/// them loose. Against a warm surface that makes the sharpest gradient the box can hold; with
/// both surfaces there the gas freezes out, which is the correct answer and not a malfunction.
pub const TEMPERATURE_MIN: f32 = 0.0;
/// Cooling below this snaps to absolute zero, and heating away from zero starts here. A
/// proportional ramp can neither reach zero nor climb off it, so the bottom of the range needs
/// this one additive step to be reachable in both directions.
pub const ZERO_SNAP: f32 = 1.0;
/// Hottest a wall can be set to.
pub const TEMPERATURE_MAX: f32 = 1600.0;
/// Left wall's temperature at startup.
pub const LEFT_TEMPERATURE: f32 = 200.0;
/// Right wall's temperature at startup.
pub const RIGHT_TEMPERATURE: f32 = 1200.0;
/// The gas starts uniform at this temperature, so the gradient visibly establishes itself.
pub const START_TEMPERATURE: f32 = 400.0;
/// How much a held temperature key multiplies (or divides) a wall's temperature each tick.
pub const TEMPERATURE_RAMP: f32 = 1.02;

/// Atoms at startup. Dense enough that the mean free path is a fraction of the court, which
/// is what makes the gas conduct rather than merely shuttle.
pub const START_ATOMS: u32 = 700;
/// The most atoms a court can hold, in case a key sticks. Not a physical limit — a promise
/// that the tick stays interactive.
pub const MAX_ATOMS: u32 = 6000;

/// Ticks a held key must be down before it starts repeating.
pub const HOLD_DELAY: u32 = 12;
/// Ticks between arrivals once a held key is repeating.
pub const HOLD_PERIOD: u32 = 2;
/// Atoms added or removed per repeat, so a hold moves the population at a useful rate.
pub const HOLD_BATCH: u32 = 5;

/// Slowest the simulation can run, as a multiple of real time.
pub const SPEED_MIN: f32 = 0.1;
/// Fastest the simulation can run. Motion is sub-stepped, so collisions survive the ceiling.
pub const SPEED_MAX: f32 = 8.0;
/// How much a held speed key multiplies the rate each tick.
pub const SPEED_RAMP: f32 = 1.02;
/// The furthest an atom may travel in one integration sub-step, as a fraction of its radius.
/// Below one radius per sub-step, two atoms cannot swap places without their overlap being
/// seen, which is what keeps the collision rate honest at high speeds.
pub const SUBSTEP_TRAVEL: f32 = 1.0;
/// Hard cap on sub-steps per tick, so a pathological setting can't stall the loop.
pub const MAX_SUBSTEPS: u32 = 16;

/// Columns the temperature profile is measured in.
pub const PROFILE_BINS: usize = 48;
/// How quickly a profile bin follows its instantaneous sample. A gas of a few hundred atoms
/// is far too noisy to read raw; this averages over roughly a second.
pub const PROFILE_SMOOTHING: f32 = 0.02;
/// How quickly the reported wall flux follows its instantaneous value.
///
/// Slower than the profile's smoothing on purpose. A wall's *net* flux is the small difference
/// between two large gross flows — every strike absorbs one atom's energy and emits another
/// of order `1.5 k T` — so the raw signal is dominated by shot noise. Averaging over a few
/// hundred ticks is what turns it into a heat current you can read.
pub const FLUX_SMOOTHING: f32 = 0.004;

/// One atom.
#[derive(Component)]
pub struct Atom {
    /// Spawn order. Only used to decide which atom leaves when the population shrinks, so
    /// that removal is deterministic rather than dependent on storage order.
    pub index: u32,
}

/// Simulation velocity, units/second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// The box the gas is in, in world units. Simulation state, changed only by [`COURT_COMMAND`].
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Court(pub Vec2);

impl Default for Court {
    fn default() -> Self {
        Self(DEFAULT_COURT)
    }
}

/// The two reservoir temperatures. Fixed: a wall holds its temperature no matter how much
/// energy passes through it.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Walls {
    /// Temperature of the -X surface.
    pub left: f32,
    /// Temperature of the +X surface.
    pub right: f32,
}

impl Default for Walls {
    fn default() -> Self {
        Self {
            left: LEFT_TEMPERATURE,
            right: RIGHT_TEMPERATURE,
        }
    }
}

impl Walls {
    /// The temperature of one surface.
    pub fn of(&self, side: Side) -> f32 {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }
}

/// Which surface.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// The -X surface.
    Left,
    /// The +X surface.
    Right,
}

impl Side {
    /// -1 for the left surface, +1 for the right.
    pub fn sign(self) -> f32 {
        match self {
            Side::Left => -1.0,
            Side::Right => 1.0,
        }
    }
}

/// How many atoms are in the box.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Census {
    /// Atoms currently in the gas.
    pub atoms: u32,
    /// Atoms ever spawned, which is where the next index comes from.
    pub spawned: u32,
}

/// Everything the run measures: the temperature profile across the box, the heat crossing
/// each surface, and the event counts behind them.
///
/// One resource rather than three because every reading is taken from the same pass over the
/// gas and reset together on a restart — and because the systems that write them are already
/// carrying as many parameters as they should.
#[derive(Resource, Clone, Debug)]
pub struct Meter {
    /// Simulated seconds since the run started.
    pub elapsed: f32,
    /// Wall strikes so far.
    pub wall_hits: u64,
    /// Atom-atom collisions so far.
    pub collisions: u64,
    /// Smoothed energy per second entering the gas from the left surface. Positive means the
    /// wall is heating the gas; at steady state the two surfaces are equal and opposite, and
    /// the magnitude is the heat current.
    pub left_flux: f32,
    /// Smoothed energy per second entering the gas from the right surface.
    pub right_flux: f32,
    /// Energy exchanged at the left surface this tick, before smoothing.
    pub pending_left: f32,
    /// Energy exchanged at the right surface this tick, before smoothing.
    pub pending_right: f32,
    /// Smoothed temperature per column, left to right. This is the gradient.
    pub profile: [f32; PROFILE_BINS],
    /// Whether a column has ever held an atom. An unfilled column takes its first sample
    /// outright instead of easing up from zero.
    pub seen: [bool; PROFILE_BINS],
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            wall_hits: 0,
            collisions: 0,
            left_flux: 0.0,
            right_flux: 0.0,
            pending_left: 0.0,
            pending_right: 0.0,
            profile: [0.0; PROFILE_BINS],
            seen: [false; PROFILE_BINS],
        }
    }
}

impl Meter {
    /// Mean measured temperature over the columns that have seen atoms.
    pub fn mean_temperature(&self) -> f32 {
        let filled: Vec<f32> = self
            .profile
            .iter()
            .zip(self.seen)
            .filter(|(_, seen)| *seen)
            .map(|(temperature, _)| *temperature)
            .collect();
        if filled.is_empty() {
            0.0
        } else {
            filled.iter().sum::<f32>() / filled.len() as f32
        }
    }
}

/// Space freezes the gas.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Paused(pub bool);

/// How fast the simulation runs, as a multiple of real time.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Speed(pub f32);

impl Default for Speed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// How far this tick advances the simulation. Written once per tick by [`set_step`].
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Step {
    /// Simulated seconds this tick, already scaled by speed. Zero while paused.
    pub seconds: f32,
    /// The speed multiplier, or zero while paused.
    pub scale: f32,
}

/// How long each held key has been down, for key repeat.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Holds {
    /// Ticks the add-atoms key has been down.
    pub more: u32,
    /// Ticks the remove-atoms key has been down.
    pub fewer: u32,
}

/// The court a window of this pixel size should get: the window's aspect ratio at
/// [`DEFAULT_COURT`]'s area, so the gas density stays put while the shape follows the window.
pub fn court_for_window(window: Vec2) -> Vec2 {
    let area = DEFAULT_COURT.x * DEFAULT_COURT.y;
    let aspect = (window.x / window.y).clamp(ASPECT_LIMITS.0, ASPECT_LIMITS.1);
    vec2(
        (area * aspect).sqrt().round(),
        (area / aspect).sqrt().round(),
    )
}

/// Encode a court size for [`COURT_COMMAND`]: whole units, so it round-trips exactly.
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

/// Kinetic energy of an atom at this velocity.
pub fn kinetic_energy(velocity: Vec2) -> f32 {
    0.5 * ATOM_MASS * velocity.length_squared()
}

/// The temperature a mean kinetic energy corresponds to. Two degrees of freedom, so a gas in
/// equilibrium at `T` averages `k T` per atom.
pub fn temperature_of(mean_kinetic_energy: f32) -> f32 {
    mean_kinetic_energy / BOLTZMANN
}

/// A standard normal sample, by Box-Muller. Deterministic given the simulation RNG.
pub fn gaussian(rng: &mut SimRng) -> f32 {
    let radial = rng.range_f32(1e-6..1.0);
    let angular = rng.range_f32(0.0..TAU);
    (-2.0 * radial.ln()).sqrt() * angular.cos()
}

/// A velocity for an atom in equilibrium at `temperature`: both components Gaussian.
pub fn thermal_velocity(rng: &mut SimRng, temperature: f32) -> Vec2 {
    let scale = (BOLTZMANN * temperature / ATOM_MASS).sqrt();
    vec2(scale * gaussian(rng), scale * gaussian(rng))
}

/// A velocity for an atom *leaving* a surface at `temperature`, heading in `inward`.
///
/// Not the same distribution as [`thermal_velocity`]: the component along the surface normal
/// is flux-weighted (Rayleigh, not Gaussian), because a wall emits fast atoms more often than
/// slow ones in exactly the proportion it receives them. Emitted atoms average `1.5 k T`
/// against the bulk's `k T`, and using the bulk distribution here would leave the gas
/// measurably colder than the walls that are heating it.
pub fn emitted_velocity(rng: &mut SimRng, temperature: f32, inward: f32) -> Vec2 {
    let scale = (BOLTZMANN * temperature / ATOM_MASS).sqrt();
    let sample = rng.range_f32(1e-6..1.0);
    let normal = scale * (-2.0 * sample.ln()).sqrt();
    vec2(inward * normal, scale * gaussian(rng))
}

/// Resolve an elastic collision between two equal-mass disks, or `None` if they are not
/// touching or are already moving apart. Equal masses mean the velocity components along the
/// line of centers simply swap, which conserves both momentum and energy exactly.
pub fn collide(positions: (Vec2, Vec2), velocities: (Vec2, Vec2)) -> Option<(Vec2, Vec2)> {
    let offset = positions.1 - positions.0;
    let distance_squared = offset.length_squared();
    let contact = 2.0 * ATOM_RADIUS;
    if distance_squared > contact * contact || distance_squared < 1e-9 {
        return None;
    }
    let normal = offset / distance_squared.sqrt();
    let closing = (velocities.1 - velocities.0).dot(normal);
    if closing >= 0.0 {
        return None; // touching but already separating
    }
    let exchange = closing * normal;
    Some((velocities.0 + exchange, velocities.1 - exchange))
}

/// Which profile column an x position falls in.
pub fn profile_bin(court: Vec2, x: f32) -> usize {
    let fraction = (x + court.x / 2.0) / court.x;
    ((fraction * PROFILE_BINS as f32).floor().max(0.0) as usize).min(PROFILE_BINS - 1)
}

/// Installs the heat-flow simulation. Add **after** [`SpatialPlugin`], whose grid rebuild has
/// to run before the collision pass reads it.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Court::default());
        app.world_mut().insert_resource(Walls::default());
        app.world_mut().insert_resource(Census::default());
        app.world_mut().insert_resource(Meter::default());
        app.world_mut().insert_resource(Paused::default());
        app.world_mut().insert_resource(Speed::default());
        app.world_mut().insert_resource(Step::default());
        app.world_mut().insert_resource(Holds::default());
        app.add_systems(Startup, fill_box);
        app.add_systems(
            FixedUpdate,
            (
                apply_court,
                pace,
                set_step,
                wall_controls,
                population_controls,
                restart,
                simulate,
                measure,
            )
                .chain(),
        );
    }
}

/// Fill the box with a uniform gas. Public so the binary can order its sprite-attachment
/// after it.
pub fn fill_box(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    court: Res<Court>,
) {
    add_atoms(&mut commands, &mut rng, &mut census, court.0, START_ATOMS);
}

/// Put `count` atoms in the box at [`START_TEMPERATURE`], at random positions.
fn add_atoms(
    commands: &mut Commands,
    rng: &mut SimRng,
    census: &mut Census,
    court: Vec2,
    count: u32,
) {
    let limit = court / 2.0 - Vec2::splat(ATOM_RADIUS);
    for _ in 0..count.min(MAX_ATOMS.saturating_sub(census.atoms)) {
        let position = vec2(
            rng.range_f32(-limit.x..limit.x),
            rng.range_f32(-limit.y..limit.y),
        );
        commands.spawn((
            Atom {
                index: census.spawned,
            },
            SpatialIndexed,
            Transform2D::from_translation(position),
            Velocity(thermal_velocity(rng, START_TEMPERATURE)),
        ));
        census.atoms += 1;
        census.spawned += 1;
    }
}

/// Move the walls when a resize command arrives, and bring the gas inside the new box.
fn apply_court(
    mut court: ResMut<Court>,
    mut orders: EventReader<CommandEvent>,
    mut atoms: Query<&mut Transform2D, With<Atom>>,
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
    let limit = court.0 / 2.0 - Vec2::splat(ATOM_RADIUS);
    for mut atom in &mut atoms {
        atom.translation = atom.translation.clamp(-limit, limit);
    }
}

/// Freeze the gas, or change how fast it runs.
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

/// Fix this tick's simulated step. Pausing is a step of zero.
fn set_step(mut step: ResMut<Step>, time: Res<Time>, speed: Res<Speed>, paused: Res<Paused>) {
    step.scale = if paused.0 { 0.0 } else { speed.0 };
    step.seconds = time.fixed_delta * step.scale;
}

/// Q/A drive the left surface, E/D the right. Held keys ramp continuously, so you can dial a
/// temperature in rather than stepping to it.
///
/// S sets both surfaces to their average. A proportional ramp can approach equality but never
/// land on it — two temperatures multiplied by the same factor stay in the same ratio — and
/// "exactly equal" is the one setting worth being able to ask for exactly, since it is the
/// difference between a small heat current and none at all.
fn wall_controls(mut walls: ResMut<Walls>, input: Res<Input>) {
    if input.pressed(Key::Q) {
        walls.left = warmer(walls.left);
    }
    if input.pressed(Key::A) {
        walls.left = cooler(walls.left);
    }
    if input.pressed(Key::E) {
        walls.right = warmer(walls.right);
    }
    if input.pressed(Key::D) {
        walls.right = cooler(walls.right);
    }
    if input.just_pressed(Key::S) {
        let mean = (walls.left + walls.right) / 2.0;
        walls.left = mean;
        walls.right = mean;
    }
    walls.left = walls.left.clamp(TEMPERATURE_MIN, TEMPERATURE_MAX);
    walls.right = walls.right.clamp(TEMPERATURE_MIN, TEMPERATURE_MAX);
}

/// One tick of heating. Multiplying can never climb off zero, so leaving absolute zero is the
/// one additive step in the ramp.
pub fn warmer(temperature: f32) -> f32 {
    (temperature * TEMPERATURE_RAMP).max(ZERO_SNAP)
}

/// One tick of cooling. Dividing only approaches zero asymptotically, so the last stretch
/// snaps, and absolute zero is somewhere you can actually arrive.
pub fn cooler(temperature: f32) -> f32 {
    let cooled = temperature / TEMPERATURE_RAMP;
    if cooled < ZERO_SNAP { 0.0 } else { cooled }
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

/// Every atom and its spawn order.
type AtomIndex<'w, 's> = Query<'w, 's, (Entity, &'static Atom)>;

/// Hold N to pour atoms in, M to take them out. Removal takes the most recently added atoms,
/// so the population is a stack rather than a lottery.
fn population_controls(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    mut holds: ResMut<Holds>,
    court: Res<Court>,
    input: Res<Input>,
    atoms: AtomIndex,
) {
    if repeating(&mut holds.more, input.pressed(Key::N)) {
        add_atoms(&mut commands, &mut rng, &mut census, court.0, HOLD_BATCH);
    }
    if repeating(&mut holds.fewer, input.pressed(Key::M)) {
        let mut newest: Vec<(u32, Entity)> = atoms
            .iter()
            .map(|(entity, atom)| (atom.index, entity))
            .collect();
        newest.sort_unstable_by_key(|(index, _)| std::cmp::Reverse(*index));
        for (_, entity) in newest.into_iter().take(HOLD_BATCH as usize) {
            commands.entity(entity).despawn();
            census.atoms = census.atoms.saturating_sub(1);
        }
    }
}

/// R refills the box: a fresh uniform gas, cleared statistics, walls left as you set them.
fn restart(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    mut meter: ResMut<Meter>,
    court: Res<Court>,
    input: Res<Input>,
    existing: Query<Entity, With<Atom>>,
) {
    if !input.just_pressed(Key::R) {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    *census = Census::default();
    *meter = Meter::default();
    add_atoms(&mut commands, &mut rng, &mut census, court.0, START_ATOMS);
}

/// One integration tick: move the gas, thermalize whatever reached a wall, and resolve the
/// collisions between atoms.
///
/// Motion is sub-stepped so that no atom travels more than [`SUBSTEP_TRAVEL`] radii between
/// collision checks. Without that, running at 8x — or simply running hot — would let atoms
/// pass through each other, and the gas would quietly stop conducting.
fn simulate(
    mut atoms: Query<(Entity, &mut Transform2D, &mut Velocity), With<Atom>>,
    grid: Res<SpatialGrid>,
    court: Res<Court>,
    walls: Res<Walls>,
    step: Res<Step>,
    mut rng: ResMut<SimRng>,
    mut meter: ResMut<Meter>,
) {
    if step.seconds <= 0.0 {
        return;
    }

    // Work on a snapshot: a collision needs to write to both atoms at once, which a query
    // cannot hand out, and a flat array keeps the pass order-independent and cache-friendly.
    let mut gas: Vec<(Entity, Vec2, Vec2)> = atoms
        .iter()
        .map(|(entity, transform, velocity)| (entity, transform.translation, velocity.0))
        .collect();
    if gas.is_empty() {
        return;
    }
    let index: FxHashMap<Entity, usize> = gas
        .iter()
        .enumerate()
        .map(|(slot, (entity, _, _))| (*entity, slot))
        .collect();

    let fastest = gas
        .iter()
        .map(|(_, _, velocity)| velocity.length())
        .fold(0.0f32, f32::max);
    let travel = fastest * step.seconds;
    let substeps = ((travel / (ATOM_RADIUS * SUBSTEP_TRAVEL)).ceil() as u32).clamp(1, MAX_SUBSTEPS);
    let dt = step.seconds / substeps as f32;

    // Candidate neighbours, gathered once against the grid with a radius that covers the whole
    // tick's motion, then reused for every sub-step. Only pairs with the higher index are
    // kept, so each pair is considered exactly once.
    let search = 2.0 * ATOM_RADIUS + 2.0 * travel;
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (slot, (_, position, _)) in gas.iter().enumerate() {
        for other in grid.query_circle(*position, search) {
            if let Some(&neighbour) = index.get(&other)
                && neighbour > slot
            {
                pairs.push((slot, neighbour));
            }
        }
    }

    let limit = court.0 / 2.0 - Vec2::splat(ATOM_RADIUS);
    for _ in 0..substeps {
        for (_, position, velocity) in gas.iter_mut() {
            *position += *velocity * dt;

            // Top and bottom are mirrors: they turn atoms around without touching their energy.
            if position.y > limit.y {
                position.y = limit.y;
                velocity.y = -velocity.y.abs();
            } else if position.y < -limit.y {
                position.y = -limit.y;
                velocity.y = velocity.y.abs();
            }

            // The two thermal surfaces. An atom that reaches one forgets its energy and leaves
            // with a fresh one drawn at that surface's temperature — that exchange is the heat.
            // Inclusive, so an atom *resting* on a surface is offered a fresh velocity every
            // tick rather than only when it crosses the face. That matters at absolute zero:
            // a 0 K surface stops atoms dead against itself, and this is what lets them leave
            // again the moment the surface has any temperature to give them.
            let side = if position.x >= limit.x {
                Some(Side::Right)
            } else if position.x <= -limit.x {
                Some(Side::Left)
            } else {
                None
            };
            if let Some(side) = side {
                position.x = side.sign() * limit.x;
                let before = kinetic_energy(*velocity);
                *velocity = emitted_velocity(&mut rng, walls.of(side), -side.sign());
                let after = kinetic_energy(*velocity);
                match side {
                    Side::Left => meter.pending_left += after - before,
                    Side::Right => meter.pending_right += after - before,
                }
                // An atom that arrives with nothing and leaves with nothing has not been
                // struck; counting those would make a frozen box look frantic.
                if before > 0.0 || after > 0.0 {
                    meter.wall_hits += 1;
                }
            }
        }

        for &(a, b) in &pairs {
            let (position_a, velocity_a) = (gas[a].1, gas[a].2);
            let (position_b, velocity_b) = (gas[b].1, gas[b].2);
            let Some((after_a, after_b)) =
                collide((position_a, position_b), (velocity_a, velocity_b))
            else {
                continue;
            };
            gas[a].2 = after_a;
            gas[b].2 = after_b;
            // Ease them apart so the pair cannot stay locked together in a dense gas.
            let offset = position_b - position_a;
            let distance = offset.length().max(1e-6);
            let overlap = (2.0 * ATOM_RADIUS - distance) / 2.0;
            let push = offset / distance * overlap;
            gas[a].1 -= push;
            gas[b].1 += push;
            meter.collisions += 1;
        }
    }

    // Keyed by entity rather than by iteration order: the snapshot and the write-back are two
    // separate traversals, and nothing in the ECS contract says a query hands them out in the
    // same order twice.
    for (entity, mut transform, mut velocity) in &mut atoms {
        let Some(&slot) = index.get(&entity) else {
            continue;
        };
        transform.translation = gas[slot].1.clamp(-limit, limit);
        velocity.0 = gas[slot].2;
    }
    meter.elapsed += step.seconds;
}

/// Measure the gas: the temperature profile across the box, and the heat crossing each wall.
/// Both are smoothed, because a few hundred atoms make an instantaneous reading unreadable.
fn measure(
    atoms: Query<(&Transform2D, &Velocity), With<Atom>>,
    court: Res<Court>,
    step: Res<Step>,
    mut meter: ResMut<Meter>,
) {
    if step.seconds <= 0.0 {
        return;
    }

    let mut energy = [0.0f32; PROFILE_BINS];
    let mut population = [0u32; PROFILE_BINS];
    for (transform, velocity) in &atoms {
        let bin = profile_bin(court.0, transform.translation.x);
        energy[bin] += kinetic_energy(velocity.0);
        population[bin] += 1;
    }
    for bin in 0..PROFILE_BINS {
        if population[bin] == 0 {
            continue;
        }
        let sample = temperature_of(energy[bin] / population[bin] as f32);
        if meter.seen[bin] {
            meter.profile[bin] += (sample - meter.profile[bin]) * PROFILE_SMOOTHING;
        } else {
            meter.profile[bin] = sample;
            meter.seen[bin] = true;
        }
    }

    let left = meter.pending_left / step.seconds;
    let right = meter.pending_right / step.seconds;
    meter.left_flux += (left - meter.left_flux) * FLUX_SMOOTHING;
    meter.right_flux += (right - meter.right_flux) * FLUX_SMOOTHING;
    meter.pending_left = 0.0;
    meter.pending_right = 0.0;
}
