//! A skeleton on a string, and nothing else.
//!
//! Eighteen bones hang off a pelvis. The pelvis is the only thing in here that is *told* what to
//! do: it traces a small closed curve, a hip's worth of movement, at whatever tempo you set.
//! Every other bone is a rigid rod free to swing about the joint it hangs from, and the whole
//! body is one system of coupled pendulums, solved together.
//!
//! Take a single bone with nothing hanging off it, and it obeys [`swing`]:
//!
//! ```text
//!   θ¨ = −(3 / 2L) · [ (g + a_y)·sin θ + a_x·cos θ ]
//!          ╰── a rod pivoted at one end ──╯
//! ```
//!
//! A rod pivoted at one end is a pendulum of effective length `2L/3`; `a` is the acceleration of
//! the joint it hangs from, which arrives as a fictitious force exactly as it does in a lift or
//! on the end of a shaken arm. That second term is the only way the dance ever reaches a bone.
//!
//! Every other bone in a body has things hanging off it, and they pull back. A forearm flung
//! outwards tugs on the upper arm; the upper arm tugs on the collar; the collar tugs on the
//! chest. That is what makes this a *coupled* system rather than eighteen separate ones, and it
//! is where the dance comes from: [`Frame`] is the body's inertia written as a matrix, and every
//! step solves `M θ¨ = r` for all eighteen angles at once. Two pendulums coupled like this are
//! the standard example of a chaotic system. This one has eighteen.
//!
//! Two things worth knowing before you look at the equation:
//!
//! **Standing up is not free.** A spine is an *inverted* pendulum: gravity is not holding it
//! there, it is trying to fold it. Tone — the torsion spring at each joint — is what holds the
//! pose, and it is scaled by exactly the weight each joint carries, so [`Tone`] `1.0` is the
//! least tone at which a plain joint can hold its own limb up. Under one the skeleton cannot
//! stand, and does not.
//!
//! A *stack* of such joints is weaker than any one of them, because they can all bow the same
//! way at once, and the trunk here is four bones in a row. That is why the trunk's joints are
//! given several times the tone of a limb's — see [`BoneSpec::firm`] — and it is a fact about
//! standing up rather than a fudge: a spine really is held far more firmly than a shoulder, and
//! a body built the other way round could not do either job.
//!
//! **A shaken pivot can stand a pendulum up.** Bob a pivot fast enough and *upside down* becomes
//! a place a pendulum will sit — Kapitza's pendulum, and one of the stranger results in
//! classical mechanics. The condition is `(Aω)² > 2·g·(2L/3)`; both sides of it are on the
//! readout, and the SHIVER step is there because it crosses it. The skeleton throws its arms in
//! the air, and it should be clear that physics is doing that and not a choreographer.
//!
//! Pure logic — no sprites, no colour. It reports where every bone is and which joints just
//! banged into their stops; the binary decides what a bone looks like and what a knock sounds
//! like.

use fulcrum::prelude::*;
use std::f32::consts::TAU;

// ---------------------------------------------------------------------------------------
// the world the bones live in
// ---------------------------------------------------------------------------------------

/// World units to a metre. The skeleton is a hair under six feet, which fixes the scale of
/// everything else in here.
pub const PER_METRE: f32 = 222.0;

/// Gravity, in world units per second squared.
///
/// Real gravity at the scale above, and not a number tuned until the swings looked nice. That
/// is the point: a bone of an arm's length then swings with an arm's period, because it is the
/// same pendulum. Fake the gravity and the dance goes syrupy or frantic, and no amount of
/// fiddling with anything else puts it right.
pub const GRAVITY: f32 = 9.81 * PER_METRE;

/// How many times the simulation is advanced per tick.
///
/// The drive reaches twenty-four cycles a second on the shiver, and its acceleration goes as
/// the *square* of that, so a sixtieth of a second is far too coarse a step: the skeleton would
/// gain energy out of nowhere and tear itself apart. Twenty-four substeps put the integrator at
/// about a kilohertz and a half. A constant, and not a measurement of how fast the machine is,
/// because a replay has to do the same arithmetic in the same order.
pub const SUBSTEPS: u32 = 48;

// ---------------------------------------------------------------------------------------
// the body
// ---------------------------------------------------------------------------------------

/// What a bone is, so that the binary can decide what it looks like without the simulation
/// having to know.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    /// The small of the back, straight off the pelvis.
    Spine,
    /// The ribcage hangs on this one, and so does most of the weight.
    Chest,
    /// Short, and therefore quick: it swings at nearly twice the rate of the spine.
    Neck,
    /// The head, carried as a bone with a skull on the end of it.
    Skull,
    /// The collar bone, which is what holds an arm out from the body.
    Collar,
    /// The first link of the arm.
    UpperArm,
    /// The second link, and the one that does the flailing.
    Forearm,
    /// The last link of the arm, light enough to be thrown about by everything above it.
    Hand,
    /// The first link of the leg, and the heaviest bone in the body.
    Thigh,
    /// The second link of the leg.
    Shin,
    /// The last, and the one that snaps.
    Foot,
}

/// Which of the two of something a bone is. Anatomy, not decoration: the rest angles below are
/// mirror images, and the binary uses it to decide which arm is nearer the viewer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// The viewer's left, drawn behind the body.
    Left,
    /// On the centre line: the spine, the head, the pelvis.
    Middle,
    /// The viewer's right, drawn in front.
    Right,
}

/// One bone: how long and how heavy, what it hangs from, where it rests, how far it may swing.
pub struct BoneSpec {
    /// What it is.
    pub part: Part,
    /// Which side of the body.
    pub side: Side,
    /// The bone it hangs from, or `None` for the three that hang off the pelvis itself.
    pub parent: Option<usize>,
    /// How far along that bone its joint sits, as a fraction of the parent's length.
    pub along: f32,
    /// Where its joint sits relative to the pelvis, for the bones with no parent. The pelvis
    /// is one rigid piece and does not turn, so these are simply carried along with it.
    pub offset: Vec2,
    /// How long it is, in world units.
    pub length: f32,
    /// How heavy it is for its length. One is a plain bone; a skull and a ribcage are a good
    /// deal more than that, and a hand rather less.
    pub weight: f32,
    /// How much of the body's tone this particular joint gets.
    ///
    /// A spine is not a shoulder. The joints of the trunk are held several times as firmly as
    /// the ones in a limb, and they have to be: a shoulder that were as firm as a spine could
    /// not swing, and a spine as loose as a shoulder could not stand up. One is a plain joint,
    /// which is what the [`Tone`] threshold of one is about.
    pub firm: f32,
    /// Where it points when the body is at rest, in degrees counter-clockwise from straight
    /// down, in the world.
    ///
    /// World angles rather than angles relative to the parent, because a pose is far easier to
    /// write down and check that way. What the simulation uses is the *difference* from the
    /// parent's rest angle, so these have to be written going the same way round as the parent
    /// — the left arm hangs at 348 rather than at −12, since its collar points at 258 and the
    /// difference wanted is 90. A test holds the whole table to that.
    pub rest: f32,
    /// How far the joint may bend either side of its rest angle, in degrees. Asymmetric, and
    /// mirrored between left and right, because joints are.
    pub limit: (f32, f32),
}

/// The whole skeleton, parents always before their children so that one pass down this list is
/// a pass down the tree.
///
/// The proportions are a human's, give or take: legs a shade under half the height, arms
/// reaching to mid-thigh, a head an eighth of the whole. They matter more than they look like
/// they should, because a bone's length is also its pendulum period — a forearm swings at very
/// nearly twice the rate of a thigh, and it is that ratio of rates, rather than the shapes,
/// that makes the flailing read as a body flailing instead of as a mobile.
#[rustfmt::skip]
pub const BONES: &[BoneSpec] = &[
    // the trunk, straight up off the pelvis
    bone(Part::Spine,    Side::Middle, None,     0.00, vec2(  0.0,  8.0), 58.0, 1.6, 3.2, 180.0, ( -40.0,  40.0)),
    bone(Part::Chest,    Side::Middle, Some(0),  1.00, Vec2::ZERO,        62.0, 2.6, 3.2, 180.0, ( -30.0,  30.0)),
    bone(Part::Neck,     Side::Middle, Some(1),  1.00, Vec2::ZERO,        20.0, 1.0, 3.0, 180.0, ( -30.0,  30.0)),
    bone(Part::Skull,    Side::Middle, Some(2),  1.00, Vec2::ZERO,        42.0, 2.8, 3.0, 180.0, ( -24.0,  24.0)),
    // the left arm: a collar out to the side, then three links of pendulum
    bone(Part::Collar,   Side::Left,   Some(1),  0.94, Vec2::ZERO,        30.0, 1.0, 2.0, 258.0, ( -15.0,  15.0)),
    bone(Part::UpperArm, Side::Left,   Some(4),  1.00, Vec2::ZERO,        64.0, 1.1, 1.0, 348.0, (-168.0,  46.0)),
    bone(Part::Forearm,  Side::Left,   Some(5),  1.00, Vec2::ZERO,        54.0, 0.9, 0.9, 352.0, ( -46.0, 136.0)),
    bone(Part::Hand,     Side::Left,   Some(6),  1.00, Vec2::ZERO,        20.0, 0.6, 0.7, 354.0, ( -55.0,  55.0)),
    // and the right, the same numbers with the signs turned over
    bone(Part::Collar,   Side::Right,  Some(1),  0.94, Vec2::ZERO,        30.0, 1.0, 2.0, 102.0, ( -15.0,  15.0)),
    bone(Part::UpperArm, Side::Right,  Some(8),  1.00, Vec2::ZERO,        64.0, 1.1, 1.0,  12.0, ( -46.0, 168.0)),
    bone(Part::Forearm,  Side::Right,  Some(9),  1.00, Vec2::ZERO,        54.0, 0.9, 0.9,   8.0, (-136.0,  46.0)),
    bone(Part::Hand,     Side::Right,  Some(10), 1.00, Vec2::ZERO,        20.0, 0.6, 0.7,   6.0, ( -55.0,  55.0)),
    // the legs, hung straight off the pelvis a hip's width apart
    bone(Part::Thigh,    Side::Left,   None,     0.00, vec2(-17.0, -4.0), 86.0, 1.8, 1.4,  -4.0, ( -95.0,  62.0)),
    bone(Part::Shin,     Side::Left,   Some(12), 1.00, Vec2::ZERO,        80.0, 1.2, 1.2,  -2.0, (-116.0,  26.0)),
    bone(Part::Foot,     Side::Left,   Some(13), 1.00, Vec2::ZERO,        24.0, 0.7, 0.9, -72.0, ( -34.0,  44.0)),
    bone(Part::Thigh,    Side::Right,  None,     0.00, vec2( 17.0, -4.0), 86.0, 1.8, 1.4,   4.0, ( -62.0,  95.0)),
    bone(Part::Shin,     Side::Right,  Some(15), 1.00, Vec2::ZERO,        80.0, 1.2, 1.2,   2.0, ( -26.0, 116.0)),
    bone(Part::Foot,     Side::Right,  Some(16), 1.00, Vec2::ZERO,        24.0, 0.7, 0.9,  72.0, ( -44.0,  34.0)),
];

/// How many bones there are.
pub const BONE_COUNT: usize = BONES.len();

/// One row of [`BONES`], spelled out. Only here to keep the table above readable as a table.
#[expect(clippy::too_many_arguments, reason = "it is a table row")]
const fn bone(
    part: Part,
    side: Side,
    parent: Option<usize>,
    along: f32,
    offset: Vec2,
    length: f32,
    weight: f32,
    firm: f32,
    rest: f32,
    limit: (f32, f32),
) -> BoneSpec {
    BoneSpec {
        part,
        side,
        parent,
        along,
        offset,
        length,
        weight,
        firm,
        rest,
        limit,
    }
}

impl BoneSpec {
    /// Where it rests, in radians.
    pub fn rest_angle(&self) -> f32 {
        self.rest.to_radians()
    }

    /// How far it may bend either side of its rest angle, in radians.
    pub fn limits(&self) -> (f32, f32) {
        (self.limit.0.to_radians(), self.limit.1.to_radians())
    }

    /// What it weighs. The unit is arbitrary — it appears on both sides of the equation of
    /// motion and cancels — but the *ratios* are not, and they are why a hand is thrown about
    /// by an arm rather than the other way round.
    pub fn mass(&self) -> f32 {
        self.length * self.weight
    }

    /// How fast it swings on its own, in radians a second: the pendulum rate of a uniform rod
    /// of this length pivoted at one end, `sqrt(3g/2L)`.
    pub fn swing_rate(&self) -> f32 {
        (1.5 * GRAVITY / self.length).sqrt()
    }
}

/// The bend this joint calls zero: how far its bone's rest angle sits from its parent's.
///
/// Worked out from the two world rest angles rather than written down, because a pose is much
/// easier to author and to check as a set of world angles. The bones that hang off the pelvis
/// measure from straight down, the pelvis being one rigid piece that does not turn.
pub fn rest_bend(index: usize) -> f32 {
    let spec = &BONES[index];
    match spec.parent {
        Some(parent) => spec.rest_angle() - BONES[parent].rest_angle(),
        None => spec.rest_angle(),
    }
}

// ---------------------------------------------------------------------------------------
// the dance
// ---------------------------------------------------------------------------------------

/// One axis of a step: a sine of some multiple of the beat.
#[derive(Clone, Copy, Debug)]
pub struct Wave {
    /// How far it travels, in world units.
    pub reach: f32,
    /// How many times it goes round per beat. Two against one is what makes a figure of eight;
    /// six against one is a shiver.
    pub beats: f32,
    /// Where in its cycle it starts, in turns.
    pub start: f32,
}

impl Wave {
    /// Where it is, at this point in the beat.
    pub fn at(&self, phase: f32) -> f32 {
        self.reach * (self.beats * phase + self.start * TAU).sin()
    }

    /// How hard it is accelerating there. Twice differentiated, so the tempo comes in
    /// *squared* — which is why the same step is a saunter at sixty beats and a mauling at two
    /// hundred, though the hips travel exactly as far either way.
    pub fn acceleration(&self, phase: f32, rate: f32) -> f32 {
        let speed = self.beats * rate;
        -self.reach * speed * speed * (self.beats * phase + self.start * TAU).sin()
    }

    /// How fast this wave sweeps, in world units a second: the `Aω` in Kapitza's condition.
    pub fn sweep(&self, rate: f32) -> f32 {
        self.reach * self.beats * rate
    }
}

/// A step: what the pelvis does, and nothing else. Three numbers, twice.
pub struct Step {
    /// Its name, for the readout.
    pub name: &'static str,
    /// One line on what it is.
    pub blurb: &'static str,
    /// Side to side.
    pub across: Wave,
    /// Up and down.
    pub along: Wave,
}

/// The five steps, on the number keys.
///
/// Every one of them is two sines. Everything else on screen — the lag in the arms, the way a
/// shin snaps through, the wobble that arrives at the skull half a beat late — is the
/// skeleton's answer to them, and not part of the question.
#[rustfmt::skip]
pub const STEPS: &[Step] = &[
    Step {
        name: "SWAY",
        blurb: "side to side, once a beat, and nothing up or down at all",
        across: Wave { reach: 26.0, beats: 1.0, start: 0.0 },
        along:  Wave { reach:  0.0, beats: 1.0, start: 0.0 },
    },
    Step {
        name: "BOB",
        blurb: "straight up and down, which is the one the knees hate",
        across: Wave { reach:  0.0, beats: 1.0, start: 0.0 },
        along:  Wave { reach: 17.0, beats: 1.0, start: 0.0 },
    },
    Step {
        name: "FIGURE OF EIGHT",
        blurb: "across once a beat and up and down twice: what hips actually do",
        across: Wave { reach: 23.0, beats: 1.0, start: 0.0 },
        along:  Wave { reach: 10.0, beats: 2.0, start: 0.0 },
    },
    Step {
        name: "ROUND",
        blurb: "a plain circle, the two axes a quarter beat apart",
        across: Wave { reach: 19.0, beats: 1.0, start: 0.00 },
        along:  Wave { reach: 19.0, beats: 1.0, start: 0.25 },
    },
    Step {
        // Small and fast rather than big and slow, which is the whole of Kapitza's condition:
        // what matters is A·ω, and ω is much the cheaper of the two to buy.
        name: "SHIVER",
        blurb: "a hand's width, five times a beat: fast enough to stand a limb on its head",
        across: Wave { reach:  0.0, beats: 5.0, start: 0.0 },
        along:  Wave { reach:  7.0, beats: 5.0, start: 0.0 },
    },
];

impl Step {
    /// Where the pelvis is, at this point in the beat.
    pub fn offset(&self, phase: f32) -> Vec2 {
        vec2(self.across.at(phase), self.along.at(phase))
    }

    /// How hard the pelvis is accelerating there.
    pub fn acceleration(&self, phase: f32, rate: f32) -> Vec2 {
        vec2(
            self.across.acceleration(phase, rate),
            self.along.acceleration(phase, rate),
        )
    }

    /// Kapitza's number for a bone of this length under this step: how far past the threshold
    /// its up-and-down part is. Over one, and upside down is somewhere a bone of that length
    /// will sit.
    pub fn kapitza(&self, length: f32, rate: f32) -> f32 {
        kapitza_number(length, self.along.reach * self.along.beats, rate)
    }
}

// ---------------------------------------------------------------------------------------
// the knobs
// ---------------------------------------------------------------------------------------

/// Slowest the music goes. Zero is allowed: the band stops and the skeleton hangs there.
pub const TEMPO_MIN: f32 = 0.0;
/// Fastest.
pub const TEMPO_MAX: f32 = 200.0;
/// Where it starts, in beats a minute. A slow four-four.
pub const TEMPO_START: f32 = 92.0;
/// How much a held key changes the tempo, in beats a minute per second.
pub const TEMPO_RAMP: f32 = 60.0;

/// Least tone. Nothing holds any joint anywhere: a bag of bones on a string.
pub const TONE_MIN: f32 = 0.0;
/// Most. Well past stiff; the whole skeleton moves as one piece and only rings a little.
pub const TONE_MAX: f32 = 6.0;
/// Where it starts.
///
/// Comfortably over one, which is where a body can first hold itself up at all, and well under
/// the point where it stops being able to flop. A dance lives here.
pub const TONE_START: f32 = 1.8;
/// How much a held key changes the tone, per second.
pub const TONE_RAMP: f32 = 1.2;

/// How much of a joint's swing is lost to drag, as a fraction of its own critical damping.
///
/// All of the damping in the body comes from tone, and none of it from anywhere else. That is
/// not a shortcut: a tense joint really is a damped one, and a dead one really is close to
/// frictionless. It also means [`Tone`] zero is a body with nothing in it at all — no spring,
/// no holding and no friction — which is a skeleton that will rattle until something stops it,
/// and is the one setting at which the energy of the whole body is conserved and can be
/// checked against itself.
pub const DRAG_RATIO: f32 = 0.28;

/// How much stiffer a joint's stop is than its tone spring at tone one.
///
/// A ligament, not a wall. Stiff enough that a joint does not visibly pass its stop, soft
/// enough that the integrator never has to take it seriously.
pub const STOP_STIFFNESS: f32 = 1200.0;
/// How much of critical the stop's own damping is. Under one, so that a joint arriving hard
/// comes back off its stop rather than sticking to it.
pub const STOP_DAMPING: f32 = 0.55;
/// How fast a joint has to arrive at a stop for it to count as a knock worth hearing, in
/// radians a second.
pub const KNOCK_FLOOR: f32 = 1.1;

// ---------------------------------------------------------------------------------------
// how the weight is distributed
// ---------------------------------------------------------------------------------------

/// The body's inertia, worked out once and never again.
///
/// This is the part that makes eighteen pendulums into one system. A bone's centre of mass sits
/// at the end of a chain of levers — its own half-length, plus a piece of every bone between it
/// and the pelvis — so its velocity depends on *every* angle above it in the tree, and the
/// kinetic energy of the whole body comes out as a quadratic form in all eighteen angular
/// rates. These are the constant parts of that form.
#[derive(Resource, Clone, Debug)]
pub struct Frame {
    /// `lever[i][j]`: how far bone `i`'s centre of mass sits along bone `j`. Its own half
    /// length for `j == i`, the distance to the next joint down for a bone above it, and zero
    /// for a bone that is not above it at all.
    pub lever: Vec<[f32; BONE_COUNT]>,
    /// `coupling[j][k] = Σ m·lever[i][j]·lever[i][k]`, the constant part of the mass matrix.
    /// Non-zero only where one of the two bones hangs off the other, which is exactly the
    /// statement that two limbs feel each other only through the body they share.
    pub coupling: Vec<[f32; BONE_COUNT]>,
    /// `carry[j] = Σ m·lever[i][j]`: the weight-moment about joint `j` of bone `j` and
    /// everything hanging off it. What gravity pulls on, and what tone is measured against.
    pub carry: [f32; BONE_COUNT],
    /// The diagonal of the mass matrix, which does not depend on the pose: how hard bone `j`
    /// is to turn about its own joint with its whole limb attached.
    pub spin: [f32; BONE_COUNT],
    /// How hard each joint's stop pushes back per radian past it, and how hard it damps.
    pub stop: [(f32, f32); BONE_COUNT],
    /// The torque each joint has to hold to keep the body in its rest pose against gravity.
    ///
    /// A spring alone cannot hold a pose: it pulls towards the rest angle, and at the rest
    /// angle it pulls with nothing, while gravity is still pulling with everything. A real body
    /// solves this the same way, by holding — so tone supplies this torque as well, up to as
    /// much of it as tone one would be strong enough to provide. It is a *joint* torque like
    /// any other, so it pushes back on the bone above, which is why a joint has to hold what
    /// its children are holding as well as its own.
    pub hold: [f32; BONE_COUNT],
}

impl Default for Frame {
    fn default() -> Self {
        let mut lever = vec![[0.0f32; BONE_COUNT]; BONE_COUNT];
        for (index, spec) in BONES.iter().enumerate() {
            // Its own half length, and then walk up the tree noting the lever each bone above
            // carries this one on.
            lever[index][index] = spec.length * 0.5;
            let mut child = index;
            while let Some(parent) = BONES[child].parent {
                lever[index][parent] = BONES[parent].length * BONES[child].along;
                child = parent;
            }
        }

        let mut coupling = vec![[0.0f32; BONE_COUNT]; BONE_COUNT];
        let mut carry = [0.0f32; BONE_COUNT];
        for (index, spec) in BONES.iter().enumerate() {
            let mass = spec.mass();
            for j in 0..BONE_COUNT {
                if lever[index][j] == 0.0 {
                    continue;
                }
                carry[j] += mass * lever[index][j];
                for k in 0..BONE_COUNT {
                    coupling[j][k] += mass * lever[index][j] * lever[index][k];
                }
            }
        }

        let mut spin = [0.0f32; BONE_COUNT];
        let mut stop = [(0.0f32, 0.0f32); BONE_COUNT];
        for (index, spec) in BONES.iter().enumerate() {
            // A rod's own moment about its middle. Added to the coupling's diagonal it comes to
            // `mL²/3` for a lone bone, which is a rod's moment about its end.
            let own = spec.mass() * spec.length * spec.length / 12.0;
            spin[index] = coupling[index][index] + own;
            let stiffness = STOP_STIFFNESS * carry[index] * GRAVITY;
            stop[index] = (
                stiffness,
                STOP_DAMPING * 2.0 * (stiffness * spin[index]).sqrt(),
            );
        }

        // What each joint must hold, worked out from the leaves inward: its own limb's weight
        // moment, plus whatever every joint below it is already pulling with.
        let mut hold = [0.0f32; BONE_COUNT];
        for index in (0..BONE_COUNT).rev() {
            hold[index] += carry[index] * GRAVITY * BONES[index].rest_angle().sin();
            if let Some(parent) = BONES[index].parent {
                hold[parent] += hold[index];
            }
        }

        Self {
            lever,
            coupling,
            carry,
            spin,
            stop,
            hold,
        }
    }
}

impl Frame {
    /// How stiff this joint's tone spring is, in torque per radian.
    ///
    /// Scaled by the weight-moment the joint carries, which is what makes [`Tone`] mean the
    /// same thing at every joint in the body: at tone one the spring's pull and gravity's pull
    /// are exactly equal and opposite for small leanings, and that is the threshold at which a
    /// joint can first hold its own limb up.
    pub fn stiffness(&self, index: usize, tone: f32) -> f32 {
        tone * BONES[index].firm * self.carry[index] * GRAVITY
    }

    /// How hard this joint drags, in torque per radian a second.
    pub fn drag(&self, index: usize, tone: f32) -> f32 {
        // Critical damping for this joint's own spring, so that one number means the same
        // fraction of a swing lost at every joint in the body.
        let critical = 2.0 * (self.stiffness(index, tone) * self.spin[index]).sqrt();
        DRAG_RATIO * critical
    }
}

// ---------------------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------------------

/// The music: how fast, and where in the bar.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Beat {
    /// Beats a minute.
    pub tempo: f32,
    /// Where in the beat, in radians, wrapped to one turn.
    pub phase: f32,
    /// How many beats have gone by. The binary hits a drum on each new one.
    pub count: u64,
}

impl Default for Beat {
    fn default() -> Self {
        Self {
            tempo: TEMPO_START,
            phase: 0.0,
            count: 0,
        }
    }
}

impl Beat {
    /// The beat as an angular rate, in radians a second.
    pub fn rate(&self) -> f32 {
        TAU * self.tempo / 60.0
    }
}

/// How hard the joints hold their pose, as a multiple of what gravity pulls with.
///
/// One is the interesting number, and it is not a tuning constant: at exactly one, a joint's
/// spring can just balance gravity on the limb it carries. Under it, nothing in the body can
/// hold itself up, and the skeleton folds.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Tone(pub f32);

impl Default for Tone {
    fn default() -> Self {
        Self(TONE_START)
    }
}

/// Which of [`STEPS`] the pelvis is dancing.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Routine(pub usize);

impl Default for Routine {
    /// The figure of eight, which is the one that looks most like dancing.
    fn default() -> Self {
        Self(2)
    }
}

impl Routine {
    /// The step itself.
    pub fn step(&self) -> &'static Step {
        &STEPS[self.0 % STEPS.len()]
    }
}

/// One joint's state: which way its bone points and how fast that is changing.
///
/// Both are *world* angles, measured counter-clockwise from straight down, and neither is ever
/// wrapped. What a joint has bent by is the difference from its parent's angle, and keeping
/// both unwrapped is what keeps that difference meaningful at the top of the spine, where the
/// angle itself is near half a turn.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Joint {
    /// Which way the bone points, in radians.
    pub angle: f32,
    /// How fast that is turning, in radians a second.
    pub rate: f32,
    /// Whether the joint is up against one of its stops.
    pub stopped: bool,
}

/// Where a bone ended up, for whoever has to draw it.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Place {
    /// The joint it turns about.
    pub pivot: Vec2,
    /// The far end.
    pub tip: Vec2,
    /// Which way it points, in radians counter-clockwise from straight down.
    pub angle: f32,
}

/// A joint arriving at its stop hard enough to be worth a noise.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Knock {
    /// Which bone.
    pub bone: usize,
    /// How fast it was bending when it got there, in radians a second.
    pub speed: f32,
}

/// The skeleton: eighteen joints, where they have got to, and where the pelvis is.
#[derive(Resource, Clone, Debug)]
pub struct Skeleton {
    /// One per bone, in [`BONES`] order.
    pub joints: Vec<Joint>,
    /// Where each bone ended up this tick.
    pub places: Vec<Place>,
    /// Where the pelvis is.
    pub hips: Vec2,
    /// How hard the pelvis is being pulled about, which is where every bit of the movement in
    /// here comes from.
    pub shove: Vec2,
    /// Joints that arrived at a stop this tick. Cleared and refilled every tick.
    pub knocks: Vec<Knock>,
}

impl Default for Skeleton {
    fn default() -> Self {
        let mut skeleton = Self {
            joints: BONES
                .iter()
                .map(|spec| Joint {
                    angle: spec.rest_angle(),
                    rate: 0.0,
                    stopped: false,
                })
                .collect(),
            places: vec![Place::default(); BONE_COUNT],
            hips: Vec2::ZERO,
            shove: Vec2::ZERO,
            knocks: Vec::new(),
        };
        skeleton.settle();
        skeleton
    }
}

impl Skeleton {
    /// Work out where every bone is, from the pelvis outward.
    ///
    /// One pass down the list, which is one pass down the tree, because a parent always comes
    /// before its children in [`BONES`].
    pub fn settle(&mut self) {
        for (index, spec) in BONES.iter().enumerate() {
            let angle = self.joints[index].angle;
            let pivot = match spec.parent {
                Some(parent) => {
                    let up = self.places[parent];
                    up.pivot + direction(up.angle) * (BONES[parent].length * spec.along)
                }
                None => self.hips + spec.offset,
            };
            self.places[index] = Place {
                pivot,
                tip: pivot + direction(angle) * spec.length,
                angle,
            };
        }
    }

    /// How far this joint is from where it would rather be. Zero in the rest pose, and what
    /// both the tone spring and the stops are measured against.
    pub fn strain(&self, index: usize) -> f32 {
        self.joints[index].angle - self.parent_angle(index) - rest_bend(index)
    }

    /// How fast this joint is bending.
    pub fn bend_rate(&self, index: usize) -> f32 {
        self.joints[index].rate - self.parent_rate(index)
    }

    /// Which way the bone this one hangs from points. Straight down for the three that hang off
    /// the pelvis, which does not turn.
    fn parent_angle(&self, index: usize) -> f32 {
        match BONES[index].parent {
            Some(parent) => self.joints[parent].angle,
            None => 0.0,
        }
    }

    /// How fast that bone is turning.
    fn parent_rate(&self, index: usize) -> f32 {
        match BONES[index].parent {
            Some(parent) => self.joints[parent].rate,
            None => 0.0,
        }
    }

    /// The whole body's energy, kinetic plus gravitational, with the pelvis held still.
    ///
    /// Nothing in the simulation reads this. It is here because it is the sharpest test there
    /// is of whether the coupling above is right: turn off the drive, the tone and the drag,
    /// and this number must not move.
    pub fn energy(&self, frame: &Frame) -> f32 {
        let mut total = 0.0;
        for (index, spec) in BONES.iter().enumerate() {
            let mass = spec.mass();
            // A centre of mass rides on every bone above it, so both where it is and how fast
            // it is going are sums over the whole chain.
            let mut speed = Vec2::ZERO;
            let mut middle = self.hips + root_offset(index);
            for other in 0..BONE_COUNT {
                let lever = frame.lever[index][other];
                if lever == 0.0 {
                    continue;
                }
                let angle = self.joints[other].angle;
                speed += crosswise(angle) * (lever * self.joints[other].rate);
                middle += direction(angle) * lever;
            }
            let own = mass * spec.length * spec.length / 12.0;
            let spin = self.joints[index].rate;
            total += 0.5 * mass * speed.length_squared() + 0.5 * own * spin * spin;
            total += mass * GRAVITY * middle.y;
        }
        total
    }
}

/// Where on the pelvis this bone's limb starts. Zero for everything but the three bones bolted
/// straight to it.
fn root_offset(index: usize) -> Vec2 {
    let mut walk = index;
    while let Some(parent) = BONES[walk].parent {
        walk = parent;
    }
    BONES[walk].offset
}

/// Which way a bone at this angle points: straight down at zero, counter-clockwise from there.
pub fn direction(angle: f32) -> Vec2 {
    vec2(angle.sin(), -angle.cos())
}

/// The same, turned a quarter turn: which way the far end moves as the angle grows.
pub fn crosswise(angle: f32) -> Vec2 {
    vec2(angle.cos(), angle.sin())
}

// ---------------------------------------------------------------------------------------
// one bone, on its own
// ---------------------------------------------------------------------------------------

/// How hard a rod of length `length` is turned by gravity and by its joint being thrown about,
/// with nothing hanging off it.
///
/// The equation the whole piece generalises, and the one the tests check against the textbook.
/// `shove` is the acceleration of the pivot, and it arrives as a fictitious force on the rod's
/// middle — the same thing that tips you over when a bus pulls away.
///
/// The solver below reduces to exactly this for a bone with nothing hanging off it: the mass
/// cancels, a rod's moment about its end is `mL²/3`, its weight-moment is `mL/2`, and the ratio
/// of the two is `3/2L`.
pub fn swing(length: f32, angle: f32, shove: Vec2) -> f32 {
    -(1.5 / length) * ((GRAVITY + shove.y) * angle.sin() + shove.x * angle.cos())
}

/// Kapitza's condition for a uniform rod of this length: how far past the threshold a pivot
/// bobbing `reach` up and down at `rate` radians a second is.
///
/// `(Aω)² / 2gℓ` with `ℓ = 2L/3`. Over one, and *upside down* stops being the way a pendulum
/// falls and becomes somewhere it will sit.
pub fn kapitza_number(length: f32, reach: f32, rate: f32) -> f32 {
    let sweep = reach * rate;
    sweep * sweep / (2.0 * GRAVITY * (2.0 * length / 3.0))
}

// ---------------------------------------------------------------------------------------
// eighteen bones, together
// ---------------------------------------------------------------------------------------

/// Work out what every joint is doing, all at once.
///
/// `M θ¨ = r`, where `M` is the body's inertia in the pose it is in and `r` is everything
/// pulling on it: gravity, the shaking of the pelvis, the tug of every bone that is already
/// turning, and the springs, drags and stops at the joints. `M` is symmetric and positive
/// definite — it is a kinetic energy — so it goes down to a Cholesky factor and back in a few
/// hundred operations.
///
/// The off-diagonal terms are the entire point. `M[j][k]` is how much turning bone `k` turns
/// bone `j`, and it is a cosine of the angle between them: two bones in line drag each other
/// hardest, two at right angles not at all. Take those terms out and every bone becomes a
/// separate pendulum being shaken for free, which is a mobile rather than a body.
fn resolve(frame: &Frame, skeleton: &Skeleton, shove: Vec2, tone: f32) -> [f32; BONE_COUNT] {
    let mut mass = [[0.0f32; BONE_COUNT]; BONE_COUNT];
    let mut force = [0.0f32; BONE_COUNT];

    for j in 0..BONE_COUNT {
        let angle = skeleton.joints[j].angle;
        let (sin, cos) = angle.sin_cos();
        // Gravity and the pelvis reach a joint by the same route, so they are one term.
        force[j] = -frame.carry[j] * ((GRAVITY + shove.y) * sin + shove.x * cos);
        #[expect(
            clippy::needless_range_loop,
            reason = "k indexes three different things"
        )]
        for k in 0..BONE_COUNT {
            let coupling = frame.coupling[j][k];
            if coupling == 0.0 {
                continue;
            }
            let between = skeleton.joints[k].angle - angle;
            mass[j][k] = coupling * between.cos();
            // What bone k's turning throws at bone j: anything swinging is also pulling
            // inwards on everything it hangs from.
            let rate = skeleton.joints[k].rate;
            force[j] += coupling * between.sin() * rate * rate;
        }
        mass[j][j] = frame.spin[j];
    }

    // The joints themselves: a spring towards the rest pose, a drag, and a stop at each end.
    // Each of them pushes on its bone *and* pulls back on the bone it hangs from, which is the
    // third law, and the reason a thrown arm rocks the body it is attached to.
    for j in 0..BONE_COUNT {
        let strain = skeleton.strain(j);
        let bend_rate = skeleton.bend_rate(j);
        // The spring, the drag, and the holding: a body at rest in a pose is a body whose
        // joints are working. Tone one is exactly enough to hold, and is also exactly where a
        // joint becomes stable, which is not a coincidence — they are the same balance.
        let mut torque = tone.min(1.0) * frame.hold[j]
            - frame.stiffness(j, tone) * strain
            - frame.drag(j, tone) * bend_rate;

        let (low, high) = BONES[j].limits();
        let past = if strain < low {
            strain - low
        } else if strain > high {
            strain - high
        } else {
            0.0
        };
        if past != 0.0 {
            let (stiffness, damping) = frame.stop[j];
            torque -= stiffness * past;
            // Damped only while it is still going deeper, so that a joint comes back off its
            // stop rather than sticking to it.
            if past * bend_rate > 0.0 {
                torque -= damping * bend_rate;
            }
        }

        force[j] += torque;
        if let Some(parent) = BONES[j].parent {
            force[parent] -= torque;
        }
    }

    solve(&mut mass, &mut force);
    force
}

/// Solve `M x = r` in place by Cholesky. `M` comes back as its own factor and `r` as `x`.
fn solve(mass: &mut [[f32; BONE_COUNT]; BONE_COUNT], rhs: &mut [f32; BONE_COUNT]) {
    for i in 0..BONE_COUNT {
        for j in 0..=i {
            let mut sum = mass[i][j];
            #[expect(clippy::needless_range_loop, reason = "k walks two rows at once")]
            for k in 0..j {
                sum -= mass[i][k] * mass[j][k];
            }
            if i == j {
                // A mass matrix cannot really be singular; the floor is only here so that a
                // rounding error can never become a NaN and spread through the whole body.
                mass[i][j] = sum.max(1.0e-9).sqrt();
            } else {
                mass[i][j] = sum / mass[j][j];
            }
        }
    }
    for i in 0..BONE_COUNT {
        let mut sum = rhs[i];
        for k in 0..i {
            sum -= mass[i][k] * rhs[k];
        }
        rhs[i] = sum / mass[i][i];
    }
    for i in (0..BONE_COUNT).rev() {
        let mut sum = rhs[i];
        for k in i + 1..BONE_COUNT {
            sum -= mass[k][i] * rhs[k];
        }
        rhs[i] = sum / mass[i][i];
    }
}

// ---------------------------------------------------------------------------------------
// the plugin
// ---------------------------------------------------------------------------------------

/// Installs the skeleton, the music, and the arithmetic that ties them together.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Frame::default());
        app.world_mut().insert_resource(Beat::default());
        app.world_mut().insert_resource(Tone::default());
        app.world_mut().insert_resource(Routine::default());
        app.world_mut().insert_resource(Skeleton::default());
        app.add_systems(
            FixedUpdate,
            (set_tempo, set_tone, choose_step, dance).chain(),
        );
    }
}

/// Up and down are the tempo, and down all the way stops the music.
fn set_tempo(input: Res<Input>, time: Res<Time>, mut beat: ResMut<Beat>) {
    let step = TEMPO_RAMP * time.fixed_delta;
    if input.pressed(Key::Up) {
        beat.tempo = (beat.tempo + step).min(TEMPO_MAX);
    }
    if input.pressed(Key::Down) {
        beat.tempo = (beat.tempo - step).max(TEMPO_MIN);
    }
}

/// Left and right are the tone: how hard the joints hold their pose.
fn set_tone(input: Res<Input>, time: Res<Time>, mut tone: ResMut<Tone>) {
    let step = TONE_RAMP * time.fixed_delta;
    if input.pressed(Key::Left) {
        tone.0 = (tone.0 - step).max(TONE_MIN);
    }
    if input.pressed(Key::Right) {
        tone.0 = (tone.0 + step).min(TONE_MAX);
    }
}

/// The number keys pick a step; `R` stands the skeleton back up.
fn choose_step(input: Res<Input>, mut routine: ResMut<Routine>, mut skeleton: ResMut<Skeleton>) {
    const KEYS: [Key; 5] = [
        Key::Digit1,
        Key::Digit2,
        Key::Digit3,
        Key::Digit4,
        Key::Digit5,
    ];
    for (slot, key) in KEYS.iter().enumerate() {
        if input.just_pressed(*key) {
            routine.0 = slot;
        }
    }
    if input.just_pressed(Key::R) {
        for (joint, spec) in skeleton.joints.iter_mut().zip(BONES) {
            joint.angle = spec.rest_angle();
            joint.rate = 0.0;
            joint.stopped = false;
        }
    }
}

/// Move the hips, and let the rest of the body find out about it.
fn dance(
    time: Res<Time>,
    frame: Res<Frame>,
    routine: Res<Routine>,
    tone: Res<Tone>,
    mut beat: ResMut<Beat>,
    mut skeleton: ResMut<Skeleton>,
) {
    skeleton.knocks.clear();
    let step = routine.step();
    let slice = time.fixed_delta / SUBSTEPS as f32;

    for _ in 0..SUBSTEPS {
        let rate = beat.rate();
        let carried = beat.phase + rate * slice;
        if carried >= TAU {
            beat.count += 1; // a whole beat has gone by: something should hit a drum
        }
        beat.phase = carried.rem_euclid(TAU);
        skeleton.hips = step.offset(beat.phase);
        skeleton.shove = step.acceleration(beat.phase, rate);

        let shove = skeleton.shove;
        let turn = resolve(&frame, &skeleton, shove, tone.0);
        for (joint, turn) in skeleton.joints.iter_mut().zip(turn) {
            // Semi-implicit: the new rate is what moves the angle, which is what keeps a swing
            // from quietly gaining amplitude every time round.
            joint.rate += turn * slice;
            joint.angle += joint.rate * slice;
        }
        // A knock is the moment a joint *arrives* at a stop, not every step it spends leaning
        // on one. Without that distinction a joint resting against its stop rattles at the
        // substep rate, which is fifteen hundred clacks a second.
        #[expect(
            clippy::needless_range_loop,
            reason = "the index is the joint, not a slot"
        )]
        for index in 0..BONE_COUNT {
            let (low, high) = BONES[index].limits();
            let strain = skeleton.strain(index);
            let against = strain < low || strain > high;
            let speed = skeleton.bend_rate(index).abs();
            if against && !skeleton.joints[index].stopped && speed >= KNOCK_FLOOR {
                skeleton.knocks.push(Knock { bone: index, speed });
            }
            skeleton.joints[index].stopped = against;
        }
    }

    skeleton.settle();
}
