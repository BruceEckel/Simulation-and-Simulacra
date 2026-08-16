//! The one sheet of noise the whole sky is read out of.
//!
//! Four channels of tiling two-dimensional noise, with a mip chain built on top of them. The
//! shader reads a cloud deck by intersecting a ray with a horizontal plane and sampling this at
//! the point it lands, so one texture fetch is a whole cloud field: no marching, no volume, no
//! octaves evaluated per pixel.
//!
//! That is the entire economy of this piece. Reading a cloud costs about what reading a
//! photograph costs, which is what leaves enough budget to compute every pixel of a large
//! display, every frame, and still have room for the seven or eight extra taps that the
//! shading and the line need.
//!
//! Three things about it matter enough to be worth stating:
//!
//! - **It tiles.** The lattice wraps, so the sheet can be laid over a sky tens of kilometres
//!   across with repeat addressing and no seam. The tests hold it to that.
//! - **It carries its own mip chain.** A cloud plane seen at a grazing angle near the horizon
//!   is minified enormously, and a sheet without mips turns that into a field of sparkling
//!   confetti. wgpu will not build the chain, so it is box-filtered here, wrapping at the edges
//!   like everything else.
//! - **Two of its channels are the same kind of noise at different frequencies.** The shader
//!   scrolls them at different speeds and adds them, which is what makes the clouds *boil*
//!   rather than slide: the sum of two things moving at different speeds is not a translation
//!   of anything.

/// A tiling sheet of noise and its mip chain, `levels[0]` full size and each one after it half
/// the last, down to a single texel.
pub struct Sheet {
    /// Texels along each edge of the top level.
    pub size: u32,
    /// RGBA8 texels, one `Vec` per mip level.
    pub levels: Vec<Vec<u8>>,
}

impl Sheet {
    /// One channel of one texel of the top level, `0..=255`.
    pub fn at(&self, x: u32, y: u32, channel: usize) -> u8 {
        let index = ((y % self.size) * self.size + (x % self.size)) as usize;
        self.levels[0][index * 4 + channel]
    }
}

/// Edge of the sheet, in texels. A power of two so the mip chain halves exactly, which is what
/// keeps the smaller levels tiling as cleanly as the largest.
pub const SHEET_SIZE: u32 = 512;

/// How many cells across the sheet each channel's lowest octave has.
///
/// The two cloud channels are deliberately close but not related by a whole number: laid over
/// each other they beat against one another for a very long way before the pattern repeats,
/// which is what stops a tiling sheet from reading as tiles.
const CELLS: [u32; 4] = [3, 5, 2, 11];

/// How many octaves each channel stacks.
const OCTAVES: [u32; 4] = [3, 3, 2, 3];

/// Build the sheet.
///
/// - `r` and `g` are the two cloud fields, the ones that are added and scrolled apart
/// - `b` is the weather map: two cells across the whole sheet, so it varies over the width of
///   the sky and decides which districts are cloudy
/// - `a` is the fine detail the edges are roughened with
pub fn sheet(seed: u32) -> Sheet {
    let size = SHEET_SIZE;
    let mut field = vec![[0.0f32; 4]; (size * size) as usize];
    for (index, texel) in field.iter_mut().enumerate() {
        let x = (index as u32 % size) as f32 + 0.5;
        let y = (index as u32 / size) as f32 + 0.5;
        let at = [x / size as f32, y / size as f32];
        for channel in 0..4 {
            texel[channel] = fbm(
                at,
                CELLS[channel],
                OCTAVES[channel],
                // Wrapping on both halves: the multiply overflows a `u32` from the third
                // channel on, which a release build folds away silently and a debug build
                // panics on. Every arithmetic step that feeds a hash has to say out loud that
                // it means to wrap.
                seed.wrapping_add((channel as u32).wrapping_mul(0x9e37_79b9)),
            );
        }
    }

    // Stretched over the byte each channel is stored in. Stacked octaves of gradient noise pile
    // up around the middle of their range and leave both ends empty, and every threshold in the
    // shader is a threshold *against these numbers*: a channel using half its range gives a
    // coverage dial with half its travel and twice the terracing.
    let mut top = vec![0u8; field.len() * 4];
    for channel in 0..4 {
        let (low, high) = window(&field, channel);
        let span = (high - low).max(1e-4);
        for (texel, out) in field.iter().zip(top.chunks_mut(4)) {
            out[channel] = (((texel[channel] - low) / span).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }

    let mut levels = vec![top];
    let mut level_size = size;
    while level_size > 1 {
        levels.push(halve(levels.last().expect("a level to halve"), level_size));
        level_size /= 2;
    }
    Sheet { size, levels }
}

/// One mip level from the one above it: every texel the mean of the four under it.
fn halve(level: &[u8], size: u32) -> Vec<u8> {
    let half = size / 2;
    let mut out = vec![0u8; (half * half * 4) as usize];
    for y in 0..half {
        for x in 0..half {
            for channel in 0..4 {
                let mut total = 0u32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let sx = x * 2 + dx;
                        let sy = y * 2 + dy;
                        total += u32::from(level[((sy * size + sx) * 4 + channel as u32) as usize]);
                    }
                }
                out[((y * half + x) * 4 + channel as u32) as usize] = (total / 4) as u8;
            }
        }
    }
    out
}

/// The range a channel really occupies, trimmed at both ends so one freak texel cannot set the
/// scale for the whole sheet.
fn window(field: &[[f32; 4]], channel: usize) -> (f32, f32) {
    let mut sample: Vec<f32> = field.iter().map(|texel| texel[channel]).collect();
    sample.sort_by(|a, b| a.partial_cmp(b).expect("the field has no NaNs in it"));
    let last = sample.len() - 1;
    (
        sample[(0.005 * last as f32) as usize],
        sample[(0.995 * last as f32) as usize],
    )
}

/// Gradient noise stacked over `octaves`, starting at `cells` cells to an edge and doubling.
pub fn fbm(at: [f32; 2], cells: u32, octaves: u32, seed: u32) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 0.5;
    let mut weight = 0.0;
    let mut frequency = cells;
    for octave in 0..octaves {
        total += amplitude * gradient_noise(at, frequency, seed.wrapping_add(octave * 0x85eb));
        weight += amplitude;
        amplitude *= 0.5;
        frequency *= 2;
    }
    (total / weight * 0.5 + 0.5).clamp(0.0, 1.0)
}

/// Gradient noise on a lattice of `cells` cells to an edge, in `-1..=1`, wrapping at the edges
/// of the unit square.
pub fn gradient_noise(at: [f32; 2], cells: u32, seed: u32) -> f32 {
    let scaled = [at[0] * cells as f32, at[1] * cells as f32];
    let base = [scaled[0].floor(), scaled[1].floor()];
    let frac = [scaled[0] - base[0], scaled[1] - base[1]];
    // Quintic ease: its second derivative is continuous, so the lattice does not show up as a
    // grid of creases once the field is lit and, worse, contoured.
    let ease = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let weight = [ease(frac[0]), ease(frac[1])];

    let mut corner = [0.0f32; 4];
    for (index, value) in corner.iter_mut().enumerate() {
        let step = [index & 1, (index >> 1) & 1];
        let lattice = [
            (base[0] as i32 + step[0] as i32).rem_euclid(cells as i32) as u32,
            (base[1] as i32 + step[1] as i32).rem_euclid(cells as i32) as u32,
        ];
        let angle = hash2(lattice, seed) as f32 * (std::f32::consts::TAU / 4_294_967_296.0);
        let (sin, cos) = angle.sin_cos();
        *value = cos * (frac[0] - step[0] as f32) + sin * (frac[1] - step[1] as f32);
    }

    let blend = |a: f32, b: f32, t: f32| a + (b - a) * t;
    blend(
        blend(corner[0], corner[1], weight[0]),
        blend(corner[2], corner[3], weight[0]),
        weight[1],
    )
}

/// A hash of two lattice coordinates and a seed. Written out rather than borrowed from a
/// hasher, because the sky has to come out the same on every machine and in every release.
fn hash2(at: [u32; 2], seed: u32) -> u32 {
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
