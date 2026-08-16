// Moebius clouds: outlines built from circular arcs, filled flat, one colour to an enclosed
// region, in one pass.
//
// There is no field in here and no noise. A cloud is a run of overlapping circles on the sphere
// of directions, and the drawing of it is two questions per pixel: is this pixel inside the union
// of those circles, and is it within a line's width of the edge. The groups arrive in the order
// they are to be drawn, so a group laid over another covers its fill and its outline together,
// which is how a crest erases the arc it stands on and leaves its own.
//
// There is one light, and it does one thing. `facing` says which way the cloud is turned at this
// pixel, taking the nearest circle for the billow the pixel stands on and the flat cut for the
// base, and `hatching` lays strokes wherever that surface is turned away from the sun and crosses
// them wherever it is turned towards the ground. The flat base of a cumulus is not asked: it faces
// the ground, nothing under the horizon lights it, so it is always in the deepest shade. That is
// the whole of the shading in this piece and the whole of the departure from the two before it: no
// tone anywhere, no colour mixed with another to stand for a surface turning away, just three flat
// levels and a pen.
//
// The strokes are drawn as a hand draws them rather than as a ruler does: no two the same distance
// apart, no two the same weight, each one wandering as it is pulled and pressed harder in the
// middle of the pull than at the ends of it, and all of them stopping short of the outline by a
// distance that changes from stroke to stroke.
//
// The line around a cloud has its own width, in `counts.z`, and it is the one number in this
// shader a key can move while the drawing is running. Everything else in the panel keeps the
// fixed weight in `pen.x`: the clouds are the subject, and thickening the horizon along with them
// would change the paper rather than the drawing. The hatch lines take the cloud's width, because
// they are the same pen.
//
// The desert underneath is the other half of the panel and is drawn the way the sky is: flat
// bands with a line along each boundary. Those lines are analytic, taken from how fast a value
// changes from one pixel to the next, so they are the same weight everywhere. The clouds do not
// need that trick, because a circle on the sky knows its own size in radians and a pixel knows
// how many radians it covers.

struct Uniforms {
    // xyz: the eye, in metres. w: tan of half the vertical field of view.
    origin: vec4<f32>,
    // xyz: where it looks. w: aspect ratio.
    forward: vec4<f32>,
    // xyz: its right. w: window width in pixels.
    right: vec4<f32>,
    // xyz: its up. w: window height in pixels.
    up: vec4<f32>,
    // xyz: towards the sun. w: cosine of its angular radius.
    sun: vec4<f32>,
    // the line everything but a cloud is drawn with, sky bands, ground bands, how far the rock
    // rises.
    pen: vec4<f32>,
    // planet radius, furthest the ground is drawn, ground tile, how far a line is feathered.
    world: vec4<f32>,
    // how many groups of circles there are, where the ring around the sun stands, how wide the
    // line around a cloud is in pixels, and how many arcs an element was built from.
    counts: vec4<f32>,
    // hatch spacing as a fraction of an element radius, how far a surface is turned down before it
    // is crossed with a second set, the angle they are drawn at, and nought for no shading at all.
    shade: vec4<f32>,
    // xyz: the direction to the sand under the horse. w: how tall he is, in radians.
    rider: vec4<f32>,
    // where he is in his stride, how wide the frame is in bearing, and two spare.
    gait: vec4<f32>,
    // The five flat colours of the sky, horizon first.
    sky_0: vec4<f32>,
    sky_1: vec4<f32>,
    sky_2: vec4<f32>,
    sky_3: vec4<f32>,
    sky_4: vec4<f32>,
    // The three of the desert, underfoot first.
    sand_0: vec4<f32>,
    sand_1: vec4<f32>,
    sand_2: vec4<f32>,
    sun_colour: vec4<f32>,
    ink: vec4<f32>,
    mesa: vec4<f32>,
    // The four flat colours a cloud is filled with, far band first.
    cloud_0: vec4<f32>,
    cloud_1: vec4<f32>,
    cloud_2: vec4<f32>,
    cloud_3: vec4<f32>,
    // width, height, 1/width, 1/height.
    screen: vec4<f32>,
    // The rock: where round the compass, how wide, how high, how far the top is tipped.
    rock: array<vec4<f32>, 24>,
}

// The sky, rebuilt every frame: only the groups in front of you, in the order they are drawn.
// Storage rather than uniform, because a crest is a union of up to two dozen circles here and a
// busy frame carries far more of them than a uniform binding holds.
struct SkyBuffer {
    // xyz: the centre of a cap holding the whole group. w: its angular radius.
    cap: array<vec4<f32>, 384>,
    // first circle, how many, which cloud colour, spare.
    span: array<vec4<f32>, 384>,
    // xyz: normal of the half-space the group is cut back to. w: where it is cut.
    plane: array<vec4<f32>, 384>,
    // xyz: a unit direction. w: an angular radius.
    disc: array<vec4<f32>, 18432>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var<storage, read> s: SkyBuffer;

struct Varying {
    @builtin(position) place: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Varying {
    var out: Varying;
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    out.uv = uv;
    out.place = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return out;
}

// ---------------------------------------------------------------------------------------
// lines
// ---------------------------------------------------------------------------------------

// How wide, in the units of `value`, a line of `pixels` pixels is here.
//
// `fwidth` says how much `value` changes from one pixel to the next, so dividing a pixel count
// by it converts a width on the screen into a width in whatever `value` happens to measure. It
// is what keeps the desert's bands and the horizon at one weight however far away they are, and
// it is only used on the ground and the sky: a cloud is measured in radians, and so is a pixel.
fn line_width(value: f32, pixels: f32) -> f32 {
    return max(fwidth(value), 1e-7) * pixels * 0.5;
}

// How much of a line is worth drawing at all.
//
// A line is only a line while the thing it follows is bigger than a pixel. Where a value swings
// through several bands from one pixel to the next, which is what every band on the ground does
// as it nears the horizon, asking for a line of constant width asks for a line wider than the
// shape it draws, and the honest answer is that there is nothing there to draw.
fn legible(gradient: f32) -> f32 {
    return 1.0 - smoothstep(0.35, 0.9, gradient);
}

// One over the boundary at `level`: 1 on the line, 0 off it, with a pixel of softness.
fn stroke(value: f32, level: f32, pixels: f32) -> f32 {
    let half = line_width(value, pixels);
    return 1.0 - smoothstep(half * 0.55, half, abs(value - level));
}

// Lines wherever `value` crosses one of `bands` evenly spaced levels.
fn contours(value: f32, bands: f32, pixels: f32) -> f32 {
    let stepped = value * bands;
    let gap = 0.5 - abs(fract(stepped) - 0.5);
    let half = line_width(stepped, pixels);
    let on = 1.0 - smoothstep(half * 0.55, half, gap);
    return on * legible(fwidth(stepped));
}

// `value` flattened into `bands` steps, given back in nought to one, and unflattened again
// wherever the steps have grown smaller than a pixel: a staircase sampled below its own tread
// size is not a staircase, it is moire.
fn flatten(value: f32, bands: f32) -> f32 {
    let stepped = (floor(clamp(value, 0.0, 0.999) * bands) + 0.5) / bands;
    return mix(stepped, clamp(value, 0.0, 1.0), 1.0 - legible(fwidth(value * bands)));
}

// ---------------------------------------------------------------------------------------
// the sky behind everything
// ---------------------------------------------------------------------------------------

// One of the five flat colours the palette gives the sky.
//
// `at` is a band, not a height: [`flatten`] has already cut the sky into five, and a band lands
// on one of the five colours rather than somewhere between two of them. The mix is here for the
// one case that needs it, where the bands have gone smaller than a pixel and the stepping has
// been undone to stop the moire.
fn sky_shelf(at: f32) -> vec3<f32> {
    let place = clamp(at, 0.0, 1.0) * 4.0;
    let low = floor(place);
    var a = u.sky_0.rgb;
    var b = u.sky_1.rgb;
    if (low >= 3.0) {
        a = u.sky_3.rgb;
        b = u.sky_4.rgb;
    } else if (low >= 2.0) {
        a = u.sky_2.rgb;
        b = u.sky_3.rgb;
    } else if (low >= 1.0) {
        a = u.sky_1.rgb;
        b = u.sky_2.rgb;
    }
    return mix(a, b, place - low);
}

// One of the three the desert has.
fn sand_shelf(at: f32) -> vec3<f32> {
    let place = clamp(at, 0.0, 1.0) * 2.0;
    let low = floor(place);
    var a = u.sand_0.rgb;
    var b = u.sand_1.rgb;
    if (low >= 1.0) {
        a = u.sand_1.rgb;
        b = u.sand_2.rgb;
    }
    return mix(a, b, place - low);
}

// A band of `bands` steps, turned into the nought-to-one a shelf is picked with.
fn shelf_of(stepped: f32, bands: f32) -> f32 {
    return (stepped * bands - 0.5) / (bands - 1.0);
}

// Five flat bands from the horizon to the zenith with a line between each pair, and the sun on
// top as a disc with a ring around it. This sky is what the clouds are drawn on, not what the
// picture is about.
fn draw_sky(dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(dir.y, 0.0, 1.0);
    // Bunched towards the horizon, which is both what the air does and what stops five bands
    // reading as five stripes.
    let height = pow(up, 0.55);
    let stepped = flatten(height, u.pen.y);
    var colour = sky_shelf(shelf_of(stepped, u.pen.y));
    colour = mix(colour, u.ink.rgb, contours(height, u.pen.y, u.pen.x) * 0.5);

    // A flat disc, its outline, and a ring standing off it.
    //
    // The ring is not decoration. Everything else in this sky is a flat area with an arc round
    // it, so a sun drawn the same way is a cloud that happens to be circular, and the eye files
    // it with the clouds. One concentric line is enough to say that this is the other thing.
    let towards = dot(dir, u.sun.xyz);
    let disc = smoothstep(u.sun.w, u.sun.w + 0.00012, towards);
    colour = mix(colour, u.sun_colour.rgb, disc);
    colour = mix(colour, u.ink.rgb, stroke(towards, u.sun.w, u.pen.x));
    colour = mix(colour, u.ink.rgb, stroke(towards, u.counts.y, u.pen.x));
    return colour;
}

// ---------------------------------------------------------------------------------------
// the desert
// ---------------------------------------------------------------------------------------

// An angle brought back into plus or minus pi.
fn wrap(angle: f32) -> f32 {
    let turn = 6.283185307;
    return angle - turn * round(angle / turn);
}

// How high the rock stands at this bearing, in the units of `dir.y`.
//
// Twenty-four blocks around the compass, flat on top and straight down the sides, with the
// tallest of them winning wherever two overlap. Moebius draws his deserts that way: the ground is
// a ruled line with a few slabs standing on it, and every curve in the panel belongs to the sky.
// Branch-free, so that the derivatives taken after it still mean what they say.
fn rock_profile(bearing: f32) -> f32 {
    var rise = 0.0;
    for (var i = 0u; i < 24u; i = i + 1u) {
        let rock = u.rock[i];
        let x = wrap(bearing - rock.x) / max(rock.y, 1e-4);
        // A flat top with a short slope at each end, rather than a curve.
        let shoulder = clamp((1.0 - abs(x)) * 3.5, 0.0, 1.0);
        let top = rock.z * (1.0 + rock.w * x) * u.pen.w;
        rise = max(rise, top * shoulder * step(abs(x), 1.0));
    }
    return rise;
}

// The sand: three flat bands running away to the horizon with a line along each boundary, swung
// about by two sine waves so the boundaries are long shallow curves rather than ruled arcs.
//
// The bands are spread over the angle below the horizon rather than over the distance along the
// ground. Distance is the honest measure and it is the wrong one here: with the frame pointed up
// at the clouds there is a hand's breadth of desert at the bottom of it, and everything past a
// kilometre away lands in the top fifth of that. Spread by distance, the desert is one flat
// colour with a bundle of lines along the top; spread by angle, it is three bands with room to be
// bands in. The eye never moves, so the two differ by a mapping and by nothing else.
fn draw_ground(dir: vec3<f32>, bearing: f32) -> vec3<f32> {
    let dip = clamp(-dir.y / 0.108, 0.0, 1.0);
    let away = 1.0 - pow(dip, 0.80);
    // Whole numbers of waves around the compass, and for a reason that is invisible until you
    // turn far enough to find it: the bearing runs from minus pi to pi and then starts again, so
    // a wave that does not close on itself over a full turn leaves a straight vertical seam
    // across the desert at due south, where the two ends of it meet and do not match.
    let sway = 0.055 * sin(bearing * 2.0 + 0.7) + 0.028 * sin(bearing * 5.0 - 1.9);
    let value = clamp(away + sway * (1.0 - away * away), 0.0, 1.0);
    let stepped = flatten(value, u.pen.z);
    let colour = sand_shelf(shelf_of(stepped, u.pen.z));
    return mix(colour, u.ink.rgb, contours(value, u.pen.z, u.pen.x));
}

// ---------------------------------------------------------------------------------------
// the clouds
// ---------------------------------------------------------------------------------------

// Which way the cloud is facing at this pixel.
//
// A cloud here is a union of balls with a flat base cut across it, and both parts of that have a
// surface a normal can be taken off. Which one this pixel stands on is already known: `lobe` is
// the circle that came nearest in the distance loop, so it is the billow this part of the cloud
// belongs to, and `on_base` says the flat cut won instead.
//
// The base is the easy half. It is a horizontal plane facing down, so its normal is the plane's
// own, turned over. Nothing above the horizon can light a surface pointing at the ground, which
// is why the bottom of a cumulus is the darkest part of it.
//
// The billow is a ball. A pixel a fraction of the radius out from the middle of one stands on a
// surface leaning that far off the way the ball faces, which is towards the eye, and turned in
// the direction it is out. That is a normal per billow rather than per cloud, and it is the whole
// of the difference between shading that follows the form and shading that sits on one side of
// the silhouette.
fn facing(dir: vec3<f32>, lobe: vec4<f32>, plane: vec4<f32>, on_base: bool) -> vec3<f32> {
    if (on_base) {
        return -plane.xyz;
    }
    let centre = lobe.xyz;
    let radius = max(lobe.w, 1e-6);
    let off = dir - centre;
    // Only the part of the offset that runs across the face of the ball says which way it leans.
    let across = off - centre * dot(off, centre);
    let lean = min(length(across) / radius, 1.0);
    // And the rest of it is how far the surface is turned towards the eye: all of it in the
    // middle of a billow, none of it at the rim.
    var normal = -centre * sqrt(max(1.0 - lean * lean, 0.0));
    if (dot(across, across) > 1e-18) {
        normal = normal + normalize(across) * lean;
    }
    return normal;
}

// A wander with no repeat you can see, in about plus or minus one.
//
// Three sines at frequencies that are not multiples of each other. It is not noise and does not
// want to be: a hand wobbles smoothly, so what this has to give is a line that is never quite
// straight rather than a line with grit on it.
fn waver(t: f32) -> f32 {
    return 0.54 * sin(t) + 0.31 * sin(t * 2.37 + 1.73) + 0.15 * sin(t * 5.11 + 4.19);
}

// A number in nought to one from the number of a stroke, so no two strokes of a set are laid down
// alike. The argument is a small whole number, which is the one range this hash is good over.
fn unsteady(n: f32) -> f32 {
    return fract(sin(n * 12.9898 + 78.233) * 43758.5453);
}

// How much ink stroke `n` of a set puts on this pixel.
//
// `place` is where the pixel stands across the set, counted in spacings, and `across` is how far
// along the stroke it lies, in radians. Four things are wrong with the stroke on purpose: it is
// shifted off its even place, it is drawn at its own weight, it wanders as it is pulled, and the
// pressure comes and goes along the pull. Those four are the difference between hatching that was
// ruled and hatching that was drawn, and none of them is random from one frame to the next: all of
// it is a function of where the stroke is on the element, so it holds still while the cloud
// drifts.
fn stroke_ink(
    place: f32,
    n: f32,
    across: f32,
    key: f32,
    spacing: f32,
    half: f32,
    soft: f32,
) -> f32 {
    let shift = (unsteady(n + key) - 0.5) * 0.30;
    let weight = 0.66 + 0.78 * unsteady(n + key + 31.0);
    // A wave about nine spacings long, with the two shorter ones `waver` carries riding on it, and
    // a phase of its own for every stroke: a set that wandered in step would be a ruled set drawn
    // on a warped sheet, which is a different thing from a hand and reads as one.
    let wander = 0.26 * waver(across / (spacing * 1.45) + unsteady(n + key + 61.0) * 6.2831853);
    let press = 1.0 + 0.26 * waver(across / (spacing * 1.9) + n + key);
    let gap = abs(place - n - shift - wander) * spacing;
    return 1.0 - smoothstep(-soft, soft, gap - half * weight * press);
}

// One set of strokes, and the pixel measured against the two it sits between.
//
// Both are needed because a stroke no longer sits in the middle of its own slot. One test alone
// would measure every pixel against the stroke whose slot it fell in, and a stroke that had leaned
// past the halfway line would come out cut off flat down the middle. `key` numbers the set, so the
// two sets that cross on an underside are not the same set turned a right angle.
fn hatch_set(along: f32, across: f32, key: f32, spacing: f32, half: f32, soft: f32) -> f32 {
    let place = along / spacing;
    let n = round(place);
    return max(
        stroke_ink(place, n, across, key, spacing, half, soft),
        stroke_ink(place, n + sign(place - n), across, key, spacing, half, soft),
    );
}

// The shading on one element, as how much ink is on this pixel.
//
// This is the one place in the whole set of pieces where a light direction does more than place
// the sun's disc, and it does it without a tone anywhere: the answer is a line, two crossed lines,
// or nothing. Hatching is how a pen shades, and a pen has one colour.
//
// Three flat levels, picked by two questions about the surface. Is it turned away from the sun,
// which is what a shadow is: if not, it is left as flat colour. Is it turned towards the ground:
// if it is, it gets a second set of lines across the first, because nothing above the horizon
// lights a downward face and the undersides of a cumulus, its flat base most of all, are the
// darkest part of it. Three levels chosen by a decision are the same thing the sky does with its
// five flat bands. What they are not is a ramp.
//
// The lines are spaced across the element rather than across the screen, so a small cloud gets a
// few and a big one gets a dozen, and they hold still while it drifts instead of sliding under it
// like a screen door. Their slant comes off a frame that has nothing to do with the sun, because
// a hand hatches at one slant across a whole drawing: take the slant off the sun and every cloud
// passing near it turns its hatching like a wheel.
fn hatching(
    dir: vec3<f32>,
    cap: vec4<f32>,
    normal: vec3<f32>,
    lobe: vec4<f32>,
    on_base: bool,
    sd: f32,
    half: f32,
    soft: f32,
) -> f32 {
    if (u.shade.w < 0.5) {
        return 0.0;
    }
    let n = cap.xyz;
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), n));
    let up = cross(n, right);
    let off = dir - n;
    let x = dot(off, right);
    let y = dot(off, up);

    // In shadow, and turned down. Both are hard boundaries with a pixel of softness on them,
    // which is the only softness this drawing allows itself.
    //
    // The base is not asked about the sun at all. It faces the ground, and nothing above the
    // horizon lights a downward face, so it is in the deepest shade whatever the sun is doing and
    // whatever the cloud is doing: a cumulus with a flat bottom has that bottom hatched, always.
    // Putting the question to the light gave the wrong answer for a cloud high in the sky towards
    // a low sun, where the cut is tipped enough off the horizontal to catch it, and a flat base
    // left the colour it was filled with is the one thing that turns a heap back into a balloon.
    //
    // A billow is a different case and does have to be asked. It is curved, and the last sliver of
    // one before it turns away is a shadow too thin to draw, so the shading on a billow starts a
    // little past the terminator: a hand does not pull three-pixel strokes along the top of a bump.
    // How wide that boundary is comes from how fast the surface turns, which is one pixel over the
    // radius of the billow, held to a width that cannot become a wash. A soft edge over a whole
    // shape is a grey, and a grey is the one thing this drawing does not have.
    var shadow = 1.0;
    if (!on_base) {
        let lit = dot(normal, u.sun.xyz) + 0.12;
        let edge = min(soft / max(lobe.w, 1e-6), 0.05);
        shadow = 1.0 - smoothstep(-edge, edge, lit);
    }
    let under = smoothstep(u.shade.y - 0.06, u.shade.y + 0.06, -normal.y);

    // The strokes. The second set crosses the first, and only the undersides get it. It crosses a
    // little off the square, because a hand turning the paper to cross a shade does not turn it a
    // right angle, and two sets meeting at ninety degrees are a grid.
    let spacing = u.shade.x * cap.w;
    let along = x * cos(u.shade.z) + y * sin(u.shade.z);
    let across = x * cos(u.shade.z + 1.36) + y * sin(u.shade.z + 1.36);
    let one = hatch_set(along, across, 0.0, spacing, half, soft);
    let two = hatch_set(across, along, 17.0, spacing, half, soft);
    let ink = max(one, two * under);

    // Inside the shape, and stopped short of the outline already running along its edge. How far
    // short changes from one stroke to the next, because a hand pulls a stroke until it is near
    // enough to the edge and lifts the pen: stopping every one of them at the same distance leaves
    // a band of even width inside the outline, and an even band is the mark of a machine.
    //
    // Three widths of the pen at most, whatever the spacing is. This is the wobble in a hand, and
    // a hand's wobble is the size of the nib rather than the size of the gap it is leaving: taken
    // off the spacing alone it would eat a tenth of the element at the loosest setting, and the
    // strip along a flat base is not a tenth of an element deep.
    let short = (0.5 + 0.5 * waver(along / (spacing * 2.4) + 2.0)) * min(spacing * 0.34, half * 3.0);
    let inside = 1.0 - smoothstep(-soft, soft, sd + half + short);

    // A hatch line is only a line while there is paper between it and the next one. On a small
    // element, or under a heavy pen, the lines meet and the shade becomes a grey fill drawn the
    // slow way, so it is taken away before it gets there. Measured against a little more than the
    // nominal width, since a stroke pressed hard is half again as wide as one pressed light.
    let open = smoothstep(1.3, 2.6, spacing / max(half * 2.4, 1e-9));

    return ink * shadow * inside * open;
}

// One of the palette's four flat cloud colours.
fn tint(index: u32) -> vec3<f32> {
    var colour = u.cloud_0.rgb;
    if (index == 1u) {
        colour = u.cloud_1.rgb;
    } else if (index == 2u) {
        colour = u.cloud_2.rgb;
    } else if (index >= 3u) {
        colour = u.cloud_3.rgb;
    }
    return colour;
}

// Every group, in order, over whatever is already there.
//
// `sd` is the distance from this pixel to the edge of the group, in radians, and the whole
// drawing comes off it: negative is inside, so it is filled, and small in either direction is
// the edge, so it is inked. Because the fill goes down before the line and the next group's fill
// goes down after both, the arcs of an earlier circle stop dead where a later one covers them,
// which is what a pen and an eraser do and is why these clouds look built rather than sampled.
//
// The line is `counts.z` pixels wide. Widening it moves the outline outwards as well as inwards,
// since the line is centred on the edge, so a fat setting does not eat the fill: the shape keeps
// its size and grows a heavier border.
fn draw_clouds(dir: vec3<f32>, under: vec3<f32>, pixel: f32) -> vec3<f32> {
    var colour = under;
    let half = pixel * u.counts.z * 0.5;
    let soft = pixel * u.world.w;
    let count = u32(u.counts.x);
    for (var g = 0u; g < count; g = g + 1u) {
        // The cap around the whole group, which throws away most of the sky for a few flops.
        //
        // Two tests, cheap one first. A group's cap cannot reach further from its centre in
        // height than its radius, and height alone rejects three quarters of the sky for one
        // subtraction: the clouds are spread up the picture, and any given pixel is at the
        // height of only a few of them. Whatever survives that gets the round test.
        let cap = s.cap[g];
        if (abs(dir.y - cap.y) > cap.w) {
            continue;
        }
        let off = dir - cap.xyz;
        if (dot(off, off) > cap.w * cap.w) {
            continue;
        }
        let span = s.span[g];
        let first = u32(span.x);
        let discs = u32(span.y);
        // The nearest circle is kept along with the distance to it. It costs a comparison in a
        // loop that was already comparing, and it is what says which billow of the cloud this
        // pixel is standing on, which is what the shading is worked out from.
        var sd = 1e6;
        var lobe = vec4<f32>(0.0, 0.0, 1.0, 1.0);
        for (var i = 0u; i < discs; i = i + 1u) {
            let disc = s.disc[first + i];
            // The chord rather than the arc, which over the size of a cloud is the same number
            // to a fraction of a percent and is a subtraction instead of an arc cosine.
            let reach = length(dir - disc.xyz) - disc.w;
            if (reach < sd) {
                sd = reach;
                lobe = disc;
            }
        }
        // Cut back to a half-space: the flat base of a cumulus, and the only edge in a cloud
        // that is not an arc. Where the cut is the nearer of the two, this pixel is standing on
        // the base rather than on any billow, and the base faces the ground.
        let plane = s.plane[g];
        let base = plane.w - dot(dir, plane.xyz);
        // Nearer than a fraction of the way, rather than simply nearer. Both distances are
        // negative inside, and the cut only has to beat two fifths of the billow's depth to count
        // as the surface here. The base of a cumulus is seen almost edge on from underneath, so a
        // band as deep as the billow is tall would be a wall rather than a floor.
        let on_base = base > sd * 0.4;
        sd = max(sd, base);

        // Fill, then the shading on it, then the outline over both. The order is a hand's: the
        // flat colour goes down, the shading goes on it, and the line is drawn last so nothing
        // crosses it.
        let fill = 1.0 - smoothstep(-soft, soft, sd - half);
        let edge = 1.0 - smoothstep(-soft, soft, abs(sd) - half);
        let normal = facing(dir, lobe, plane, on_base);
        colour = mix(colour, tint(u32(span.z)), fill);
        colour = mix(
            colour,
            u.ink.rgb,
            hatching(dir, cap, normal, lobe, on_base, sd, half, soft),
        );
        colour = mix(colour, u.ink.rgb, edge);
    }
    return colour;
}

// ---------------------------------------------------------------------------------------
// the traveller
// ---------------------------------------------------------------------------------------

// How far `p` is from the segment running from `a` to `b`.
fn bone(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let along = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-9), 0.0, 1.0);
    return length(pa - ba * along);
}

// A man on a horse, walking, in a box one unit tall with the sand at `y` nought and the middle
// of the horse at `x` nought. He faces the way he is going, which is his own negative `x`.
//
// Every part of him is a segment with a radius, which is the shape a pen leaves and unions as
// cleanly as the circles a cloud is built from. What comes back is a distance, filled where it
// is negative and inked where it is near nought, so he is drawn by the rules the clouds are
// drawn by and needs nothing of his own.
//
// The legs are the only part that moves. A walk is two diagonal pairs half a stride out of step,
// and the swing is the whole of it: there are no knees in here, because a knee at this size is
// one pixel of argument.
fn traveller(p: vec2<f32>, gait: f32) -> f32 {
    let swing = sin(gait) * 0.10;
    let other = sin(gait + 3.14159265) * 0.10;

    // The horse: barrel, neck, head, tail.
    var d = bone(p, vec2<f32>(-0.14, 0.40), vec2<f32>(0.15, 0.41)) - 0.095;
    d = min(d, bone(p, vec2<f32>(0.16, 0.47), vec2<f32>(0.26, 0.60)) - 0.050);
    d = min(d, bone(p, vec2<f32>(0.26, 0.61), vec2<f32>(0.37, 0.57)) - 0.041);
    d = min(d, bone(p, vec2<f32>(-0.22, 0.47), vec2<f32>(-0.31, 0.27)) - 0.025);

    // Four legs, near pair and far pair, each pair swinging against the other.
    d = min(d, bone(p, vec2<f32>(0.12, 0.35), vec2<f32>(0.12 + swing, 0.0)) - 0.023);
    d = min(d, bone(p, vec2<f32>(-0.11, 0.35), vec2<f32>(-0.11 + other, 0.0)) - 0.023);
    d = min(d, bone(p, vec2<f32>(0.08, 0.35), vec2<f32>(0.08 + other, 0.0)) - 0.021);
    d = min(d, bone(p, vec2<f32>(-0.15, 0.35), vec2<f32>(-0.15 + swing, 0.0)) - 0.021);

    // The man: leg down the barrel, body, arm to the reins, head, and the hat that says which
    // desert this is.
    d = min(d, bone(p, vec2<f32>(0.01, 0.52), vec2<f32>(0.09, 0.38)) - 0.028);
    // Narrower than his head on purpose. A body as wide as the head above it is one column of
    // ink at this size, and the head is what says there is a man up there rather than a post.
    d = min(d, bone(p, vec2<f32>(0.00, 0.50), vec2<f32>(0.03, 0.72)) - 0.038);
    d = min(d, bone(p, vec2<f32>(0.03, 0.66), vec2<f32>(0.14, 0.56)) - 0.024);
    d = min(d, bone(p, vec2<f32>(0.04, 0.78), vec2<f32>(0.04, 0.80)) - 0.050);
    d = min(d, bone(p, vec2<f32>(0.04, 0.855), vec2<f32>(0.04, 0.905)) - 0.036);
    d = min(d, bone(p, vec2<f32>(-0.06, 0.855), vec2<f32>(0.13, 0.855)) - 0.018);
    return d;
}

// The same direction, turned `angle` radians round the compass. He rides at one height, so a turn
// about the vertical is the whole of the move from one copy of him to the next.
fn turned(dir: vec3<f32>, angle: f32) -> vec3<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec3<f32>(dir.x * c + dir.z * s, dir.y, dir.z * c - dir.x * s);
}

// Him, on the sand, drawn over it.
//
// He is placed as a direction and a size rather than as a point in the world, so the drawing of
// him is the same two comparisons everything else here gets. `right` and `up` are tangents where
// he stands, and dividing by his angular height turns the sky into the box the shape is drawn in.
fn draw_one_rider(dir: vec3<f32>, under: vec3<f32>, pixel: f32, n: vec3<f32>) -> vec3<f32> {
    let size = u.rider.w;
    // A box around him, before any of the shape is worked out. He is one figure on a whole
    // compass, so nearly every pixel in the frame leaves here.
    let off = dir - n;
    if (dot(off, off) > size * size * 2.0) {
        return under;
    }
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), n));
    let up = cross(n, right);
    let p = vec2<f32>(dot(off, right), dot(off, up)) / size;
    let d = traveller(p, u.gait.x) * size;

    let half = pixel * u.pen.x * 0.5;
    let soft = pixel * u.world.w;
    // The rock's colour, because he stands on the same horizon as the rock and the palette has
    // one flat colour for the things that do.
    var colour = mix(under, u.mesa.rgb, 1.0 - smoothstep(-soft, soft, d - half));
    colour = mix(colour, u.ink.rgb, 1.0 - smoothstep(-soft, soft, abs(d) - half));
    return colour;
}

// And him again, a frame to either side, so that the picture is a loop.
//
// His bearing is folded into the width of the frame on the way in, which puts him on the screen at
// every moment; the two copies are what make the fold a walk rather than a jump. Without them the
// half of him hanging off one edge would appear at the other the instant his middle crossed, and
// with them the half leaving is the half arriving. At most two of the three are ever on the screen
// and the box test in `draw_one_rider` throws the others out for a subtraction each.
fn draw_rider(dir: vec3<f32>, under: vec3<f32>, pixel: f32) -> vec3<f32> {
    var colour = under;
    for (var copy = -1; copy <= 1; copy = copy + 1) {
        colour = draw_one_rider(dir, colour, pixel, turned(u.rider.xyz, f32(copy) * u.gait.y));
    }
    return colour;
}

// ---------------------------------------------------------------------------------------
// the frame
// ---------------------------------------------------------------------------------------

@fragment
fn fs_main(in: Varying) -> @location(0) vec4<f32> {
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let ray = u.forward.xyz
        + u.right.xyz * ndc.x * u.origin.w * u.forward.w
        + u.up.xyz * ndc.y * u.origin.w;
    let reach = length(ray);
    let dir = ray / reach;
    let bearing = atan2(dir.x, dir.z);

    // How many radians of sky one pixel covers, worked out from the projection rather than from
    // a derivative. It is what a circle's edge is measured against, and taking it analytically
    // is what lets the cloud loop branch: `fwidth` would have to be told what every pixel of a
    // quad is doing, and here they are all doing something different.
    let step_up = u.up.xyz * u.origin.w;
    let pixel = 2.0 * length(step_up - dir * dot(dir, step_up)) / (u.up.w * reach);

    // The sky, and then everything on the ground, all worked out before the first branch so that
    // every derivative in them means what it says.
    var colour = draw_sky(dir);

    let standing = dir.y - rock_profile(bearing);
    let rim = line_width(standing, 1.2);
    let is_rock = 1.0 - smoothstep(-rim, rim, standing);
    let rock_colour = mix(u.mesa.rgb, u.ink.rgb, stroke(standing, 0.0, u.pen.x));
    let sand = draw_ground(dir, bearing);
    let below = 1.0 - smoothstep(-0.0009, 0.0009, dir.y);
    let horizon = stroke(dir.y, 0.0, u.pen.x * 1.2);

    // The clouds sit over the sky and under the desert, so a cloud low enough to reach the
    // horizon is cut by the ground instead of hanging in front of it.
    colour = draw_clouds(dir, colour, pixel);

    colour = mix(colour, rock_colour, is_rock * step(0.0, dir.y));
    colour = mix(colour, sand, below);
    colour = mix(colour, u.ink.rgb, horizon);
    // Last, and over the sand he is riding on. He is the nearest thing in the picture and the
    // only one in it that is going anywhere.
    colour = draw_rider(dir, colour, pixel);

    return vec4<f32>(colour, 1.0);
}
