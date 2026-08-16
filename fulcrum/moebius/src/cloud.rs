//! The sky, as a list of circles and the order to draw them in.
//!
//! Moebius draws a cloud as an outline and fills it. The outline is not a curve of his own
//! invention: it is the edge of a run of overlapping circles, so it is made of circular arcs
//! meeting at cusps, and where a later circle covers an earlier one the earlier arc is not
//! there any more. That is the whole construction, and it is what this module builds.
//!
//! A cloud is built in three passes. The **mass** is four to seven big circles overlapping
//! heavily along a spine, which on their own give a smooth heap. The **fringe** is a dozen small
//! ones walked around the upper rim of that mass, each standing a little proud of it, and it is
//! where the scallops come from: a Moebius cloud is not a few big circles, it is a few big
//! circles with a dozen small ones set around the top. Both go into one union with one outline.
//!
//! The **crests** are laid over the finished body afterwards, one circle to a group, so each is
//! drawn over what is already there. A crest that straddles the body's edge adds a bump to the
//! silhouette and leaves its own arc running on into the mass, which is the mark that says
//! *cloud* in a Moebius panel. One that falls inside leaves a closed arc: the second cloud
//! sitting on the first. Nothing is shaded anywhere. An enclosed region is one flat colour, and
//! the arcs are the drawing.
//!
//! Everything here is a function of one number, the clock, so there is no state to drift and a
//! still can be rendered at any moment in the weather without playing up to it.
//!
//! Circles live on the sphere of directions rather than on a plane. A circle here is a unit
//! direction and an angular radius, and the distance to its edge is `length(dir - centre) - r`,
//! which is the chord rather than the arc and is within a fraction of a percent over the sizes
//! a cloud has. That keeps a cloud round when you turn your head under it, and it means the
//! horizon needs no special case: a cloud that reaches below it is cut by the desert.

use std::f32::consts::{PI, TAU};

// ---------------------------------------------------------------------------------------
// how big the sky is allowed to get
// ---------------------------------------------------------------------------------------

/// Most groups that can be sent to the GPU in one frame. A group is one union of circles with
/// one outline: a cloud's body, or one of its crests.
///
/// This is a limit on what is *in front of you*, not on what the sky holds. The three bands
/// together come to two or three hundred groups around the whole compass, of which the frame
/// holds a fifth, so there is room here for a sky several times as crowded as this one before
/// anything is dropped. `tests/clouds.rs` watches that margin.
pub const MAX_GROUPS: usize = 384;
/// Most circles, over all of those groups.
pub const MAX_DISCS: usize = 1536;

// ---------------------------------------------------------------------------------------
// the three bands
// ---------------------------------------------------------------------------------------

/// One band of the sky: a population of clouds all of a size, all at a height, all carried at
/// the same rate.
///
/// Height stands in for distance, the way it does when you look up. The band low against the
/// horizon is the far one, so its clouds are small, slow and many; the band overhead is near,
/// so its clouds are large, quick and few. Drawing them in that order is the whole of the depth
/// in this picture, and it is the only kind of depth allowed: there is no shading to help.
pub struct Band {
    /// How many clouds are spread around the whole compass.
    pub clouds: u32,
    /// The range of angles above the horizon they sit at, in radians.
    pub elevation: (f32, f32),
    /// The range of half-widths, in radians. A cloud is about three of these across.
    pub scale: (f32, f32),
    /// How far the body is drawn out sideways. One is a heap as wide as it is deep; anything
    /// more is a cloud lying down, which is what one does when it is far enough away to be seen
    /// nearly edge on.
    pub stretch: f32,
    /// How fast the wind carries them around you, in radians a second at pace one.
    pub rate: f32,
    /// How long one cloud lasts, in seconds, from first puff to last.
    pub life: (f32, f32),
    /// How many circles make the mass: the few big ones that give the cloud its shape.
    pub mass: (u32, u32),
    /// How many small ones are set around the top of that mass to scallop its edge.
    pub fringe: (u32, u32),
    /// And how many crests are laid over the whole thing afterwards.
    pub crests: (u32, u32),
    /// Which of the palette's four cloud colours it is filled with.
    pub tint: u32,
    /// How often a cloud in this band gets a flat base cut across it.
    pub flat_base: f32,
    /// Nought for a heaped cumulus, one for a streak drawn out to a point at both ends.
    pub taper: f32,
}

/// Near band first in the list, far band last. They are drawn in the other order.
pub const BANDS: [Band; 3] = [
    // Overhead: the big shapes, few, quick, and cut off flat underneath the way a cumulus is.
    // Big enough to be the subject of the panel and no bigger: a cloud wider than about a third
    // of the frame stops being a cloud and becomes a wall with the frame cutting both ends off.
    Band {
        clouds: 22,
        elevation: (0.235, 0.44),
        scale: (0.068, 0.125),
        stretch: 1.05,
        rate: 0.0215,
        life: (150.0, 240.0),
        mass: (4, 7),
        fringe: (11, 18),
        crests: (2, 5),
        tint: 3,
        flat_base: 0.80,
        taper: 0.0,
    },
    // The middle distance, and the busiest part of the drawing.
    Band {
        clouds: 40,
        elevation: (0.090, 0.270),
        scale: (0.036, 0.070),
        stretch: 1.45,
        rate: 0.0128,
        life: (170.0, 260.0),
        mass: (3, 5),
        fringe: (6, 10),
        crests: (1, 4),
        tint: 2,
        flat_base: 0.62,
        taper: 0.30,
    },
    // Along the horizon: small, slow, and drawn out sideways until they are more streak than
    // cloud, which is what distance does to a cumulus seen almost edge on.
    Band {
        clouds: 46,
        elevation: (0.008, 0.092),
        scale: (0.024, 0.046),
        stretch: 3.0,
        rate: 0.0062,
        life: (200.0, 300.0),
        mass: (2, 3),
        fringe: (2, 4),
        crests: (0, 2),
        tint: 1,
        flat_base: 0.24,
        taper: 0.85,
    },
];

/// How much of the palette's fourth colour shows up in the bands below the top one. A little
/// variety of flat colour between neighbouring clouds, which is a separation an artist would
/// make and not a light value.
const PALE: f32 = 0.22;

/// How far past the last circle a group's bounding cap reaches, in radians. Enough to hold the
/// outline, which is drawn centred on the edge and so sticks out by half its width.
const MARGIN: f32 = 0.004;

/// The smallest circle worth drawing, in radians: about two pixels across on a tall window.
///
/// A circle drawn smaller than the line around it is not a circle, it is a full stop, and every
/// puff arriving and every puff leaving passes through that size on its way. Dropping them below
/// this is the difference between a cloud that gathers and one with grit in it.
pub const SMALLEST: f32 = 0.0035;

// ---------------------------------------------------------------------------------------
// what comes out
// ---------------------------------------------------------------------------------------

/// One union of circles with one outline around it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Group {
    /// Where its circles start in [`Sky::discs`].
    pub first: u32,
    /// How many there are.
    pub count: u32,
    /// Which band its cloud belongs to, near band nought. Not sent to the GPU: the order the
    /// groups are in already says everything the drawing needs, and this says what that order
    /// means.
    pub band: u32,
    /// Which of the palette's four cloud colours fills it.
    pub tint: u32,
    /// A half-space the union is cut back to, as a unit normal and an offset: the flat base of
    /// a cumulus. `[0, 0, 0, -1000]` for a group that has none.
    pub plane: [f32; 4],
    /// Centre and angular radius of a cap that holds the whole group, for throwing it away
    /// early on both sides of the bus.
    pub cap: [f32; 4],
}

impl Group {
    /// The plane of a group that is not cut.
    pub const NO_PLANE: [f32; 4] = [0.0, 0.0, 0.0, -1000.0];
}

/// A whole sky, flattened: every circle in one list, and the groups over it in drawing order,
/// which is back to front.
#[derive(Clone, Debug, Default)]
pub struct Sky {
    /// Unit direction in `xyz`, angular radius in `w`.
    pub discs: Vec<[f32; 4]>,
    /// In the order they are drawn.
    pub groups: Vec<Group>,
    /// Where the mass of the cloud being built has put its circles, so the fringe and the crests
    /// know where the outline is. Kept here rather than made afresh for each of a hundred clouds
    /// sixty times a second.
    rim: Vec<[f32; 4]>,
}

impl Sky {
    /// The sky at a moment, from nothing.
    pub fn at(clock: f32) -> Self {
        let mut sky = Self::default();
        sky.build(clock);
        sky
    }

    /// The sky at a moment, into storage that has been used before.
    pub fn build(&mut self, clock: f32) {
        self.discs.clear();
        self.groups.clear();
        // Far band first: it is drawn first and everything else is drawn over it.
        for (index, band) in BANDS.iter().enumerate().rev() {
            for cloud in 0..band.clouds {
                self.cloud(clock, index as u32, band, cloud);
            }
        }
    }

    /// One cloud: its body, then its crests over the top.
    fn cloud(&mut self, clock: f32, band_index: u32, band: &Band, cloud: u32) {
        // Two draws from the hash. The first is fixed for the life of the piece and decides how
        // long this slot's clouds last and how far out of step with the others they are; the
        // second is redrawn every time the slot starts a new cloud, and decides its shape.
        let mut slot = Rng::new(band_index, cloud, 0x51ed_2701);
        let life = slot.range(band.life.0, band.life.1);
        let phase = clock / life + slot.draw();
        let generation = phase.floor();
        let age = phase - generation;
        let sweep = slot.range(0.82, 1.24) * band.rate;

        let mut seed = Rng::new(
            band_index * 977 + cloud,
            generation as i32 as u32,
            0x2f9b_c1d3,
        );
        let azimuth = seed.range(0.0, TAU) + sweep * clock;
        let elevation = seed.range(band.elevation.0, band.elevation.1);
        let scale = seed.range(band.scale.0, band.scale.1);
        let frame = Frame::new(azimuth, elevation);
        let tint = if seed.draw() < PALE && band.tint < 3 {
            band.tint + 1
        } else {
            band.tint
        };

        // The mass: four to seven big circles overlapping heavily along a spine. On their own
        // they are a smooth heap, because circles that overlap by more than half their radius
        // give an outline with no corner worth seeing. That is what they are for.
        let first = self.discs.len() as u32;
        // Counted against the length, not against the cloud. A circle has one size and a body
        // has a length, so a cloud drawn out to three times the width needs three times the
        // circles to stay a mass: the same handful spread over three times the spine are a row
        // of separate bubbles with gaps showing between them.
        let mass = spread(seed.count(band.mass), band.stretch);
        // Lifted out of `self` for the length of this cloud, because the circles go into `self`
        // as they are made and the rim of the mass has to be read while that is happening.
        let mut rim = std::mem::take(&mut self.rim);
        rim.clear();
        for lobe in 0..mass {
            let s = (lobe as f32 + 0.5) / mass as f32;
            let mound = (PI * s).sin().powf(0.8);
            let along = ((s - 0.5) * 2.0 + seed.range(-0.05, 0.05)) * scale * band.stretch
                // A slow shear along the body, out of step from one circle to the next. It is
                // what keeps the silhouette from being the same silhouette slid sideways.
                + 0.055 * scale * (clock * seed.range(0.03, 0.09) + seed.range(0.0, TAU)).sin();
            let lift = (0.13 + 0.44 * mound * (1.0 - band.taper) + seed.range(-0.04, 0.04)) * scale;
            let swell = mound.powf(mix(0.55, 1.50, band.taper));
            let radius = (mix(0.19, 0.03, band.taper) + mix(0.40, 0.47, band.taper) * swell)
                * seed.range(0.86, 1.14)
                * scale
                * seed.envelope(age, (s - 0.5).abs() * 2.0);
            rim.push([along, lift, radius, s]);
            self.push(&frame, along, lift, radius);
        }

        // The fringe: small circles set around the upper rim of those, in the same union, each
        // standing a little proud of the mass so that it puts one bump and two cusps into the
        // outline. This is where the arcs come from. A Moebius cloud is not a few big circles,
        // it is a few big circles with a dozen small ones walked around the top of them, and the
        // difference between the two is the difference between a cloud and a bag of balloons.
        let fringe = spread(seed.count(band.fringe), band.stretch);
        for _ in 0..fringe {
            // Every draw is made before anything is decided, and the same number of them on
            // every turn of the loop. That is not a style: the hash is a stream, so a turn that
            // takes fewer numbers out of it than its neighbours shifts every later circle in the
            // cloud by one number, and it does that the moment some condition flips. A cloud
            // whose circles are all a smooth function of the clock except at the instants when
            // one of them reaches nothing is a cloud that twitches.
            let place = seed.draw();
            let swing = seed.range(-0.85, 0.85);
            let stand = seed.range(0.76, 0.96);
            let shrink = seed.range(0.26, 0.46);
            let Some(&[along, lift, radius, s]) = pick(&rim, place) else {
                continue;
            };
            // Leaned outwards towards the ends of the cloud and set on top in the middle, so
            // that the fringe follows the outline rather than burying itself in the mass.
            let lean = (s - 0.5) * 2.3 + swing;
            // A fringe circle out at the end of the cloud comes and goes with the end it sits on.
            let alive = seed.envelope(age, (s - 0.5).abs() * 2.0);
            self.push(
                &frame,
                along + lean.sin() * radius * stand,
                lift + lean.cos() * radius * stand,
                radius * shrink * alive,
            );
        }
        let count = self.discs.len() as u32 - first;

        // Cut flat underneath. A cumulus has a base, and the base is a straight line: it is the
        // one edge in a cloud that is not an arc, and leaving it out makes the whole heap read
        // as a bag of bubbles.
        let plane = if seed.draw() < band.flat_base {
            let cut = -0.02 * scale;
            [frame.north[0], frame.north[1], frame.north[2], cut]
        } else {
            Group::NO_PLANE
        };
        self.close(first, count, band_index, tint, plane);

        // The crests. Each is one circle and its own group, so each is drawn over what is
        // already there: its arc survives inside the body, and the body's arc does not survive
        // inside it.
        let crests = seed.count(band.crests);
        for _ in 0..crests {
            // Every draw first, and the same number of them every turn, for the reason given
            // over the fringe above.
            let place = seed.draw();
            let straddles = seed.draw() < 0.66;
            let proud = seed.range(0.55, 0.95);
            let sunk = seed.range(-0.15, 0.30);
            let sideways = seed.range(-0.45, 0.45);
            let out = seed.range(0.3, 0.9);
            let shrink = seed.range(0.40, 0.82);
            let alive = seed.envelope(age, out);
            let Some(&[along, lift, radius, _]) = pick(&rim, place) else {
                continue;
            };
            // Two thirds of them straddle the edge and make a bump with an arc running on into
            // the cloud; the rest fall inside and close on themselves.
            let stand = if straddles { proud } else { sunk };
            let first = self.discs.len() as u32;
            self.push(
                &frame,
                along + sideways * radius,
                lift + stand * radius,
                radius * shrink * alive,
            );
            let count = self.discs.len() as u32 - first;
            self.close(first, count, band_index, tint, Group::NO_PLANE);
        }
        self.rim = rim;
    }

    /// Put one circle into the list, at an offset from the cloud's centre measured in radians
    /// along the local east and north.
    fn push(&mut self, frame: &Frame, along: f32, lift: f32, radius: f32) {
        if radius <= SMALLEST {
            return;
        }
        let dir = frame.at(along, lift);
        self.discs.push([dir[0], dir[1], dir[2], radius]);
    }

    /// Finish a group: work out the cap that holds it and add it, unless nothing was put in it.
    fn close(&mut self, first: u32, count: u32, band: u32, tint: u32, plane: [f32; 4]) {
        if count == 0 {
            return;
        }
        let discs = &self.discs[first as usize..(first + count) as usize];
        let mut sum = [0.0f32; 3];
        for disc in discs {
            for (axis, value) in sum.iter_mut().zip(disc) {
                *axis += value;
            }
        }
        let centre = normalize(sum);
        let mut radius = 0.0f32;
        for disc in discs {
            let offset = [
                disc[0] - centre[0],
                disc[1] - centre[1],
                disc[2] - centre[2],
            ];
            radius = radius.max(length(offset) + disc[3]);
        }
        self.groups.push(Group {
            first,
            count,
            band,
            tint,
            plane,
            cap: [centre[0], centre[1], centre[2], radius + MARGIN],
        });
    }
}

// ---------------------------------------------------------------------------------------
// where a cloud stands
// ---------------------------------------------------------------------------------------

/// A cloud's own frame: where its centre points, and the two directions its circles are placed
/// along. `east` runs level with the horizon and `north` runs up the sky, both at right angles
/// to the centre, so an offset in radians along either is an offset in radians on the sky.
struct Frame {
    centre: [f32; 3],
    east: [f32; 3],
    north: [f32; 3],
}

impl Frame {
    fn new(azimuth: f32, elevation: f32) -> Self {
        let (across, along) = azimuth.sin_cos();
        let (rise, run) = elevation.sin_cos();
        Self {
            centre: [across * run, rise, along * run],
            east: [along, 0.0, -across],
            north: [-across * rise, run, -along * rise],
        }
    }

    /// The direction at an offset from the centre.
    fn at(&self, along: f32, lift: f32) -> [f32; 3] {
        normalize([
            self.centre[0] + self.east[0] * along + self.north[0] * lift,
            self.centre[1] + self.east[1] * along + self.north[1] * lift,
            self.centre[2] + self.east[2] * along + self.north[2] * lift,
        ])
    }
}

/// Length of a vector.
pub fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// The same vector, one unit long.
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = length(v).max(1e-9);
    [v[0] / len, v[1] / len, v[2] / len]
}

/// Between two numbers.
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Nought below `edge0`, one above `edge1`, and the smooth step between.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-9)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A count of circles, scaled up to cover a body drawn out by `stretch`.
fn spread(count: u32, stretch: f32) -> u32 {
    ((count as f32 * stretch).round() as u32).max(1)
}

/// One of a list, by a number in nought to one.
fn pick<T>(items: &[T], at: f32) -> Option<&T> {
    if items.is_empty() {
        return None;
    }
    let index = ((at * items.len() as f32) as usize).min(items.len() - 1);
    Some(&items[index])
}

// ---------------------------------------------------------------------------------------
// the hash
// ---------------------------------------------------------------------------------------

/// A stream of numbers from a seed, so that a cloud is a function of which slot it is in and
/// which of that slot's clouds it is. No state is kept anywhere and nothing accumulates, so the
/// sky at a given moment is the same sky however you arrived at that moment.
pub struct Rng(u32);

impl Rng {
    /// A stream from three numbers.
    pub fn new(a: u32, b: u32, c: u32) -> Self {
        Self(mangle(a ^ mangle(b ^ mangle(c.wrapping_mul(0x9e37_79b9)))))
    }

    /// The next number, in nought to one.
    pub fn draw(&mut self) -> f32 {
        self.0 = mangle(self.0.wrapping_add(0x9e37_79b9));
        (self.0 >> 8) as f32 / 16_777_216.0
    }

    /// The next number, over a range.
    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.draw()
    }

    /// A whole number over an inclusive range.
    pub fn count(&mut self, range: (u32, u32)) -> u32 {
        let span = range.1.saturating_sub(range.0) + 1;
        range.0 + (self.draw() * span as f32) as u32 % span
    }

    /// How much of one circle is there, at `age` through the cloud's life, for a circle `out` of
    /// the way from the middle of the cloud.
    ///
    /// Every circle arrives and leaves on its own schedule, so a cloud grows out of a puff in the
    /// middle, stands at its full width for most of its life, and pulls back into a puff again.
    /// That is a better answer than scaling the whole cloud up and down, which is a cloud being
    /// zoomed rather than a cloud forming.
    ///
    /// The schedules are staggered by `out`, so the outermost circle is the last to arrive and
    /// the first to go, and they are all held inside the first and last few percent of the life
    /// so that the slot is empty when it is handed to the next cloud and the change of shape is
    /// never seen. The middle is there for four fifths of the life, which is the number that
    /// matters: stagger the whole body evenly and half the sky is a cloud with most of itself
    /// missing, which reads as a balloon rather than as weather.
    pub fn envelope(&mut self, age: f32, out: f32) -> f32 {
        let birth = 0.02 + 0.15 * out + self.range(0.0, 0.05);
        let grown = birth + self.range(0.04, 0.11);
        let gone = (0.97 - 0.20 * out - self.range(0.0, 0.05)).max(grown + 0.02);
        let fading = gone - self.range(0.04, 0.10);
        smoothstep(birth, grown, age) * (1.0 - smoothstep(fading, gone, age))
    }
}

/// One round of bit mixing.
fn mangle(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}
