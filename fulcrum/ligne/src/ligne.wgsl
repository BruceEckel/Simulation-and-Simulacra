// Ligne claire clouds: three decks of live two-dimensional cloud field, drawn in flat colour
// with a clean line around everything, in one pass.
//
// There is no volume in here and nothing is marched. A cloud deck is a horizontal plane at a
// given altitude; a ray is intersected with it once, and the sheet of noise is read at the
// point where it lands. Everything after that -- the relief, the cast shadow, the flat bands,
// the line -- is worked out from that one field and its slope.
//
// The whole shader is deliberately branch-free. `fwidth` is what gives every line in the
// picture its constant width in *pixels* however far away the thing it is drawing is, and
// `fwidth` is only meaningful where all four pixels of a quad are doing the same thing. So the
// sky, the desert and all three decks are computed for every pixel and then selected between,
// which costs a little and buys a line that never thickens, never thins and never breaks.

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
    // xyz: how far the wind has carried the sky, in metres. w: how far the second field has
    // been carried past the first, which is what makes the clouds boil.
    wind: vec4<f32>,
    // Three decks, near one first: altitude, metres to one tile of the sheet, coverage
    // threshold, how far into the haze it is.
    deck_a: vec4<f32>,
    deck_b: vec4<f32>,
    deck_c: vec4<f32>,
    // relief in metres, shadow reach as a fraction of a tile, shadow strength, crown height.
    puff: vec4<f32>,
    // tone bands, ambient, light, how much of the tone is height.
    shade: vec4<f32>,
    // line width in pixels, sky bands, ground bands, how far the mesas rise.
    line: vec4<f32>,
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    sun_colour: vec4<f32>,
    ink: vec4<f32>,
    ground_near: vec4<f32>,
    ground_far: vec4<f32>,
    mesa: vec4<f32>,
    haze: vec4<f32>,
    // The flat ramp a cloud is filled from, rain-dark base to lit crown.
    cloud_0: vec4<f32>,
    cloud_1: vec4<f32>,
    cloud_2: vec4<f32>,
    cloud_3: vec4<f32>,
    // width, height, 1/width, 1/height.
    screen: vec4<f32>,
    // planet radius, furthest anything is drawn, ground tile in metres, sheet size in texels.
    world: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var sheet: texture_2d<f32>;
@group(1) @binding(1) var sheet_sampler: sampler;

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
// This is the whole trick of the piece. `fwidth` says how much `value` changes from one pixel
// to the next, so dividing a pixel count by it converts a width on the screen into a width in
// whatever `value` happens to measure. A contour drawn this way is the same weight everywhere:
// on a cloud overhead, on one at the horizon, and on the same cloud after the window is
// resized. It is the difference between a drawn line and a scaled one.
fn line_width(value: f32, pixels: f32) -> f32 {
    return max(fwidth(value), 1e-7) * pixels * 0.5;
}

// How much of a line is worth drawing at all.
//
// A line is only a line while the thing it follows is bigger than a pixel. Where a value swings
// through several bands from one pixel to the next -- which is what happens to every field in
// this picture as it approaches the horizon -- asking for a line of constant width asks for a
// line wider than the shape it is drawing, and the honest answer is that there is nothing there
// to draw. Without this the bottom of the sky fills in solid.
fn legible(gradient: f32) -> f32 {
    return 1.0 - smoothstep(0.35, 0.9, gradient);
}

// One over the boundary at `level`: 1 on the line, 0 off it, with a pixel of softness.
//
// A single boundary needs no legibility test: a line of constant width in pixels stays one line
// however fast the value under it is moving. It is only where *many* boundaries crowd into one
// pixel, which is what [`contours`] has to deal with, that the question arises.
fn stroke(value: f32, level: f32, pixels: f32) -> f32 {
    let half = line_width(value, pixels);
    return 1.0 - smoothstep(half * 0.55, half, abs(value - level));
}

// Lines wherever `value` crosses one of `bands` evenly spaced levels.
fn contours(value: f32, bands: f32, pixels: f32) -> f32 {
    let stepped = value * bands;
    // Distance to the nearest whole step, which is where a band boundary is.
    let gap = 0.5 - abs(fract(stepped) - 0.5);
    let half = line_width(stepped, pixels);
    let on = 1.0 - smoothstep(half * 0.55, half, gap);
    return on * legible(fwidth(stepped));
}

// `value` flattened into `bands` steps, given back in nought to one.
//
// And unflattened again wherever the steps have grown smaller than a pixel: a staircase sampled
// below its own tread size is not a staircase, it is moire, and the smooth value it was cut
// from is exactly the average the eye wanted anyway.
fn flatten(value: f32, bands: f32) -> f32 {
    let stepped = (floor(clamp(value, 0.0, 0.999) * bands) + 0.5) / bands;
    return mix(stepped, clamp(value, 0.0, 1.0), 1.0 - legible(fwidth(value * bands)));
}

// A colour off the four-stop ramp a cloud is filled from.
fn ramp(at: f32) -> vec3<f32> {
    let place = clamp(at, 0.0, 1.0) * 3.0;
    let low = floor(place);
    let blend = place - low;
    var a = u.cloud_0.rgb;
    var b = u.cloud_1.rgb;
    if (low >= 2.0) {
        a = u.cloud_2.rgb;
        b = u.cloud_3.rgb;
    } else if (low >= 1.0) {
        a = u.cloud_1.rgb;
        b = u.cloud_2.rgb;
    }
    return mix(a, b, blend);
}

// ---------------------------------------------------------------------------------------
// the sheet
// ---------------------------------------------------------------------------------------

// The cloud field of one deck at a point on its plane, in nought to one.
//
// Two reads of the same sheet at different scales, scrolled at different speeds. That second
// clause is what makes these clouds weather rather than wallpaper: the sum of two fields moving
// at different speeds is not a translation of anything, so a cloud grows, leans, splits and
// closes up again as it crosses, and never repeats.
fn deck_field(p: vec2<f32>, deck: vec4<f32>, lod: f32) -> vec2<f32> {
    let q = (p + u.wind.xz) / deck.y;
    let first = textureSampleLevel(sheet, sheet_sampler, q, lod).r;
    let drift = vec2<f32>(u.wind.w, -u.wind.w * 0.6) / deck.y;
    let second = textureSampleLevel(
        sheet,
        sheet_sampler,
        q * 1.37 + drift + vec2<f32>(0.41, 0.19),
        lod + 0.46,
    ).g;
    // Both halves come back: the sum is the cloud, and the first field on its own is what the
    // shading and the shadow walk are read from. Those want the shape of the lobe rather than
    // the fine grain on it, and taking them off one read instead of two is a third of the
    // texture traffic of the whole frame.
    return vec2<f32>(first * 0.62 + second * 0.38, first);
}

// The slow half of the field, for the six reads a deck's shading needs.
fn deck_base(p: vec2<f32>, deck: vec4<f32>, lod: f32) -> f32 {
    return textureSampleLevel(sheet, sheet_sampler, (p + u.wind.xz) / deck.y, lod).r;
}

// Where the weather is: a reading of the same sheet over nine tiles at once, so it varies over
// the whole width of the sky and not over one cloud. It decides which districts are cloudy.
fn weather(p: vec2<f32>, deck: vec4<f32>, lod: f32) -> f32 {
    let q = (p + u.wind.xz) / (deck.y * 9.0);
    return textureSampleLevel(sheet, sheet_sampler, q, max(lod - 3.17, 0.0)).b;
}

// Where a ray leaves a sphere it starts inside. The eye is always under every deck, so there is
// exactly one crossing ahead of it.
fn shell_exit(dir: vec3<f32>, radius: f32) -> f32 {
    let centre = vec3<f32>(0.0, -u.world.x, 0.0);
    let offset = u.origin.xyz - centre;
    let b = dot(offset, dir);
    let c = dot(offset, offset) - radius * radius;
    let disc = b * b - c;
    return -b + sqrt(max(disc, 0.0));
}

// ---------------------------------------------------------------------------------------
// one deck of cloud
// ---------------------------------------------------------------------------------------

// Colour and coverage of one deck along `dir`, with its line already drawn into it.
fn draw_deck(dir: vec3<f32>, deck: vec4<f32>, pixel: f32) -> vec4<f32> {
    let up = max(dir.y, 0.004);
    var reach = min(shell_exit(dir, u.world.x + deck.x), u.world.y);
    var p = u.origin.xz + dir.xz * reach;

    // How much of the plane one pixel covers, and from that which level of the sheet to read.
    // A deck seen edge-on near the horizon is minified enormously, and reading the top level
    // there gives a band of sparkling confetti instead of distant cloud.
    let footprint = reach * pixel / up;
    let lod = max(log2(footprint / deck.y * u.world.w), 0.0);

    // Lift the sample onto the cloud's own top.
    //
    // A deck is a plane, but the cloud standing on it is not, and a ray meets the top of a
    // cloud some way before it meets the plane underneath. Two rounds of asking how high the
    // cloud is here and then re-intersecting at that height is enough to converge on the top
    // for anything but a cliff, and it is the difference between a pattern painted on a ceiling
    // and a heap with a side to it: near the horizon the clouds now stand up and show their
    // flanks, and overhead they bulge towards you and cover the ones behind.
    let first = deck_field(p, deck, lod).x;
    let lift = clamp((first - deck.z) * u.puff.x, 0.0, deck.x * 0.6);
    reach = min(shell_exit(dir, u.world.x + deck.x + lift), u.world.y);
    p = u.origin.xz + dir.xz * reach;

    let both = deck_field(p, deck, lod);
    let f = both.x;
    let base = both.y;
    let local = deck.z * (1.35 - 0.75 * weather(p, deck, lod));
    let thickness = f - local;

    // The slope of the field, in field units per metre, taken from two more reads a small step
    // away. This is what stands in for a volume: a two-dimensional field read as the height of
    // a cloud top has a normal, and a normal is all the light needs.
    // Read over a long step on purpose. The field has octaves down to a few hundred metres,
    // and a slope measured across sixty of them is the slope of the smallest thing in the
    // field: the light then follows the grain instead of the shape, and the contours come back
    // as lace. This measures the lobe.
    let step = max(deck.y * 0.022, footprint);
    let along = (deck_base(p + vec2<f32>(step, 0.0), deck, lod) - base) / step;
    let across = (deck_base(p + vec2<f32>(0.0, step), deck, lod) - base) / step;
    let normal = normalize(vec3<f32>(-along * u.puff.x, 1.0, -across * u.puff.x));
    let lambert = max(dot(normal, u.sun.xyz), 0.0);

    // Cast shadow, the same way: walk a few steps towards the sun along the ground and ask
    // whether the cloud there stands higher than the sun ray does by the time it gets there.
    // Three taps is not many, but it is enough to put one cloud's shadow on the shoulder of the
    // next, which is the thing that makes a flat field look like a heap.
    let flat_sun = normalize(vec2<f32>(u.sun.x, u.sun.z) + vec2<f32>(1e-5, 0.0));
    let climb = max(u.sun.y, 0.06) / u.puff.x;
    let reach_shadow = deck.y * u.puff.y;
    var blocked = 0.0;
    for (var i = 1; i <= 3; i = i + 1) {
        let s = f32(i) / 3.0 * reach_shadow;
        let there = deck_base(p + flat_sun * s, deck, lod);
        blocked = max(blocked, there - base - s * climb);
    }
    let shadow = clamp(blocked * u.puff.z, 0.0, 1.0);

    // Tone, then flattened into bands. The flattening is the drawing: between one boundary and
    // the next there is one colour and nothing else.
    let crown = smoothstep(0.0, u.puff.w, thickness);
    // Height carries more of the tone than slope does. Slope alone puts a band boundary on
    // every ripple in the field; height alone is flat. Between them the cloud reads as a heap
    // with a lit side, in five or six areas of colour.
    let tone = clamp(
        u.shade.y + u.shade.z * lambert * (1.0 - shadow * 0.55) + u.shade.w * crown,
        0.0,
        1.0,
    );
    let stepped = flatten(tone, u.shade.x);
    var colour = ramp(stepped);

    // The line: round the outside of the cloud, and along every tone boundary inside it.
    let inside = contours(tone, u.shade.x, u.line.x);
    let outline = stroke(thickness, 0.0, u.line.x * 2.4);
    // The outline is heavier than the contours inside it, which is how a pen does it: the
    // silhouette is the drawing and the rest is modelling.
    colour = mix(colour, u.ink.rgb, clamp(inside * 0.78 + outline, 0.0, 1.0));

    // Distance takes the colour towards the haze, and the deck's own place in the sky takes it
    // further: three decks of the same white read as one deck, and the same three hazed apart
    // read as a sky with a top and a bottom to it.
    let far = 1.0 - exp(-reach * 0.000021);
    colour = mix(colour, u.haze.rgb, clamp(deck.w + far * 0.42, 0.0, 0.88));

    // Coverage, softened by exactly one line width so the edge is neither jagged nor woolly.
    let edge = line_width(thickness, 1.5);
    var alpha = smoothstep(-edge, edge, thickness);
    alpha = alpha * smoothstep(0.0, 0.012, dir.y);
    return vec4<f32>(colour, alpha);
}

// ---------------------------------------------------------------------------------------
// the world under the clouds
// ---------------------------------------------------------------------------------------

// The sky: flat bands from the horizon to the zenith, with a line between each pair of them,
// and the sun sitting on top as a disc with a ring around it.
fn draw_sky(dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(dir.y, 0.0, 1.0);
    // Bunched towards the horizon, which is both what the air does and what stops the bands
    // reading as stripes.
    let height = pow(up, 0.30);
    let stepped = flatten(height, u.line.y);
    var colour = mix(u.sky_horizon.rgb, u.sky_zenith.rgb, stepped);
    colour = mix(colour, u.ink.rgb, contours(height, u.line.y, u.line.x * 0.8) * 0.55);

    let towards = dot(dir, u.sun.xyz);
    let disc = smoothstep(u.sun.w, u.sun.w + 0.00012, towards);
    colour = mix(colour, u.sun_colour.rgb, disc);
    colour = mix(colour, u.ink.rgb, stroke(towards, u.sun.w, u.line.x * 1.4));
    return colour;
}

// Rock standing on the horizon, as a height in the same units as `dir.y`.
//
// Read around a circle rather than off an angle, so there is no seam behind you where the angle
// wraps: the ring of samples closes on itself because the sheet tiles.
fn mesa_profile(dir: vec3<f32>) -> f32 {
    let flat = normalize(vec2<f32>(dir.x, dir.z) + vec2<f32>(1e-5, 0.0));
    let ring = flat * 0.37 + vec2<f32>(0.5, 0.5);
    let low = textureSampleLevel(sheet, sheet_sampler, ring, 2.0).b;
    let high = textureSampleLevel(sheet, sheet_sampler, ring * 2.3 + 0.13, 1.0).a;
    return (low * 0.7 + high * 0.3 - 0.42) * u.line.w;
}

// The desert: flat bands running away to the horizon, with a line along each one, and the sand
// itself drifting.
fn draw_ground(dir: vec3<f32>, pixel: f32) -> vec3<f32> {
    let down = min(dir.y, -0.0004);
    let reach = min(-u.origin.y / down, u.world.y);
    let p = u.origin.xz + dir.xz * reach;
    let footprint = reach * pixel / (-down);
    let lod = max(log2(footprint / u.world.z * u.world.w), 0.0);

    let dune = textureSampleLevel(sheet, sheet_sampler, p / u.world.z, lod).a;
    let fine = textureSampleLevel(sheet, sheet_sampler, p / (u.world.z * 0.31) + 0.27, lod).r;
    // Near ground reads darker and the far ground pale, with the dunes swinging the boundary
    // about so the bands are long shallow curves rather than ruled lines.
    let away = 1.0 - exp(-reach * 0.00022);
    let value = clamp(away * 0.75 + dune * 0.4 + fine * 0.12 - 0.16, 0.0, 1.0);
    let stepped = flatten(value, u.line.z);
    var colour = mix(u.ground_near.rgb, u.ground_far.rgb, stepped);
    colour = mix(colour, u.ink.rgb, contours(value, u.line.z, u.line.x));
    return mix(colour, u.haze.rgb, smoothstep(0.55, 1.0, away));
}

// ---------------------------------------------------------------------------------------
// the frame
// ---------------------------------------------------------------------------------------

// `over`, the painter's operator: what is in front, over what is behind.
fn over(under: vec3<f32>, top: vec4<f32>) -> vec3<f32> {
    return mix(under, top.rgb, top.a);
}

@fragment
fn fs_main(in: Varying) -> @location(0) vec4<f32> {
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let dir = normalize(
        u.forward.xyz
            + u.right.xyz * ndc.x * u.origin.w * u.forward.w
            + u.up.xyz * ndc.y * u.origin.w
    );
    // How much of the world one pixel covers, as an angle. Every footprint in the shader is
    // this multiplied by a distance.
    let pixel = 2.0 * u.origin.w / u.up.w;

    // The sky, then the rock standing on it, then the sand: all three worked out for every
    // pixel and then chosen between, so that every `fwidth` in here means what it says.
    var colour = draw_sky(dir);

    let profile = mesa_profile(dir);
    let standing = dir.y - profile;
    let is_mesa = 1.0 - smoothstep(-line_width(standing, 1.2), line_width(standing, 1.2), standing);
    var rock = mix(u.mesa.rgb, u.ink.rgb, stroke(standing, 0.0, u.line.x * 1.5));
    colour = mix(colour, rock, is_mesa * step(0.0, dir.y));

    let ground = draw_ground(dir, pixel);
    let below = 1.0 - smoothstep(-0.0009, 0.0009, dir.y);
    colour = mix(colour, ground, below);
    // The horizon itself, drawn rather than left to be an accident of where two fills meet.
    colour = mix(colour, u.ink.rgb, stroke(dir.y, 0.0, u.line.x * 1.6));

    // Far deck first. A nearer deck is a lower one: at any angle above the horizon, the lower
    // plane is the closer of the two.
    colour = over(colour, draw_deck(dir, u.deck_c, pixel));
    colour = over(colour, draw_deck(dir, u.deck_b, pixel));
    colour = over(colour, draw_deck(dir, u.deck_a, pixel));

    return vec4<f32>(colour, 1.0);
}
