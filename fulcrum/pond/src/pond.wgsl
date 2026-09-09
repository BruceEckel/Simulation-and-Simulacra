// A pond, lit and looked through.
//
// There is no simulation in here. The water has moved on the CPU and its height is in
// a texture, one float a cell. All this does is decide, for one pixel, which way the surface
// tilts there, and from the tilt two things: where on the bed of the pond the eye is looking
// through the water, and whether the surface is catching the light.
//
// The colour comes from the bed, which is a slow gradient of the palette with a little
// movement in it, shifted up the palette where the surface is high and down where it is low.
// The sparkle is a highlight on the tilt. The lantern is a glow on the bed, which is why a
// ring crossing it makes it wobble: the ring bends the line of sight to it.

struct Uniforms {
    // cells across, cells down, pixels to a cell, cells between texture rows.
    field: vec4<f32>,
    // window width, window height, seconds since the start, where the colours have drifted to.
    screen: vec4<f32>,
    // the lantern: x and y in pixels, how bright, and how wide its glow is in pixels.
    lantern: vec4<f32>,
    // which way the light is, and how bright the highlight it makes.
    light: vec4<f32>,
    // slope gain, refraction in pixels, height tint, vignette.
    tune: vec4<f32>,
    // The palette, deepest to brightest.
    stops: array<vec4<f32>, 5>,
    // The colour of the lantern's light.
    glow: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var field: texture_2d<f32>;

struct Varying {
    @builtin(position) place: vec4<f32>,
}

// One triangle covering the frame. Nothing is transformed, so the fragment's built-in position
// is the physical pixel, which is what the lookup into the water takes.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Varying {
    var out: Varying;
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    out.place = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return out;
}

// The height of one cell. Off the field is level.
fn tap(cell: vec2<i32>) -> f32 {
    let last = vec2<i32>(i32(u.field.x) - 1, i32(u.field.y) - 1);
    return textureLoad(field, clamp(cell, vec2<i32>(0, 0), last), 0).r;
}

// The height at a point, in cell units, blended from the four cells around it. Cell centres
// sit at half-integers.
fn height(p: vec2<f32>) -> f32 {
    let q = p - vec2<f32>(0.5, 0.5);
    let base = floor(q);
    let f = q - base;
    let c = vec2<i32>(base);
    let a = tap(c);
    let b = tap(c + vec2<i32>(1, 0));
    let d = tap(c + vec2<i32>(0, 1));
    let e = tap(c + vec2<i32>(1, 1));
    return mix(mix(a, b, f.x), mix(d, e, f.x), f.y);
}

// A colour from the palette at `t` in `0..1`, eased between neighbouring stops.
fn palette(t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0) * 4.0;
    let i = i32(floor(min(x, 3.999)));
    let f = smoothstep(0.0, 1.0, x - f32(i));
    return mix(u.stops[i].rgb, u.stops[i + 1].rgb, f);
}

// Fold any number into `0..1` and back, so a position on the palette can drift forever and
// not meet an edge: past the brightest stop it turns round and heads back down.
fn fold(t: f32) -> f32 {
    return 1.0 - abs(1.0 - fract(t * 0.5) * 2.0);
}

@fragment
fn fs_main(in: Varying) -> @location(0) vec4<f32> {
    let pixel = in.place.xy;
    let cell = max(u.field.z, 1.0);
    let p = pixel / cell;

    // The surface here, and which way it tilts: the difference between the heights a cell
    // either side, in each direction.
    let h = height(p);
    let dx = height(p + vec2<f32>(1.0, 0.0)) - height(p - vec2<f32>(1.0, 0.0));
    let dy = height(p + vec2<f32>(0.0, 1.0)) - height(p - vec2<f32>(0.0, 1.0));
    let n = normalize(vec3<f32>(-dx * u.tune.x, -dy * u.tune.x, 1.0));

    // Where on the bed the eye is looking, through a surface tilted like that.
    let bed = pixel + n.xy * u.tune.y;
    let uv = bed / u.screen.xy;

    // The bed: a gradient up the window, tilted a little across it, with a slow movement in
    // it so that flat water is not a still picture.
    let clock = u.screen.z * 0.05;
    let mottle = sin(uv.x * 5.3 + clock) * sin(uv.y * 4.1 - clock * 0.7)
        + 0.6 * sin((uv.x + uv.y) * 3.3 + clock * 0.4);
    var t = 0.10 + 0.46 * uv.y + 0.14 * uv.x + 0.08 * mottle + u.screen.w;
    t = fold(t);
    // A crest is coloured from higher up the palette and a trough from lower down. This is
    // what gives a ring its bands.
    t = t + h * u.tune.z;
    var colour = palette(t);

    // A steeply tilted surface shows a little less of what is under it.
    colour = colour * (1.0 - 0.3 * (1.0 - n.z));

    // The highlight: where the tilt turns the light back toward the eye.
    let halfway = normalize(normalize(u.light.xyz) + vec3<f32>(0.0, 0.0, 1.0));
    let spec = pow(max(dot(n, halfway), 0.0), 40.0) * u.light.w;
    let bright = u.stops[4].rgb;
    colour = colour + bright * spec;

    // The lantern, seen through the water: a soft glow on the bed, falling off with a long
    // tail, and lighting the highlights near it a little more.
    let d = (bed - u.lantern.xy) / max(u.lantern.w, 1.0);
    let near = 1.0 + dot(d, d);
    let glow = u.lantern.z / (near * sqrt(near));
    colour = colour + u.glow.rgb * glow;
    colour = colour + bright * spec * glow * 1.5;

    // Darker at the corners, so the middle is where the eye settles.
    let r = length(pixel / u.screen.xy - vec2<f32>(0.5, 0.5)) * 1.4142;
    colour = colour * (1.0 - u.tune.w * smoothstep(0.45, 1.0, r));

    // Everything above was done between the palette's numbers as written, which are display
    // values. The frame is sRGB, so the hardware is about to apply the display curve; this
    // takes it off first, once, on the one colour that came out.
    return vec4<f32>(to_light(clamp(colour, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}

// A display value to the light it stands for: the sRGB transfer curve, inverted.
fn to_light(colour: vec3<f32>) -> vec3<f32> {
    let low = colour / 12.92;
    let high = pow((colour + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, colour <= vec3<f32>(0.04045));
}
