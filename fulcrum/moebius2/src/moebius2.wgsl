// Moebius clouds: outlines built from circular arcs, filled flat, one colour to an enclosed
// region, in one pass.
//
// There is no field in here, no noise and no light. A cloud is a run of overlapping circles on
// the sphere of directions, and the drawing of it is two questions per pixel: is this pixel
// inside the union of those circles, and is it within a line's width of the edge. The groups
// arrive in the order they are to be drawn, so a group laid over another covers its fill and its
// outline together, which is how a crest erases the arc it stands on and leaves its own.
//
// The line around a cloud has its own width, in `counts.z`, and it is the one number in this
// shader a key can move while the drawing is running. Everything else in the panel keeps the
// fixed weight in `pen.x`: the clouds are the subject, and thickening the horizon along with them
// would change the paper rather than the drawing.
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
// Storage rather than uniform, because a crest is a union of up to six circles here and a busy
// frame carries more of them than a uniform binding holds.
struct SkyBuffer {
    // xyz: the centre of a cap holding the whole group. w: its angular radius.
    cap: array<vec4<f32>, 384>,
    // first circle, how many, which cloud colour, spare.
    span: array<vec4<f32>, 384>,
    // xyz: normal of the half-space the group is cut back to. w: where it is cut.
    plane: array<vec4<f32>, 384>,
    // xyz: a unit direction. w: an angular radius.
    disc: array<vec4<f32>, 4608>,
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
        var sd = 1e6;
        for (var i = 0u; i < discs; i = i + 1u) {
            let disc = s.disc[first + i];
            // The chord rather than the arc, which over the size of a cloud is the same number
            // to a fraction of a percent and is a subtraction instead of an arc cosine.
            sd = min(sd, length(dir - disc.xyz) - disc.w);
        }
        // Cut back to a half-space: the flat base of a cumulus, and the only edge in a cloud
        // that is not an arc.
        let plane = s.plane[g];
        sd = max(sd, plane.w - dot(dir, plane.xyz));

        let fill = 1.0 - smoothstep(-soft, soft, sd - half);
        let edge = 1.0 - smoothstep(-soft, soft, abs(sd) - half);
        colour = mix(colour, tint(u32(span.z)), fill);
        colour = mix(colour, u.ink.rgb, edge);
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

    return vec4<f32>(colour, 1.0);
}
