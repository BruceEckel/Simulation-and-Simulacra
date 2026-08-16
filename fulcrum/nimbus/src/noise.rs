//! The two noise volumes the cloud shape is carved out of.
//!
//! Both of them tile: the value at `x` and the value at `x + size` are the same sample, by
//! construction rather than by luck, because the lattice the noise is built on wraps. That is
//! not a nicety. The shader samples these with repeat addressing over a cloud layer tens of
//! kilometres across, so a volume with a seam in it would put that seam across the sky every
//! few hundred metres.
//!
//! The recipe is the one the industry settled on (Guerrilla's clouds in *Horizon*, by way of
//! Schneider's write-ups): a low-frequency volume that decides where clouds *are*, and a
//! small high-frequency volume that eats into their edges to make them look like cauliflower
//! rather than like blobs.
//!
//! No GPU in here. This is arithmetic over a lattice, so it is testable, seeded and
//! reproducible, and the tests hold it to the tiling claim.

/// A cubic volume of RGBA8 texels, `data[((z * size) + y) * size + x]` at four bytes each.
#[derive(Clone)]
pub struct Volume {
    /// Texels along each edge.
    pub size: u32,
    /// `size^3` RGBA texels.
    pub data: Vec<u8>,
}

impl Volume {
    /// One channel of one texel, `0..=255`.
    pub fn at(&self, x: u32, y: u32, z: u32, channel: usize) -> u8 {
        let size = self.size;
        let index = ((z % size) * size * size + (y % size) * size + (x % size)) as usize;
        self.data[index * 4 + channel]
    }
}

/// Edge of the shape volume, in texels. A hundred and twenty-eight cubed is eight megabytes
/// and about a second of arithmetic; below it the billows start to look like the lattice they
/// were built on.
pub const SHAPE_SIZE: u32 = 128;

/// Edge of the detail volume. Small on purpose: it is sampled at twenty times the frequency of
/// the shape volume, so its own texels are never seen from far enough away to be resolved.
pub const DETAIL_SIZE: u32 = 32;

/// The frequencies the worley cells are counted in, for each of the three channels.
const CELLS: [u32; 3] = [4, 8, 16];

/// How the three worley frequencies are weighted when they are stacked into one figure.
const OCTAVES: [f32; 3] = [0.625, 0.25, 0.125];

/// Build the low-frequency volume: where clouds are.
///
/// - `r` is perlin-worley, the billowy field the coverage test is made against
/// - `g`, `b`, `a` are inverted worley at three frequencies, which the shader stacks to erode
///   the base shape
pub fn shape_volume(seed: u32) -> Volume {
    build(SHAPE_SIZE, |p| {
        let cells = worley_channels(p, seed);
        let stacked = cells[0] * OCTAVES[0] + cells[1] * OCTAVES[1] + cells[2] * OCTAVES[2];
        // Perlin-worley: the smooth field pushed up towards one wherever worley is dense.
        // Perlin alone gives clouds that look like smoke, worley alone gives clouds that look
        // like foam; this is the mixture that looks like weather.
        let smooth = perlin_fbm(p, 4, 4, seed ^ 0x9e37_79b9);
        let billow = remap(smooth, stacked - 1.0, 1.0, 0.0, 1.0);
        [billow, cells[0], cells[1], cells[2]]
    })
}

/// Build the high-frequency volume: what eats the edges.
pub fn detail_volume(seed: u32) -> Volume {
    build(DETAIL_SIZE, |p| {
        let cells = worley_channels(p, seed ^ 0x85eb_ca6b);
        [cells[0], cells[1], cells[2], 1.0]
    })
}

/// Inverted worley at the three [`CELLS`] frequencies.
fn worley_channels(p: [f32; 3], seed: u32) -> [f32; 3] {
    [
        1.0 - worley(p, CELLS[0], seed),
        1.0 - worley(p, CELLS[1], seed ^ 0x27d4_eb2f),
        1.0 - worley(p, CELLS[2], seed ^ 0x1656_67b1),
    ]
}

/// Fill a volume from a function of the unit cube, dealing the slices out across threads.
///
/// Threaded because this is the better part of a billion lattice evaluations and it happens
/// while the window is waiting to show its first frame. Every worker writes its own slices and
/// reads nothing, so the answer does not depend on how many of them there are.
fn build(size: u32, sample: impl Fn([f32; 3]) -> [f32; 4] + Sync) -> Volume {
    let texels = (size as usize).pow(3);
    let mut field = vec![[0.0f32; 4]; texels];
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(16);
    let band = (size as usize).div_ceil(workers) * (size as usize) * (size as usize);
    std::thread::scope(|scope| {
        for (index, slice) in field.chunks_mut(band.max(1)).enumerate() {
            let sample = &sample;
            let first = index * band;
            scope.spawn(move || {
                for (offset, texel) in slice.iter_mut().enumerate() {
                    let flat = (first + offset) as u32;
                    let z = flat / (size * size);
                    let y = (flat / size) % size;
                    let x = flat % size;
                    *texel = sample([
                        (x as f32 + 0.5) / size as f32,
                        (y as f32 + 0.5) / size as f32,
                        (z as f32 + 0.5) / size as f32,
                    ]);
                }
            });
        }
    });

    // Stretch every channel over the byte it is about to be stored in.
    //
    // This is not cosmetic. Both of the fields below come out of the machinery bunched: worley
    // distance rarely gets near zero, and the perlin-worley mixture lives between about 0.64
    // and 0.87. Stored raw, that mixture would use sixty of the two hundred and fifty-six
    // values available to it, which is four times the quantisation it needs and shows up in the
    // sky as terracing. It would also leave the coverage dial with a tiny useful arc, since
    // coverage is a threshold *against these numbers*: with the field bunched at the top, the
    // sky goes from clear to solid over a few hundredths of a turn.
    let mut data = vec![0u8; texels * 4];
    for channel in 0..4 {
        let (low, high) = window(&field, channel);
        let span = (high - low).max(1e-4);
        for (texel, out) in field.iter().zip(data.chunks_mut(4)) {
            out[channel] = (((texel[channel] - low) / span).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    Volume { size, data }
}

/// The range a channel really occupies, measured off a sample of it and trimmed at both ends so
/// that one freak texel cannot set the scale for the whole volume.
fn window(field: &[[f32; 4]], channel: usize) -> (f32, f32) {
    let stride = (field.len() / 60_000).max(1);
    let mut sample: Vec<f32> = field
        .iter()
        .step_by(stride)
        .map(|texel| texel[channel])
        .collect();
    sample.sort_by(|a, b| a.partial_cmp(b).expect("the field has no NaNs in it"));
    let last = sample.len() - 1;
    let at = |share: f32| sample[(share * last as f32) as usize];
    (at(0.005), at(0.995))
}

/// Move `value` from the range `[low, high]` into `[to_low, to_high]`, unclamped.
fn remap(value: f32, low: f32, high: f32, to_low: f32, to_high: f32) -> f32 {
    to_low + (value - low) / (high - low) * (to_high - to_low)
}

/// Distance from `p` to the nearest of `cells^3` feature points, one to a cell, scaled so that
/// zero is on a point and one is about as far from every point as a position can get.
///
/// Wraps at the edges of the unit cube, which is what makes the volume tile.
pub fn worley(p: [f32; 3], cells: u32, seed: u32) -> f32 {
    let scaled = [
        p[0] * cells as f32,
        p[1] * cells as f32,
        p[2] * cells as f32,
    ];
    let base = [
        scaled[0].floor() as i32,
        scaled[1].floor() as i32,
        scaled[2].floor() as i32,
    ];
    let mut nearest = f32::MAX;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let cell = [base[0] + dx, base[1] + dy, base[2] + dz];
                let point = feature(cell, cells, seed);
                let offset = [
                    cell[0] as f32 + point[0] - scaled[0],
                    cell[1] as f32 + point[1] - scaled[1],
                    cell[2] as f32 + point[2] - scaled[2],
                ];
                let square = offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2];
                nearest = nearest.min(square);
            }
        }
    }
    nearest.sqrt().min(1.0)
}

/// Where the feature point of a cell sits inside it. The cell index is wrapped first, so the
/// cell off one edge of the volume is the cell against the other one.
fn feature(cell: [i32; 3], cells: u32, seed: u32) -> [f32; 3] {
    let wrapped = [
        cell[0].rem_euclid(cells as i32) as u32,
        cell[1].rem_euclid(cells as i32) as u32,
        cell[2].rem_euclid(cells as i32) as u32,
    ];
    let hash = hash3(wrapped, seed);
    [
        ((hash & 0x3ff) as f32) / 1023.0,
        (((hash >> 10) & 0x3ff) as f32) / 1023.0,
        (((hash >> 20) & 0x3ff) as f32) / 1023.0,
    ]
}

/// Gradient noise stacked over `octaves`, starting at `cells` cells to an edge and doubling,
/// brought into `0..=1`.
pub fn perlin_fbm(p: [f32; 3], cells: u32, octaves: u32, seed: u32) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 0.5;
    let mut weight = 0.0;
    let mut frequency = cells;
    for octave in 0..octaves {
        total += amplitude * perlin(p, frequency, seed.wrapping_add(octave * 0x9e37));
        weight += amplitude;
        amplitude *= 0.5;
        frequency *= 2;
    }
    (total / weight * 0.5 + 0.5).clamp(0.0, 1.0)
}

/// Gradient noise on a lattice of `cells` cells to an edge, in `-1..=1`, wrapping at the edges
/// of the unit cube.
pub fn perlin(p: [f32; 3], cells: u32, seed: u32) -> f32 {
    let scaled = [
        p[0] * cells as f32,
        p[1] * cells as f32,
        p[2] * cells as f32,
    ];
    let base = [scaled[0].floor(), scaled[1].floor(), scaled[2].floor()];
    let frac = [
        scaled[0] - base[0],
        scaled[1] - base[1],
        scaled[2] - base[2],
    ];
    // Quintic ease, so the second derivative is continuous and the lattice does not show up as
    // a grid of creases once the noise is lit.
    let ease = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let weight = [ease(frac[0]), ease(frac[1]), ease(frac[2])];

    let mut corners = [0.0f32; 8];
    for (index, corner) in corners.iter_mut().enumerate() {
        let step = [index & 1, (index >> 1) & 1, (index >> 2) & 1];
        let lattice = [
            (base[0] as i32 + step[0] as i32).rem_euclid(cells as i32) as u32,
            (base[1] as i32 + step[1] as i32).rem_euclid(cells as i32) as u32,
            (base[2] as i32 + step[2] as i32).rem_euclid(cells as i32) as u32,
        ];
        let gradient = gradient_of(lattice, seed);
        let offset = [
            frac[0] - step[0] as f32,
            frac[1] - step[1] as f32,
            frac[2] - step[2] as f32,
        ];
        *corner = gradient[0] * offset[0] + gradient[1] * offset[1] + gradient[2] * offset[2];
    }

    let blend = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x0 = blend(corners[0], corners[1], weight[0]);
    let x1 = blend(corners[2], corners[3], weight[0]);
    let x2 = blend(corners[4], corners[5], weight[0]);
    let x3 = blend(corners[6], corners[7], weight[0]);
    let y0 = blend(x0, x1, weight[1]);
    let y1 = blend(x2, x3, weight[1]);
    blend(y0, y1, weight[2])
}

/// One of the twelve edge-of-a-cube directions, picked by hash. Ken Perlin's own set: they are
/// evenly spread, and a dot product with one of them is two adds and no multiplies.
fn gradient_of(lattice: [u32; 3], seed: u32) -> [f32; 3] {
    const EDGES: [[f32; 3]; 12] = [
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [1.0, -1.0, 0.0],
        [-1.0, -1.0, 0.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
        [1.0, 0.0, -1.0],
        [-1.0, 0.0, -1.0],
        [0.0, 1.0, 1.0],
        [0.0, -1.0, 1.0],
        [0.0, 1.0, -1.0],
        [0.0, -1.0, -1.0],
    ];
    EDGES[(hash3(lattice, seed) % 12) as usize]
}

/// A hash of three lattice coordinates and a seed. Nothing clever, but it has to be stable
/// across runs and machines, so it is written out rather than borrowed from a hasher whose
/// output is allowed to change.
fn hash3(at: [u32; 3], seed: u32) -> u32 {
    let mut hash = seed ^ 0x9e37_79b9;
    for value in at {
        hash ^= value.wrapping_mul(0x85eb_ca6b);
        hash = hash.wrapping_mul(0xc2b2_ae35);
        hash ^= hash >> 15;
    }
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0x27d4_eb2f);
    hash ^ (hash >> 16)
}
