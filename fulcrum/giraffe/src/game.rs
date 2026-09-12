//! Giraffe: a spatial prisoner's dilemma in which some of the players say what they need.
//!
//! The piece puts two bodies of work on one board and lets you watch whether they belong
//! together. One is evolutionary game theory: Nowak and May's lattice, where every cell plays
//! the prisoner's dilemma with its eight neighbours and then copies whichever of them did best.
//! The other is Marshall Rosenberg's Nonviolent Communication, whose claim is that most of
//! what looks like a conflict of interests is a conflict of *strategies*, and that two parties
//! who say what they need usually find the needs were not in conflict. The hypothesis this
//! piece is built to try is that the second is a move in the first: that saying what you need
//! is a strategy, that it has a cost, and that whether it spreads depends on how often needs
//! truly conflict. Four decisions carry that:
//!
//! **A cell is two choices, not one.** A *stance*, which is what it does when there is
//! something to take: share, or take. And a *voice*, which is whether it says what it needs
//! before the two of them act: a giraffe does, a jackal does not. Rosenberg's jackal is not
//! silent, but what it says is judgement and demand, which tells the other party nothing about
//! what it needs; for the arithmetic that is silence. See [`Strategy`].
//!
//! **Saying what you need costs something, and changes what game is being played.** A giraffe
//! pays [`Terms::cost`] in every encounter, heard or not: that is the exposure, and it
//! makes the disclosure believable. When two giraffes meet, the pair finds out which game they
//! are in. Some of the time the needs truly conflict and they play the dilemma they would have
//! played in silence. The rest of the time the needs were compatible all along, and both come away
//! with more than sharing would have given them: that is [`Terms::bounty`], the whole orange
//! each instead of half. How often needs truly conflict is [`Terms::conflict`], the dial the
//! whole piece turns on. See [`payoff`].
//!
//! **Nothing here is asserted.** The dial is under the scroll wheel. At one end needs do not
//! truly conflict, which is Rosenberg's world, and giraffes should sweep the board. At the other
//! they always do, and a giraffe is a cell paying to say something that changes nothing. The
//! piece does not say where the world is; it lets you set the dial and plant a patch of either
//! kind and watch. See [`touch`].
//!
//! **Left alone, it goes on.** A generation passes every [`Pace::period`] seconds. Now and
//! then a small patch of some strategy appears somewhere on its own, so the board does not
//! settle into a still picture and so a giraffe turns up eventually whether or not you plant
//! one. See [`drift`].
//!
//! Pure logic. No sprites, no colour, no entities: the board is one vector of strategies, so a
//! thousand generations run headless in a third of a second in release.

use fulcrum::prelude::*;

/// The board, in world units. The camera letterboxes this into whatever window there is.
pub const ARENA: Vec2 = Vec2::new(1280.0, 800.0);
/// World units to a cell.
pub const CELL: f32 = 8.0;
/// Cells across and down. The board is a torus: the right edge meets the left, the top the
/// bottom, so no cell has fewer neighbours than another and the edges are not a special place.
pub const ACROSS: u32 = 160;
pub const DOWN: u32 = 100;

/// What two sharers each get from one another. Nowak and May's R, which sets the scale.
pub const SHARE: f32 = 1.0;
/// What a taker gets from a sharer: the least the keys will set it to, the start, and the
/// most. Nowak and May's b, the temptation. Between one and two it is a dilemma: taking is
/// always the better answer to whatever the other does, and two takers get nothing. The start
/// is where a silent board neither freezes into sharers nor is swept by takers but keeps
/// churning between the two, which on eight neighbours with no self-play is a narrow band
/// around 1.6; `examples/scan.rs` is how it was found.
pub const TEMPTATION: (f32, f32, f32) = (1.0, 1.6, 2.0);
/// How often needs truly conflict at the start, in `0..=1`. Half: the piece has no opinion,
/// and half is the one figure that does not express one. On the starting terms a planted
/// patch of giraffes still grows from here, and stops growing a few notches of the wheel up.
pub const CONFLICT: f32 = 0.5;
/// What each party gets, over and above [`SHARE`], when both said what they needed and the
/// needs turned out to be compatible.
pub const BOUNTY: f32 = 0.5;
/// What saying what you need costs, per encounter, whether or not the other party listens.
pub const COST: f32 = 0.1;

/// How much of the starting board takes rather than shares. Nobody speaks at the start; the
/// first giraffe is yours to plant, or the board's to find.
pub const TAKER_FRACTION: f32 = 0.1;

/// Seconds between generations: the quickest, the starting pace, and the slowest.
pub const PERIOD: (f32, f32, f32) = (0.15, 0.8, 4.0);
/// How quickly a held pace key moves the period, as a fraction per second.
pub const PACE_RATE: f32 = 0.6;
/// How quickly a held temptation key moves it, per second.
pub const TEMPTATION_RATE: f32 = 0.15;
/// How far one notch of the scroll wheel moves the conflict dial, and how quickly the dial
/// follows: the board sees the change as a slide rather than a jump.
pub const DIAL_STEP: f32 = 0.05;
pub const DIAL_EASE: f32 = 3.0;

/// The radius of the patch a press plants, in cells, at the moment of the press and after it
/// has been held for [`PLANT_GROW`] seconds.
pub const PLANT_RADIUS: (f32, f32) = (1.6, 6.5);
pub const PLANT_GROW: f32 = 2.0;

/// A patch that appears on its own: how wide, in cells, and the shortest and longest wait
/// between one and the next, in seconds.
pub const DRIFT_RADIUS: f32 = 1.6;
pub const DRIFT_GAP: (f32, f32) = (6.0, 14.0);

/// What a cell does when there is something to take.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Stance {
    Share,
    Take,
}

/// Whether a cell says what it needs before the two of them act.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Voice {
    /// Says nothing, or says what it thinks of the other party, which comes to the same thing.
    Jackal,
    /// Says what it needs.
    Giraffe,
}

/// One cell's whole strategy. Copied as a unit when a neighbour does better.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Strategy {
    pub voice: Voice,
    pub stance: Stance,
}

impl Strategy {
    pub const fn new(voice: Voice, stance: Stance) -> Self {
        Self { voice, stance }
    }

    /// Every strategy, in [`index`](Self::index) order.
    pub const ALL: [Strategy; 4] = [
        Strategy::new(Voice::Jackal, Stance::Share),
        Strategy::new(Voice::Jackal, Stance::Take),
        Strategy::new(Voice::Giraffe, Stance::Share),
        Strategy::new(Voice::Giraffe, Stance::Take),
    ];

    /// The silent sharer, which the board starts as.
    pub const QUIET: Strategy = Self::ALL[0];
    /// The silent taker: what the right button plants.
    pub const JACKAL: Strategy = Self::ALL[1];
    /// The sharer who says what it needs: what the left button plants.
    pub const GIRAFFE: Strategy = Self::ALL[2];
    /// The taker who says what it needs: open about the need, and still takes.
    pub const HAWK: Strategy = Self::ALL[3];

    /// A number in `0..4`, jackals before giraffes and sharers before takers. The palettes are
    /// indexed by this.
    pub fn index(self) -> usize {
        (self.voice == Voice::Giraffe) as usize * 2 + (self.stance == Stance::Take) as usize
    }

    pub fn speaks(self) -> bool {
        self.voice == Voice::Giraffe
    }

    pub fn shares(self) -> bool {
        self.stance == Stance::Share
    }
}

/// The terms the board plays under.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Terms {
    /// How often two parties' needs truly conflict, in `0..=1`. The hypothesis dial.
    pub conflict: f32,
    /// What a taker gets from a sharer, against the [`SHARE`] two sharers each get.
    pub temptation: f32,
    /// What each party gets over and above [`SHARE`] when both spoke and the needs were
    /// compatible.
    pub bounty: f32,
    /// What speaking costs, per encounter.
    pub cost: f32,
}

impl Default for Terms {
    fn default() -> Self {
        Self {
            conflict: CONFLICT,
            temptation: TEMPTATION.1,
            bounty: BOUNTY,
            cost: COST,
        }
    }
}

impl Terms {
    /// The most any one encounter can pay, which brightness is measured against.
    pub fn ceiling(&self) -> f32 {
        self.temptation.max(SHARE + self.bounty)
    }
}

/// What `me` gets from one encounter with `other`.
///
/// The contest is Nowak and May's: two sharers get [`SHARE`] each, a taker gets the temptation
/// from a sharer who gets nothing, and two takers get nothing. Speaking costs [`Terms::cost`]
/// whoever is listening. When both speak, the pair finds out which game they are in: with
/// probability `1 - conflict` the needs were compatible and each gets [`SHARE`] plus the
/// bounty whatever their stance, since there is nothing to take; otherwise they play the
/// contest. The average of those is returned, so the board is deterministic and the
/// dial changes the whole board's terms at once rather than one encounter's luck.
pub fn payoff(terms: &Terms, me: Strategy, other: Strategy) -> f32 {
    let contest = match (me.stance, other.stance) {
        (Stance::Share, Stance::Share) => SHARE,
        (Stance::Share, Stance::Take) => 0.0,
        (Stance::Take, Stance::Share) => terms.temptation,
        (Stance::Take, Stance::Take) => 0.0,
    };
    match (me.voice, other.voice) {
        (Voice::Giraffe, Voice::Giraffe) => {
            let conflict = terms.conflict.clamp(0.0, 1.0);
            (1.0 - conflict) * (SHARE + terms.bounty) + conflict * contest - terms.cost
        }
        (Voice::Giraffe, Voice::Jackal) => contest - terms.cost,
        (Voice::Jackal, _) => contest,
    }
}

/// The eight neighbours, in the order they are consulted. Fixed, because a tie between two
/// neighbours goes to the one consulted first.
pub const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// The board: a strategy in every cell, and what each cell scored last generation.
#[derive(Resource, Clone, Debug)]
pub struct Field {
    /// The strategy in every cell, row by row, [`ACROSS`] to a row.
    pub cells: Vec<Strategy>,
    /// What every cell scored in the last generation: its payoff summed over its neighbours.
    pub score: Vec<f32>,
    /// Generations since each cell last changed strategy. Zero the generation it changed.
    pub age: Vec<u32>,
    /// Generations played.
    pub generation: u64,
    /// Bumped by every change, so a renderer can tell whether what it has is current.
    pub revision: u64,
    /// Scratch for the next generation, kept so that a generation allocates nothing.
    next: Vec<Strategy>,
}

impl Default for Field {
    fn default() -> Self {
        Self::new()
    }
}

/// Which strategy is coming and how long the pointer has been held, while a button is down.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Touch {
    pub held: Option<(Strategy, f32)>,
}

/// When the next generation is due, and how long one takes.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    /// Seconds between generations.
    pub period: f32,
    /// Seconds until the next one.
    pub due: f32,
    /// Nothing moves while this is set. Planting still works.
    pub paused: bool,
}

impl Default for Pace {
    fn default() -> Self {
        Self {
            period: PERIOD.1,
            due: PERIOD.1,
            paused: false,
        }
    }
}

impl Pace {
    /// How far through the current generation the board is, in `0..1`.
    pub fn phase(&self) -> f32 {
        (1.0 - self.due / self.period.max(1e-6)).clamp(0.0, 1.0)
    }
}

/// Where the conflict dial has been turned to. The board's own conflict figure slides toward
/// it.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Dial {
    pub wanted: f32,
}

impl Default for Dial {
    fn default() -> Self {
        Self { wanted: CONFLICT }
    }
}

/// When the next patch appears on its own.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Drift {
    pub due: f32,
}

impl Default for Drift {
    fn default() -> Self {
        Self { due: 8.0 }
    }
}

/// Smoothstep. Every ramp in the piece goes through this: a linear ramp has a corner at each
/// end, and a corner is a small event.
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The cell under a point in world units, as a fractional cell coordinate. The world's origin
/// is the middle of the board and its y axis points up; cell `(0, 0)` is the bottom-left.
pub fn cell_of(world: Vec2) -> Vec2 {
    (world + ARENA / 2.0) / CELL
}

/// The middle of cell `(x, y)`, in world units.
pub fn centre_of(x: u32, y: u32) -> Vec2 {
    vec2(x as f32 + 0.5, y as f32 + 0.5) * CELL - ARENA / 2.0
}

impl Field {
    /// A board of silent sharers. [`seed`](Self::seed) sprinkles the takers.
    pub fn new() -> Self {
        let cells = (ACROSS * DOWN) as usize;
        Self {
            cells: vec![Strategy::QUIET; cells],
            score: vec![0.0; cells],
            age: vec![0; cells],
            generation: 0,
            revision: 0,
            next: vec![Strategy::QUIET; cells],
        }
    }

    /// Cells on the board.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The index of cell `(x, y)`, wrapping either coordinate round the torus.
    pub fn index(x: i32, y: i32) -> usize {
        let x = x.rem_euclid(ACROSS as i32) as u32;
        let y = y.rem_euclid(DOWN as i32) as u32;
        (y * ACROSS + x) as usize
    }

    /// The cell `(x, y)` of an index.
    pub fn place(index: usize) -> (u32, u32) {
        let index = index as u32;
        (index % ACROSS, index / ACROSS)
    }

    /// The strategy at cell `(x, y)`, wrapping.
    pub fn at(&self, x: i32, y: i32) -> Strategy {
        self.cells[Self::index(x, y)]
    }

    /// How many cells hold each strategy, in [`Strategy::index`] order.
    pub fn tally(&self) -> [usize; 4] {
        let mut tally = [0; 4];
        for cell in &self.cells {
            tally[cell.index()] += 1;
        }
        tally
    }

    /// How many cells hold `strategy`.
    pub fn count(&self, strategy: Strategy) -> usize {
        self.tally()[strategy.index()]
    }

    /// What fraction of the board speaks.
    pub fn speaking(&self) -> f32 {
        let tally = self.tally();
        (tally[2] + tally[3]) as f32 / self.len().max(1) as f32
    }

    /// What fraction of the board shares.
    pub fn sharing(&self) -> f32 {
        let tally = self.tally();
        (tally[0] + tally[2]) as f32 / self.len().max(1) as f32
    }

    /// Start over: silent sharers, with takers sprinkled through them.
    pub fn seed(&mut self, rng: &mut SimRng) {
        for cell in &mut self.cells {
            *cell = if rng.chance(TAKER_FRACTION) {
                Strategy::JACKAL
            } else {
                Strategy::QUIET
            };
        }
        self.score.fill(0.0);
        self.age.fill(0);
        self.generation = 0;
        self.revision += 1;
    }

    /// Put `strategy` in every cell whose middle is within `radius` cells of `centre`, which
    /// is in cell coordinates. Wraps, like everything else on the board.
    pub fn plant(&mut self, centre: Vec2, radius: f32, strategy: Strategy) {
        let radius = radius.max(0.5);
        let reach = radius.ceil() as i32;
        let cx = centre.x.floor() as i32;
        let cy = centre.y.floor() as i32;
        for y in cy - reach..=cy + reach {
            for x in cx - reach..=cx + reach {
                let middle = vec2(x as f32 + 0.5, y as f32 + 0.5);
                if middle.distance_squared(centre) <= radius * radius {
                    let i = Self::index(x, y);
                    if self.cells[i] != strategy {
                        self.cells[i] = strategy;
                        self.age[i] = 0;
                    }
                }
            }
        }
        self.revision += 1;
    }

    /// One generation, all cells at once: every cell plays its eight neighbours, and then
    /// every cell takes up the strategy of whichever of the nine, itself included, scored
    /// highest. A tie goes to the cell's own strategy, and between neighbours to the one
    /// consulted first in [`NEIGHBOURS`].
    ///
    /// Synchronous on purpose, as Nowak and May had it. Updating cells one at a time in some
    /// order would make the order part of the rule.
    pub fn generation(&mut self, terms: &Terms) {
        let across = ACROSS as i32;
        let down = DOWN as i32;
        for y in 0..down {
            for x in 0..across {
                let i = Self::index(x, y);
                let me = self.cells[i];
                let mut total = 0.0;
                for (dx, dy) in NEIGHBOURS {
                    total += payoff(terms, me, self.at(x + dx, y + dy));
                }
                self.score[i] = total;
            }
        }
        for y in 0..down {
            for x in 0..across {
                let i = Self::index(x, y);
                let mut best = self.score[i];
                let mut choice = self.cells[i];
                for (dx, dy) in NEIGHBOURS {
                    let j = Self::index(x + dx, y + dy);
                    if self.score[j] > best {
                        best = self.score[j];
                        choice = self.cells[j];
                    }
                }
                self.next[i] = choice;
                self.age[i] = if choice == self.cells[i] {
                    self.age[i].saturating_add(1)
                } else {
                    0
                };
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        self.generation += 1;
        self.revision += 1;
    }
}

/// Installs the piece.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Field::default());
        app.world_mut().insert_resource(Terms::default());
        app.world_mut().insert_resource(Dial::default());
        app.world_mut().insert_resource(Pace::default());
        app.world_mut().insert_resource(Touch::default());
        app.world_mut().insert_resource(Drift::default());
        app.add_systems(Startup, sow);
        app.add_systems(FixedUpdate, (controls, dial, touch, drift, advance).chain());
    }
}

/// The first board.
fn sow(mut field: ResMut<Field>, mut rng: ResMut<SimRng>) {
    field.seed(&mut rng);
}

/// The keys. None of them can hurry a generation past the quickest pace.
fn controls(
    input: Res<Input>,
    time: Res<Time>,
    mut field: ResMut<Field>,
    mut terms: ResMut<Terms>,
    mut pace: ResMut<Pace>,
    mut rng: ResMut<SimRng>,
) {
    let dt = time.fixed_delta;
    // Quicker or slower generations. Held rather than stepped, so the pace slides.
    if input.pressed(Key::Up) {
        pace.period *= 1.0 - PACE_RATE * dt;
    }
    if input.pressed(Key::Down) {
        pace.period *= 1.0 + PACE_RATE * dt;
    }
    pace.period = pace.period.clamp(PERIOD.0, PERIOD.2);
    pace.due = pace.due.min(pace.period);
    // More or less temptation to take.
    if input.pressed(Key::Right) {
        terms.temptation += TEMPTATION_RATE * dt;
    }
    if input.pressed(Key::Left) {
        terms.temptation -= TEMPTATION_RATE * dt;
    }
    terms.temptation = terms.temptation.clamp(TEMPTATION.0, TEMPTATION.2);
    if input.just_pressed(Key::Space) {
        pace.paused = !pace.paused;
    }
    if input.just_pressed(Key::R) {
        field.seed(&mut rng);
    }
}

/// The scroll wheel turns the conflict dial, and the board's conflict figure slides after it.
fn dial(input: Res<Input>, time: Res<Time>, mut dial: ResMut<Dial>, mut terms: ResMut<Terms>) {
    dial.wanted = (dial.wanted + input.scroll_delta() * DIAL_STEP).clamp(0.0, 1.0);
    let gap = dial.wanted - terms.conflict;
    terms.conflict += gap * (DIAL_EASE * time.fixed_delta).min(1.0);
    if gap.abs() < 1e-4 {
        terms.conflict = dial.wanted;
    }
}

/// A press plants a patch: giraffes under the left button, jackals under the right. The patch
/// grows while the button is held and follows the pointer, so a drag paints a swathe.
fn touch(input: Res<Input>, time: Res<Time>, mut field: ResMut<Field>, mut held: ResMut<Touch>) {
    let dt = time.fixed_delta;
    let at = cell_of(input.mouse_world());
    let on_board = at.x >= 0.0 && at.y >= 0.0 && at.x < ACROSS as f32 && at.y < DOWN as f32;

    let pressed = if input.mouse_just_pressed(MouseButton::Left) {
        Some(Strategy::GIRAFFE)
    } else if input.mouse_just_pressed(MouseButton::Right) {
        Some(Strategy::JACKAL)
    } else {
        None
    };
    if let Some(strategy) = pressed {
        held.held = Some((strategy, 0.0));
    }

    let Some((strategy, seconds)) = held.held else {
        return;
    };
    let button = if strategy == Strategy::GIRAFFE {
        MouseButton::Left
    } else {
        MouseButton::Right
    };
    if !input.mouse_pressed(button) {
        held.held = None;
        return;
    }
    let seconds = seconds + dt;
    held.held = Some((strategy, seconds));
    if on_board {
        let radius =
            PLANT_RADIUS.0 + (PLANT_RADIUS.1 - PLANT_RADIUS.0) * ease(seconds / PLANT_GROW);
        field.plant(at, radius, strategy);
    }
}

/// A small patch of some strategy, now and then, somewhere. It keeps the board from settling
/// into a still picture, and it means a giraffe turns up eventually whether or not you plant
/// one.
fn drift(
    time: Res<Time>,
    mut drift: ResMut<Drift>,
    mut rng: ResMut<SimRng>,
    mut field: ResMut<Field>,
) {
    drift.due -= time.fixed_delta;
    if drift.due > 0.0 {
        return;
    }
    drift.due = rng.range_f32(DRIFT_GAP.0..DRIFT_GAP.1);
    let strategy = Strategy::ALL[rng.range_i32(0..4) as usize];
    let at = vec2(
        rng.range_f32(0.0..ACROSS as f32),
        rng.range_f32(0.0..DOWN as f32),
    );
    field.plant(at, DRIFT_RADIUS, strategy);
}

/// Play a generation when one is due.
fn advance(time: Res<Time>, terms: Res<Terms>, mut pace: ResMut<Pace>, mut field: ResMut<Field>) {
    if pace.paused {
        return;
    }
    pace.due -= time.fixed_delta;
    if pace.due > 0.0 {
        return;
    }
    pace.due += pace.period;
    field.generation(&terms);
}
