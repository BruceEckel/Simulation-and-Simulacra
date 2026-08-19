//! The rules: forty-four of them, in three families, all read by one engine.
//!
//! Conway's Life is one line of this table. What makes the rest of them reachable from the same
//! code is that they are all the same shape of rule: count the live cells in a neighbourhood,
//! and let that count decide what the middle cell does next. Change which counts give birth,
//! which counts let a cell survive, how many states a cell passes through on its way out, and
//! how wide the neighbourhood is, and the whole published zoo falls out.
//!
//! # The three families
//!
//! **Life-like** is the one everybody means. Eight neighbours, two states, and a rule written
//! `B3/S23`: the digits after `B` are the neighbour counts that bring a dead cell to life, and
//! the digits after `S` are the counts that let a live one stay. There are 2^18 of these and a
//! few dozen have been found worth naming.
//!
//! **Generations** adds one thing: a cell that fails its survival test does not go straight to
//! dead. It ages through `C - 2` further states first, and while it is doing so it is neither
//! alive nor empty — it does not count as a neighbour, but it is in the way. That single
//! addition is what turns Life's stillness into the perpetual motion of Brian's Brain, where
//! nothing whatever holds its place. Written `B2/S/3`.
//!
//! **Larger than Life** widens the neighbourhood instead. A radius of five is a hundred and
//! twenty-one cells rather than eight, the thresholds become bands rather than sets of digits,
//! and the middle cell may be counted along with its neighbours. What comes out looks organic
//! rather than mechanical: Bosco's rule, at radius five, has hollow crawling "bugs" where Life
//! has gliders. Written the way Golly writes it, `R5,C0,M1,S34..58,B34..45,NM`.
//!
//! # Where these came from
//!
//! The rulestrings are the published ones, from Mirek Wojtowicz's rule lexicon (the list behind
//! MCell, at <https://mcell.ca/>), LifeWiki (<https://conwaylife.com/wiki/>), Golly's Larger
//! than Life documentation, and Wikipedia's table of notable Life-like rules. Nothing here is
//! invented: a rule with a name has that name because somebody found something in it.
//!
//! One thing to watch when reading them elsewhere. MCell writes a Generations rule
//! survival-first, as `345/2/4`; Golly and LifeWiki write it birth-first, as `B2/S345/4`. They
//! are the same rule. This file uses the birth-first form throughout.

// ---------------------------------------------------------------------------------------
// the pieces a rule is made of
// ---------------------------------------------------------------------------------------

/// Which family a rule belongs to. Used for saying so in the readout and for the key that
/// jumps between them, so the table below is kept grouped in this order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    /// Eight neighbours, two states: the ordinary `B/S` rules.
    LifeLike,
    /// The same, with a cell ageing through further states instead of dying outright.
    Generations,
    /// A wider neighbourhood, and thresholds written as bands.
    LargerThanLife,
}

impl Family {
    /// Its name, for the readout.
    pub fn name(self) -> &'static str {
        match self {
            Family::LifeLike => "Life-like",
            Family::Generations => "Generations",
            Family::LargerThanLife => "Larger than Life",
        }
    }
}

/// The shape of the neighbourhood a rule counts over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// The square block of the given radius around the cell. Radius one is the ordinary eight.
    Moore(u32),
    /// The four cells sharing an edge.
    VonNeumann,
}

impl Shape {
    /// How many cells it holds, the middle one included.
    pub fn cells(self) -> u32 {
        match self {
            Shape::Moore(radius) => (2 * radius + 1) * (2 * radius + 1),
            Shape::VonNeumann => 5,
        }
    }
}

/// A set of neighbour counts: the thing a rule tests against.
///
/// Two forms, because the two notations are genuinely different. A Life-like rule names its
/// counts one digit at a time and they need not be contiguous — `B3678` has a hole at 4 and 5.
/// A Larger than Life rule counts up to several hundred neighbours and states a band.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Counts {
    /// The digits of a rulestring, as a bitmask over the counts `0..32`.
    Set(u32),
    /// An inclusive band, low and high.
    Range(u32, u32),
}

impl Counts {
    /// Does this count pass?
    #[inline]
    pub fn holds(self, count: u32) -> bool {
        match self {
            Counts::Set(mask) => count < 32 && mask & (1 << count) != 0,
            Counts::Range(low, high) => count >= low && count <= high,
        }
    }

    /// How it is written in a rulestring. Empty for a set with nothing in it, which is how
    /// Seeds and Brian's Brain say that nothing survives.
    pub fn written(self) -> String {
        match self {
            // Only 0..=8 can occur: every rule written with digits here has eight neighbours.
            Counts::Set(mask) => (0..9u32)
                .filter(|count| mask & (1 << count) != 0)
                .map(|count| char::from_digit(count, 10).expect("a single digit"))
                .collect(),
            Counts::Range(low, high) => format!("{low}..{high}"),
        }
    }
}

/// The digits of a rulestring as a bitmask. `digits("3678")` is the set {3, 6, 7, 8}.
///
/// A `const fn` so the table below can be written the way the rules are published, with the
/// digits in quotes, and still be a compile-time constant with nothing parsed at startup.
const fn digits(text: &str) -> u32 {
    let bytes = text.as_bytes();
    let mut mask = 0;
    let mut index = 0;
    while index < bytes.len() {
        mask |= 1 << (bytes[index] - b'0');
        index += 1;
    }
    mask
}

/// How a rule likes to be started.
///
/// Every rule carries its own, because the right first frame is not the same for all of them.
/// A field of Seeds seeded at a third full detonates and is over before you have looked at it;
/// Gnarl wants exactly one live cell and nothing else; Day & Night is unchanged by swapping
/// live for dead and so wants a soup at one half, the only density that does not prefer one of
/// its two phases.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Seeding {
    /// Random cells over the whole field, at the given density.
    Soup(f32),
    /// Random cells inside a centred square, at the given density, the square sized as a
    /// fraction of the shorter side of the field.
    Patch(f32, f32),
    /// One live cell in the middle.
    Spark,
    /// A solid square of the given side in the middle.
    Block(u32),
}

// ---------------------------------------------------------------------------------------
// a rule
// ---------------------------------------------------------------------------------------

/// One rule, and everything needed to run it and to say what it is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rule {
    /// What it is called, which is the name it was published under.
    pub name: &'static str,
    /// Which family it belongs to.
    pub family: Family,
    /// One line about what you are looking at.
    pub blurb: &'static str,
    /// The neighbourhood it counts over.
    pub shape: Shape,
    /// Whether the middle cell is counted along with its neighbours. Larger than Life's `M1`;
    /// false for every rule written in `B`/`S` digits.
    pub centre: bool,
    /// Counts that bring a dead cell to life.
    pub birth: Counts,
    /// Counts that let a live cell stay live.
    pub survive: Counts,
    /// How many states a cell has. Two is dead and alive; more adds that many stages of dying
    /// in between, which is what Generations means.
    pub states: u32,
    /// How it likes to be started.
    pub seed: Seeding,
}

impl Rule {
    /// A two-state rule over the ordinary eight neighbours, written as its `B` and `S` digits.
    const fn life(
        name: &'static str,
        birth: &'static str,
        survive: &'static str,
        seed: Seeding,
        blurb: &'static str,
    ) -> Self {
        Self {
            name,
            family: Family::LifeLike,
            blurb,
            shape: Shape::Moore(1),
            centre: false,
            birth: Counts::Set(digits(birth)),
            survive: Counts::Set(digits(survive)),
            states: 2,
            seed,
        }
    }

    /// The same, with `states` counting the stages of dying as well as dead and alive.
    const fn generations(
        name: &'static str,
        birth: &'static str,
        survive: &'static str,
        states: u32,
        seed: Seeding,
        blurb: &'static str,
    ) -> Self {
        Self {
            name,
            family: Family::Generations,
            blurb,
            shape: Shape::Moore(1),
            centre: false,
            birth: Counts::Set(digits(birth)),
            survive: Counts::Set(digits(survive)),
            states,
            seed,
        }
    }

    /// A Larger than Life rule, in the order Golly writes it: the neighbourhood, how many
    /// states, whether the middle cell counts, the survival band, and the birth band.
    #[expect(
        clippy::too_many_arguments,
        reason = "a rulestring has this many parts"
    )]
    const fn larger(
        name: &'static str,
        shape: Shape,
        states: u32,
        centre: bool,
        survive: (u32, u32),
        birth: (u32, u32),
        seed: Seeding,
        blurb: &'static str,
    ) -> Self {
        Self {
            name,
            family: Family::LargerThanLife,
            blurb,
            shape,
            centre,
            birth: Counts::Range(birth.0, birth.1),
            survive: Counts::Range(survive.0, survive.1),
            states,
            seed,
        }
    }

    /// The rule as it is published, in the notation its family is published in.
    pub fn rulestring(&self) -> String {
        match self.family {
            Family::LifeLike => format!("B{}/S{}", self.birth.written(), self.survive.written()),
            Family::Generations => format!(
                "B{}/S{}/{}",
                self.birth.written(),
                self.survive.written(),
                self.states
            ),
            Family::LargerThanLife => {
                let (radius, neighbourhood) = match self.shape {
                    Shape::Moore(radius) => (radius, 'M'),
                    Shape::VonNeumann => (1, 'N'),
                };
                format!(
                    "R{radius},C{},M{},S{},B{},N{neighbourhood}",
                    // Golly writes a two-state rule as C0 rather than C2.
                    if self.states == 2 { 0 } else { self.states },
                    u32::from(self.centre),
                    self.survive.written(),
                    self.birth.written(),
                )
            }
        }
    }

    /// The largest count this rule can ever see: the whole neighbourhood, less the middle cell
    /// when the middle cell does not count.
    pub fn ceiling(&self) -> u32 {
        self.shape.cells() - u32::from(!self.centre)
    }
}

// ---------------------------------------------------------------------------------------
// the table
// ---------------------------------------------------------------------------------------

/// Every rule, grouped by family and in the order the keys walk them.
///
/// The order inside a family is roughly "start here": Life first, then the rules nearest to it,
/// and the ones that behave least like it at the end.
pub const RULES: &[Rule] = &[
    // -----------------------------------------------------------------------------------
    // Life-like: eight neighbours, two states
    // -----------------------------------------------------------------------------------
    Rule::life(
        "Life",
        "3",
        "23",
        Seeding::Soup(0.32),
        "Conway's own: born on three, staying on two or three. Still lifes, blinkers, gliders.",
    ),
    Rule::life(
        "HighLife",
        "36",
        "23",
        Seeding::Soup(0.32),
        "Life with one digit added, and that digit buys a replicator that copies itself.",
    ),
    Rule::life(
        "Day & Night",
        "3678",
        "34678",
        Seeding::Soup(0.5),
        "Unchanged if you swap live for dead: the two phases are one rule, and both are busy.",
    ),
    Rule::life(
        "DryLife",
        "37",
        "23",
        Seeding::Soup(0.32),
        "Life again, plus birth on seven: the same small objects in a coarser, drier field.",
    ),
    Rule::life(
        "Pseudo Life",
        "357",
        "238",
        Seeding::Soup(0.32),
        "One digit from Life in each half, and nothing of Life survives it. No glider lives here.",
    ),
    Rule::life(
        "2x2",
        "36",
        "125",
        Seeding::Soup(0.32),
        "A pattern built of two-by-two blocks stays built of them, however far it runs.",
    ),
    Rule::life(
        "Move",
        "368",
        "245",
        Seeding::Soup(0.32),
        "Slow spaceships with enormous periods: things crawl here rather than fly.",
    ),
    Rule::life(
        "Long Life",
        "345",
        "5",
        Seeding::Soup(0.32),
        "Survival on five alone. Sparse, deliberate, and very slow to settle.",
    ),
    Rule::life(
        "Maze",
        "3",
        "12345",
        Seeding::Patch(0.5, 0.03),
        "Life's births with survival almost everywhere: a speck grows corridors and dead ends.",
    ),
    Rule::life(
        "Mazectric",
        "3",
        "1234",
        Seeding::Patch(0.5, 0.03),
        "The same maze one digit tighter, which straightens the halls and lengthens them.",
    ),
    Rule::life(
        "Coral",
        "3",
        "45678",
        Seeding::Patch(0.5, 0.05),
        "Survival only when crowded, so all the growth is at the rim: a slow fingered crust.",
    ),
    Rule::life(
        "Flakes",
        "3",
        "012345678",
        Seeding::Patch(0.35, 0.04),
        "Life without death: nothing that lives ever stops, and the field crystallises.",
    ),
    Rule::life(
        "Coagulations",
        "378",
        "235678",
        Seeding::Soup(0.32),
        "A stain that spreads and does not lift, thickening wherever it has already been.",
    ),
    Rule::life(
        "Assimilation",
        "345",
        "4567",
        Seeding::Soup(0.5),
        "Continents with rounded coasts that reach out, meet, and merge into one another.",
    ),
    Rule::life(
        "Walled Cities",
        "45678",
        "2345",
        Seeding::Soup(0.5),
        "Districts that build a wall around themselves and then keep moving inside it.",
    ),
    Rule::life(
        "Diamoeba",
        "35678",
        "5678",
        Seeding::Soup(0.5),
        "Chaotic diamonds with straight diagonal edges and a boiling interior.",
    ),
    Rule::life(
        "Amoeba",
        "357",
        "1358",
        Seeding::Soup(0.32),
        "A chaotic sponge, balanced between filling the field and dying out of it.",
    ),
    Rule::life(
        "Anneal",
        "4678",
        "35678",
        Seeding::Soup(0.5),
        "A majority vote: every boundary shortens, and a soup rounds itself into domains.",
    ),
    Rule::life(
        "Seeds",
        "2",
        "",
        Seeding::Patch(0.25, 0.06),
        "Nothing survives its own generation, so every pattern is a phoenix. Explosive.",
    ),
    Rule::life(
        "Serviettes",
        "234",
        "",
        Seeding::Block(2),
        "Nothing survives here either, and a single block draws a Persian rug out of it.",
    ),
    Rule::life(
        "Replicator",
        "1357",
        "1357",
        Seeding::Spark,
        "Odd counts only. Whatever you draw is eventually replaced by copies of itself.",
    ),
    Rule::life(
        "Gnarl",
        "1",
        "1",
        Seeding::Spark,
        "One neighbour, and only one. From a single cell a gnarled fractal grows outwards.",
    ),
    // -----------------------------------------------------------------------------------
    // Generations: the same eight neighbours, and a cell that ages instead of dying
    // -----------------------------------------------------------------------------------
    Rule::generations(
        "Brian's Brain",
        "2",
        "",
        3,
        Seeding::Soup(0.15),
        "On, dying, off. Nothing holds still for one generation: the field is all wavefront.",
    ),
    Rule::generations(
        "Star Wars",
        "2",
        "345",
        4,
        Seeding::Soup(0.15),
        "Brian's Brain given something to stand on: guns hold their ground and fire photons.",
    ),
    Rule::generations(
        "Fireworks",
        "13",
        "2",
        21,
        Seeding::Patch(0.2, 0.15),
        "Twenty-one states of fading, so every spark leaves a long slow trail hanging behind it.",
    ),
    Rule::generations(
        "Faders",
        "2",
        "2",
        25,
        Seeding::Soup(0.15),
        "Almost nothing is alive at any moment, and the whole picture is what has just died.",
    ),
    Rule::generations(
        "Frogs",
        "34",
        "12",
        3,
        Seeding::Soup(0.3),
        "Three states and a tight survival band: small hopping colonies that never settle.",
    ),
    Rule::generations(
        "Prairie on Fire",
        "34",
        "345",
        6,
        Seeding::Soup(0.3),
        "A grass fire. The front burns outward, and the ground behind it takes six states to cool.",
    ),
    Rule::generations(
        "Lava",
        "45678",
        "12345",
        8,
        Seeding::Soup(0.4),
        "Birth only where it is already crowded: slow molten fronts under a cooling crust.",
    ),
    Rule::generations(
        "Burst",
        "3468",
        "0235678",
        9,
        Seeding::Soup(0.3),
        "Survival even on zero, so a colony holds together, swells, and breaks open.",
    ),
    Rule::generations(
        "Rake",
        "2678",
        "3467",
        6,
        Seeding::Soup(0.3),
        "Named for what it makes: guns that travel while they fire, laying a trail of ships.",
    ),
    Rule::generations(
        "Caterpillars",
        "378",
        "124567",
        4,
        Seeding::Soup(0.3),
        "Long segmented things that crawl over the field and over one another.",
    ),
    Rule::generations(
        "Bloomerang",
        "34678",
        "234",
        24,
        Seeding::Soup(0.3),
        "Twenty-four states behind a narrow survival band: long tails wandering the field.",
    ),
    Rule::generations(
        "Wanderers",
        "34678",
        "345",
        5,
        Seeding::Soup(0.3),
        "The same births as Bloomerang, five states rather than twenty-four, and it travels.",
    ),
    Rule::generations(
        "Swirl",
        "34",
        "23",
        8,
        Seeding::Soup(0.3),
        "Life's survival rule with eight states stacked behind it, which sets the field turning.",
    ),
    Rule::generations(
        "Banners",
        "3457",
        "2367",
        5,
        Seeding::Soup(0.3),
        "Broad flat sheets with fraying edges, trailing five states of colour behind them.",
    ),
    Rule::generations(
        "Xtasy",
        "2356",
        "1456",
        16,
        Seeding::Soup(0.3),
        "Sixteen states over a wide birth band: dense, loud, and never twice the same.",
    ),
    // -----------------------------------------------------------------------------------
    // Larger than Life: a wider neighbourhood, and bands instead of digits
    // -----------------------------------------------------------------------------------
    Rule::larger(
        "Bugs",
        Shape::Moore(5),
        2,
        true,
        (34, 58),
        (34, 45),
        Seeding::Soup(0.4),
        "Bosco's rule, at radius five. Where Life has gliders, this has hollow crawling bugs.",
    ),
    Rule::larger(
        "Majority",
        Shape::Moore(4),
        2,
        true,
        (41, 81),
        (41, 81),
        Seeding::Soup(0.5),
        "A vote of eighty-one cells, birth and survival alike: a soup anneals into domains.",
    ),
    Rule::larger(
        "Waffle",
        Shape::Moore(7),
        2,
        true,
        (100, 200),
        (75, 170),
        Seeding::Soup(0.4),
        "Radius seven, births easier than survival: it grows a slab and then eats holes in it.",
    ),
    Rule::larger(
        "Globe",
        Shape::Moore(8),
        2,
        false,
        (163, 223),
        (74, 252),
        Seeding::Soup(0.4),
        "A very wide birth band under a narrow survival one: long banded horizontal shapes.",
    ),
    Rule::larger(
        "Bugsmovie",
        Shape::Moore(10),
        2,
        true,
        (123, 212),
        (123, 170),
        Seeding::Soup(0.4),
        "Bugs again at twice the radius: four hundred and forty-one cells decide every cell.",
    ),
    Rule::larger(
        "Modern Art",
        Shape::Moore(10),
        255,
        true,
        (2, 3),
        (3, 3),
        Seeding::Patch(0.001, 0.5),
        "Two hundred and fifty-five states over a neighbourhood of four hundred and forty-one.",
    ),
    Rule::larger(
        "Gnarl (von Neumann)",
        Shape::VonNeumann,
        2,
        true,
        (1, 1),
        (1, 1),
        Seeding::Spark,
        "Gnarl over the four orthogonal neighbours: the same fractal, squared off.",
    ),
];

/// The rule the piece opens on, which is the one everybody came for.
pub const OPENING: usize = 0;

/// The first rule of the family after this one, wrapping round. What the family key does.
pub fn next_family(current: usize) -> usize {
    let here = RULES[current % RULES.len()].family;
    let count = RULES.len();
    for step in 1..=count {
        let index = (current + step) % count;
        let family = RULES[index].family;
        if family == here {
            continue;
        }
        // Walk back to the first rule of that family, so the key always lands on its opening
        // rule rather than wherever the scan happened to cross the boundary.
        let mut first = index;
        while first > 0 && RULES[first - 1].family == family {
            first -= 1;
        }
        return first;
    }
    current
}
