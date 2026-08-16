// Volumetric clouds over a desert, in two passes.
//
// Pass one (`fs_clouds`) raymarches a shell of cloud around a planet and lights every sample by
// marching a second, shorter ray at the sun. It runs at whatever fraction of the window the
// viewer has asked for, and writes high dynamic range: light, not colour.
//
// Pass two (`fs_finish`) runs at the full size of the window, tone-maps that, and optionally
// steps the result into flat bands with a line drawn where the bands meet, which is the other
// piece in this repo done to a volume instead of a bitmap.
//
// Everything is sampled with `textureSampleLevel` rather than `textureSample`. The march breaks
// out of its loop as soon as it has run out of light to gather, which makes the control flow
// non-uniform, and a sampling call that wants implicit derivatives is not allowed there.

struct Uniforms {
    // xyz: where the eye is, in metres. w: tan of half the vertical field of view.
    origin: vec4<f32>,
    // xyz: where it looks. w: aspect ratio.
    forward: vec4<f32>,
    // xyz: its right. w: internal width in pixels.
    right: vec4<f32>,
    // xyz: its up. w: internal height in pixels.
    up: vec4<f32>,
    // xyz: direction towards the sun. w: cosine of the sun's angular radius.
    sun: vec4<f32>,
    // xyz: how far the weather has blown, in metres. w: how much faster the fine detail blows.
    wind: vec4<f32>,
    // coverage, extinction per metre, how hard the detail erodes, where the anvil flattens.
    shape: vec4<f32>,
    // most steps, sun steps, furthest the march goes, how long one step is.
    march: vec4<f32>,
    // cloud base, cloud top, planet radius, how many metres one turn of the shape volume covers.
    layer: vec4<f32>,
    // detail scale, forward scattering, powder, how hard the sun march absorbs.
    tune: vec4<f32>,
    // ambient strength, silver lining, ground haze per metre, exposure.
    mix0: vec4<f32>,
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    // rgb: sunlight. w: its strength.
    sun_colour: vec4<f32>,
    // rgb: the light the sky throws back into the clouds.
    ambient: vec4<f32>,
    ground_near: vec4<f32>,
    ground_far: vec4<f32>,
    // rgb: what distance fades into.
    haze: vec4<f32>,
    // width, height, 1/width, 1/height of the window.
    screen: vec4<f32>,
    // bands (zero for none), ink strength, ink threshold, spare.
    finish: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

@group(1) @binding(0) var shape_volume: texture_3d<f32>;
@group(1) @binding(1) var detail_volume: texture_3d<f32>;
@group(1) @binding(2) var volume_sampler: sampler;

struct Varying {
    @builtin(position) place: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// One triangle covering the screen, built from the vertex index. No vertex buffer, no quad, no
// seam down the diagonal.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Varying {
    var out: Varying;
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    out.uv = uv;
    out.place = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return out;
}

// ---------------------------------------------------------------------------------------
// the cloud volume
// ---------------------------------------------------------------------------------------

// How much further one stride of the empty-air march is than one step of the fine one.
const COARSE: f32 = 2.5;

// How much the shape volume is squeezed vertically. Below one, so a cloud is taller than it is
// wide for the same lump of noise.
const VERTICAL: f32 = 0.75;

fn remap(value: f32, low: f32, high: f32, to_low: f32, to_high: f32) -> f32 {
    return to_low + (value - low) / (high - low) * (to_high - to_low);
}

fn planet_centre() -> vec3<f32> {
    return vec3<f32>(0.0, -u.layer.z, 0.0);
}

// How far up the cloud layer a point sits, nought at the base and one at the top.
fn height_fraction(p: vec3<f32>) -> f32 {
    let altitude = length(p - planet_centre()) - u.layer.z;
    return clamp((altitude - u.layer.x) / (u.layer.y - u.layer.x), 0.0, 1.0);
}

// The vertical profile of a heap cloud: pinched at the base where the air is still climbing,
// widest through the middle, and spread flat where it hits the ceiling it cannot pass.
fn profile(h: f32, towering: f32) -> f32 {
    let base = smoothstep(0.0, 0.09, h);
    // How far up this part of the sky is allowed to build before it spreads out. A low ceiling
    // gives a flat sheet of fair-weather cloud; a high one gives a tower with an anvil on it.
    let ceiling = u.shape.w * towering;
    // A long slope rather than a lid. A ceiling that cuts hard leaves every cloud with the same
    // flat top at the same altitude, which is a layer of slabs; sloped, the density simply runs
    // out somewhere up there and the noise decides where, so the tops come out different
    // heights and different shapes.
    let cap = 1.0 - smoothstep(ceiling * 0.9, ceiling * 1.6, h);
    // Nearly flat through the body of the cloud. A density that climbs steeply with height
    // squeezes everything the coverage test can find into a thin band, and a thin band of cloud
    // seen from underneath is a field of pancakes: the cloud has to be allowed to be as tall as
    // it is wide before it can look like anything else.
    let weight = mix(0.72, 1.0, smoothstep(0.0, 0.3, h / max(ceiling, 0.05)));
    return base * cap * weight;
}

// Extinction per metre at a point: nought in clear air, up to `shape.y` inside a cloud.
//
// `detail` says whether to pay for the second volume. The sun march and the far half of the
// view march do not: the erosion is a metre-scale effect, and at those distances it is being
// asked to carve features smaller than the step that samples them.
fn density_at(p: vec3<f32>, h: f32, detail: bool) -> f32 {
    if (h <= 0.0 || h >= 1.0) {
        return 0.0;
    }
    // The weather map: one sample of the same volume taken flat and very wide, so it varies
    // over kilometres and does not vary with height at all. Without it every cloud in the sky
    // is the same cloud, because one field at one scale has one character. With it the sky has
    // districts: places where the coverage runs high and the tops tower, and places where it
    // stays thin, and the wind carries those districts past as well.
    let map = textureSampleLevel(
        shape_volume,
        volume_sampler,
        vec3<f32>((p.xz + u.wind.xz) / (u.layer.w * 5.0), 0.19),
        0.0,
    );
    let local = u.shape.x * (0.45 + 1.35 * map.r);
    let towering = 0.45 + 0.95 * map.g;

    // The volume is isotropic and the layer it is being read through is four kilometres deep
    // and tens of kilometres wide, so read straight it gives pancakes: features as wide as they
    // are tall, then flattened by the profile. Squeezing the vertical coordinate stretches
    // every feature upwards and gives the field something to build towers out of.
    var q = (p + u.wind.xyz) / u.layer.w;
    q.y = q.y * VERTICAL;
    let base = textureSampleLevel(shape_volume, volume_sampler, q, 0.0);
    let stacked = base.g * 0.625 + base.b * 0.25 + base.a * 0.125;
    var d = remap(base.r, stacked - 1.0, 1.0, 0.0, 1.0) * profile(h, towering);
    d = remap(d, 1.0 - local, 1.0, 0.0, 1.0);
    if (d <= 0.0) {
        return 0.0;
    }
    if (detail) {
        let fine = textureSampleLevel(
            detail_volume,
            volume_sampler,
            (p + u.wind.xyz * u.wind.w) / u.tune.x,
            0.0,
        );
        let stack = fine.r * 0.625 + fine.g * 0.25 + fine.b * 0.125;
        // Wispy where it is thin at the bottom, curdled where it is billowing at the top: the
        // erosion is inverted through the layer, which is what stops every cloud in the sky
        // being eaten by the same pattern.
        let bite = mix(1.0 - stack, stack, clamp(h * 2.5, 0.0, 1.0)) * u.shape.z;
        d = remap(d, bite, 1.0, 0.0, 1.0);
    }
    return clamp(d, 0.0, 1.0) * u.shape.y;
}

// Where a ray leaves a sphere it starts inside. The camera is always under the cloud layer, so
// both shells have exactly one crossing ahead of it and this is all the geometry needed.
fn shell_exit(origin: vec3<f32>, dir: vec3<f32>, radius: f32) -> f32 {
    let offset = origin - planet_centre();
    let b = dot(offset, dir);
    let c = dot(offset, offset) - radius * radius;
    let disc = b * b - c;
    if (disc < 0.0) {
        return -1.0;
    }
    return -b + sqrt(disc);
}

// Henyey-Greenstein: how much light a cloud droplet throws at the angle it was already going.
//
// Written without the `1 / 4pi` that normalises it over the sphere, which is deliberate. The
// normalised form averages to one *as a probability density*, and multiplying a sunlight
// radiance by it leaves every cloud not directly in front of the sun lit at about a fiftieth of
// what it should be: a sky of flat grey lumps. Dropping the constant makes this a gain about
// the average instead, which is what a single-scattering approximation actually wants.
fn henyey(cos_angle: f32, g: f32) -> f32 {
    let gg = g * g;
    return (1.0 - gg) / pow(1.0 + gg - 2.0 * g * cos_angle, 1.5);
}

// The phase function actually used: mostly forward, with a little thrown back, so a cloud is
// bright when it is between you and the sun and still readable when it is not.
fn phase(cos_angle: f32) -> f32 {
    return mix(henyey(cos_angle, u.tune.y), henyey(cos_angle, -0.15), 0.28);
}

// How much cloud stands between a point and the sun, as optical depth.
//
// Six steps, each half again as long as the last. Cheap where it matters (the first few
// hundred metres decide most of the shading) and long enough to reach the top of the layer.
fn sun_depth(p: vec3<f32>) -> f32 {
    let steps = i32(u.march.y);
    var walked = 0.0;
    // Short to begin with and growing fast: what shades a sample is mostly the cloud in the
    // first few hundred metres above it, and a first step of a third of a kilometre measures
    // that stretch with a single sample taken past the end of it.
    var step = (u.layer.y - u.layer.x) * 0.025;
    var depth = 0.0;
    for (var i = 0; i < steps; i = i + 1) {
        let sample = p + u.sun.xyz * (walked + step * 0.5);
        depth = depth + density_at(sample, height_fraction(sample), false) * step;
        walked = walked + step;
        step = step * 1.7;
    }
    return depth * u.tune.w;
}

// Sunlight reaching a sample, in three scattering octaves.
//
// One octave of Beer's law is a cloud whose inside is black, because single scattering is not
// how light gets into a cloud. Each further octave absorbs less and scatters wider, which is a
// cheap stand-in for light that has bounced its way in, and it is the difference between a
// storm cloud and a lump of coal.
fn sun_light(depth: f32, cos_angle: f32) -> f32 {
    var gathered = 0.0;
    var absorb = 1.0;
    var weight = 1.0;
    var spread = 1.0;
    var total = 0.0;
    for (var octave = 0; octave < 3; octave = octave + 1) {
        gathered = gathered + weight * exp(-depth * absorb) * phase(cos_angle * spread);
        total = total + weight;
        absorb = absorb * 0.5;
        weight = weight * 0.5;
        spread = spread * 0.5;
    }
    // Divided by the weights, so that a sample with nothing between it and the sun is lit by
    // one sun rather than by one and three quarters. Without this every thin cloud in the sky
    // comes back overexposed and the whole field goes white.
    return gathered / total;
}

// ---------------------------------------------------------------------------------------
// the world under the clouds
// ---------------------------------------------------------------------------------------

fn sky_colour(dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(dir.y, 0.0, 1.0);
    // The zenith wins quickly. Two saturated colours mixed half and half make grey, so the
    // band where the sky is half horizon and half zenith has to be kept narrow, and the warmth
    // that belongs at the horizon is added as a glow rather than mixed in as a colour.
    var colour = mix(u.sky_horizon.rgb, u.sky_zenith.rgb, pow(up, 0.18));
    let towards = max(dot(dir, u.sun.xyz), 0.0);
    // A wide warm glow sitting on the horizon under the sun, and a tight one right around it.
    colour = colour + u.sun_colour.rgb * pow(towards, 3.0) * 0.20 * exp(-up * 5.0);
    colour = colour + u.sun_colour.rgb * pow(towards, 40.0) * 0.6;
    return colour;
}

fn sun_disc(dir: vec3<f32>) -> vec3<f32> {
    let towards = dot(dir, u.sun.xyz);
    let edge = smoothstep(u.sun.w, u.sun.w + 0.00018, towards);
    return u.sun_colour.rgb * u.sun_colour.w * edge;
}

// How much of the sun a point on the ground can see. Four samples through the layer, which is
// enough for a shadow whose edge is soft anyway.
fn ground_shadow(p: vec3<f32>) -> f32 {
    let low = shell_exit(p, u.sun.xyz, u.layer.z + u.layer.x);
    let high = shell_exit(p, u.sun.xyz, u.layer.z + u.layer.y);
    if (high <= low) {
        return 1.0;
    }
    // Only the first stretch of the layer, and never more than a few kilometres of it. Under a
    // low sun the ray from a patch of sand to the sun is inside the cloud layer for twenty-odd
    // kilometres, and six samples spread over that measures nothing but its own step size: the
    // shadows come back as hard polygons with straight edges.
    let reach = min(high - low, 5000.0);
    let step = reach / 6.0;
    var depth = 0.0;
    for (var i = 0; i < 6; i = i + 1) {
        let sample = p + u.sun.xyz * (low + (f32(i) + 0.5) * step);
        depth = depth + density_at(sample, height_fraction(sample), false) * step;
    }
    return exp(-depth * 0.8);
}

fn desert(p: vec3<f32>, distance: f32) -> vec3<f32> {
    // The sand's own colour comes out of the same volume the clouds do, read as two flat slices
    // at very different scales: one for the long swells, one for the drift on top of them.
    let broad = textureSampleLevel(
        shape_volume,
        volume_sampler,
        vec3<f32>(p.xz * 0.00004, 0.37),
        0.0,
    ).r;
    let fine = textureSampleLevel(
        shape_volume,
        volume_sampler,
        vec3<f32>(p.xz * 0.00021, 0.71),
        0.0,
    ).g;
    let tone = clamp(broad * 0.75 + fine * 0.45 - 0.1, 0.0, 1.0);
    let albedo = mix(u.ground_near.rgb, u.ground_far.rgb, tone);
    // Flat sand, so its normal is straight up and the whole of its lighting is how high the sun
    // is. That is why the desert goes dark under a low sun while the clouds over it stay lit:
    // they are catching the same light side-on.
    let lambert = max(u.sun.y, 0.0) * ground_shadow(p);
    let light = u.sun_colour.rgb * lambert + u.ambient.rgb * u.mix0.x * 0.6;
    let fog = 1.0 - exp(-distance * u.mix0.z);
    return mix(albedo * light, u.haze.rgb, fog);
}

// ---------------------------------------------------------------------------------------
// pass one: the march
// ---------------------------------------------------------------------------------------

// An ordered dither, so the step pattern shows up as a fixed weave rather than as bands across
// the sky. Fixed rather than animated: noise that changes every frame needs somewhere to
// accumulate, and there is no history buffer here.
fn dither(place: vec2<f32>) -> f32 {
    // Interleaved gradient noise. A four-by-four ordered table was the first thing here and it
    // wrote its own weave across every cloud: sixteen offsets repeating on a grid is a pattern,
    // and a pattern in the sampling is a pattern in the picture. This one has no period the eye
    // can find and still spreads evenly over the step.
    return fract(52.9829189 * fract(0.06711056 * place.x + 0.00583715 * place.y));
}

@fragment
fn fs_clouds(in: Varying) -> @location(0) vec4<f32> {
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let dir = normalize(
        u.forward.xyz
            + u.right.xyz * ndc.x * u.origin.w * u.forward.w
            + u.up.xyz * ndc.y * u.origin.w
    );
    let origin = u.origin.xyz;

    // What is behind the clouds: sand below the horizon, sky above it.
    var background = sky_colour(dir) + sun_disc(dir);
    if (dir.y < -0.0006) {
        let distance = -origin.y / dir.y;
        background = desert(origin + dir * distance, distance);
    }

    // Clouds only exist above the horizon: the eye is under the layer, so a ray that is going
    // down has hit sand before it could reach any.
    if (dir.y <= 0.001) {
        return vec4<f32>(background, 0.0);
    }

    let enter = shell_exit(origin, dir, u.layer.z + u.layer.x);
    let leave = min(shell_exit(origin, dir, u.layer.z + u.layer.y), u.march.z);
    if (leave <= enter) {
        return vec4<f32>(background, 0.0);
    }

    let span = leave - enter;
    let steps = i32(u.march.x);
    // A step is never shorter than it needs to be and never longer than it can afford to be.
    // Near the horizon a ray is inside the layer for tens of kilometres, and dividing that by
    // the step budget gives half-kilometre steps that walk straight through whole clouds; the
    // ceiling here trades reaching the far end for not doing that.
    let dt = clamp(span / f32(steps), u.march.w, u.march.w * 2.5);
    let cos_angle = dot(dir, u.sun.xyz);
    let jitter = dither(in.place.xy);

    var transmittance = 1.0;
    var scattered = vec3<f32>(0.0);
    var walked = enter + dt * jitter * 0.7;

    // Two speeds. Most of a sky is empty, and a march that samples empty air at the resolution
    // it samples cloud spends nine tenths of its budget establishing that there is nothing
    // there. So it strides until it touches something, backs up one stride, and creeps; when it
    // has been in clear air for a few fine steps it goes back to striding. Same picture, about
    // a third of the samples, which is the difference between this running and not running on
    // the machine it was written on.
    let stride = dt * COARSE;
    var creeping = false;
    var since_hit = 0;

    for (var i = 0; i < steps; i = i + 1) {
        if (walked >= leave) {
            break;
        }
        let p = origin + dir * walked;
        let h = height_fraction(p);
        // The erosion is only worth its samples while its features are bigger than the step
        // taking them, which rules it out for the striding phase and for the far half of the
        // march either way.
        let fine = creeping && walked < u.march.z * 0.3;
        let d = density_at(p, h, fine);
        if (d > 0.0) {
            if (!creeping) {
                creeping = true;
                since_hit = 0;
                walked = max(enter, walked - stride);
                continue;
            }
            let light = sun_light(sun_depth(p), cos_angle);
            // Powder: the dark rind on the sunlit side of a cloud, where light has gone in but
            // has not yet had room to scatter back out.
            let powder = 1.0 - exp(-d * dt * 6.0);
            let lit = u.sun_colour.rgb * light * mix(1.0, powder, u.tune.z);
            let sky_fill = u.ambient.rgb * u.mix0.x * mix(0.35, 1.0, h);
            let source = lit + sky_fill;
            let through = exp(-d * dt);
            scattered = scattered + transmittance * source * (1.0 - through);
            transmittance = transmittance * through;
            if (transmittance < 0.015) {
                break;
            }
            since_hit = 0;
            walked = walked + dt;
        } else {
            if (creeping) {
                since_hit = since_hit + 1;
                if (since_hit > 6) {
                    creeping = false;
                }
            }
            walked = walked + select(stride, dt, creeping);
        }
    }

    // Distance eats contrast: clouds a long way off are being looked at through the same air
    // the horizon is. Their light goes to the colour of that air and their silhouette opens up,
    // so a cloud thirty kilometres away dissolves into the horizon instead of standing there as
    // a hard little shape with everything the near ones have.
    let fade = 1.0 - exp(-enter * u.mix0.z * 0.9);
    scattered = mix(scattered, u.haze.rgb * (1.0 - transmittance), fade);
    transmittance = mix(transmittance, 1.0, fade * 0.6);

    let colour = background * transmittance + scattered;
    return vec4<f32>(colour, 1.0 - transmittance);
}

// ---------------------------------------------------------------------------------------
// pass two: the finish
// ---------------------------------------------------------------------------------------

@group(1) @binding(0) var scene: texture_2d<f32>;
@group(1) @binding(1) var scene_sampler: sampler;

fn tonemap(light: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(1.0) - exp(-light * u.mix0.w);
}

fn luma(colour: vec3<f32>) -> f32 {
    return dot(colour, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// The tone of a pixel after banding, as a step number rather than a level, so that neighbouring
// pixels can be compared for whether a line belongs between them.
fn band_of(uv: vec2<f32>) -> f32 {
    let sample = textureSampleLevel(scene, scene_sampler, uv, 0.0);
    let tone = luma(tonemap(sample.rgb));
    // The cloud's own edge counts as one step too, so a pale cloud against a pale sky is still
    // drawn round. One step, not three: a wisp whose opacity wanders across several thresholds
    // gets a line drawn at every one of them, and the cloud comes back as lace.
    return floor(tone * u.finish.x) + step(0.5, sample.a) * 64.0;
}

@fragment
fn fs_finish(in: Varying) -> @location(0) vec4<f32> {
    let sample = textureSampleLevel(scene, scene_sampler, in.uv, 0.0);
    var colour = tonemap(sample.rgb);

    if (u.finish.x < 0.5) {
        return vec4<f32>(colour, 1.0);
    }

    // Flat colour: the tone is stepped and the hue is carried along unchanged, so a band is one
    // colour over its whole area and the edge between two of them is hard.
    let tone = luma(colour);
    if (tone > 0.0001) {
        let stepped = (floor(tone * u.finish.x) + 0.5) / u.finish.x;
        colour = colour * (stepped / tone);
    }
    // And pushed away from grey. A volumetric cloud is very nearly neutral, which is correct
    // and which flattens to putty the moment the tone is stepped; the flat-colour half of this
    // piece is about colour, so it is allowed to insist on some.
    colour = mix(vec3<f32>(luma(colour)), colour, 1.45);

    // The line. Four taps around the pixel, and ink wherever the band underfoot is not the band
    // next door: a contour of the volume, drawn at the size of the window rather than the size
    // of the march.
    let step = u.screen.zw;
    let here = band_of(in.uv);
    var edge = 0.0;
    edge = edge + abs(band_of(in.uv + vec2<f32>(step.x, 0.0)) - here);
    edge = edge + abs(band_of(in.uv - vec2<f32>(step.x, 0.0)) - here);
    edge = edge + abs(band_of(in.uv + vec2<f32>(0.0, step.y)) - here);
    edge = edge + abs(band_of(in.uv - vec2<f32>(0.0, step.y)) - here);
    let ink = clamp(edge, 0.0, 1.0) * u.finish.y;
    colour = colour * (1.0 - ink);

    return vec4<f32>(colour, 1.0);
}
