// Conway's Game of Life and its relatives, coloured.
//
// There is no simulation in here. The rule has already run on the CPU and the answer is in a
// texture: two bytes a cell, `r` saying what the cell is and `g` saying how long it has been
// that. All this does is decide which pixel is which cell, and what colour that cell is.
//
// It reads with `textureLoad` at integer coordinates rather than sampling, on purpose. There is
// no sampler bound at all. A cell is one flat colour with a hard edge, at sixty-four pixels a
// cell and at one, because a cellular automaton with soft edges is a picture of something else.

struct Uniforms {
    // cells across, cells down, pixels to a cell, cells between texture rows.
    field: vec4<f32>,
    // window width, window height, whether cell edges are drawn, how dark they are.
    screen: vec4<f32>,
    // whether age is read, whether trails are read, the smallest cell that gets an edge, spare.
    reading: vec4<f32>,
    // An empty cell.
    back: vec4<f32>,
    // A live cell that has held for a while.
    live: vec4<f32>,
    // A live cell born this generation.
    fresh: vec4<f32>,
    // A cell part-way through a Generations rule's dying states.
    dying: vec4<f32>,
    // Where a cell recently was.
    trail: vec4<f32>,
    // The line between cells.
    ink: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var field: texture_2d<f32>;

struct Varying {
    @builtin(position) place: vec4<f32>,
}

// One triangle covering the frame. Nothing is transformed, so the fragment's built-in position
// is the physical pixel, which is exactly what the cell lookup wants.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Varying {
    var out: Varying;
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    out.place = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: Varying) -> @location(0) vec4<f32> {
    let cell = max(u.field.z, 1.0);
    let pixel = in.place.xy;
    let column = floor(pixel.x / cell);
    let row = floor(pixel.y / cell);

    // The field is sized to cover the window, but a window whose width is not a whole number of
    // cells leaves a sliver past the last one, and so does the cell cap on an enormous display.
    if (column >= u.field.x || row >= u.field.y) {
        return vec4<f32>(u.back.rgb, 1.0);
    }

    let texel = textureLoad(field, vec2<i32>(i32(column), i32(row)), 0);
    // What the cell is: one for alive, zero for empty, and anything between for a cell part-way
    // through dying, counting down as it goes.
    let life = texel.r;
    // How long it has been that: the age of a live cell, the trail left by a dead one.
    let mark = texel.g;

    var colour: vec3<f32>;
    if (life > 0.998) {
        // Alive. With the age reading off, `settled` is one and every live cell is `live`;
        // with it on, a cell arrives at `fresh` and eases towards `live` as it holds.
        let settled = mix(1.0, mark, u.reading.x);
        colour = mix(u.fresh.rgb, u.live.rgb, settled);
    } else if (life > 0.001) {
        // A Generations cell on its way out. Nothing is ever this colour in a two-state rule.
        colour = mix(u.back.rgb, u.dying.rgb, life);
    } else {
        // Empty, and possibly still warm. Squared, so a trail falls away quickly at first and
        // then lingers, which is what a long exposure looks like.
        colour = mix(u.back.rgb, u.trail.rgb, mark * mark * u.reading.y);
    }

    // The line between cells, drawn along the top and left of each one. Only when a cell is
    // big enough to have an inside worth bounding.
    if (u.screen.z > 0.5 && cell >= u.reading.z) {
        let inside = pixel - vec2<f32>(column, row) * cell;
        if (inside.x < 1.0 || inside.y < 1.0) {
            colour = mix(colour, u.ink.rgb, u.screen.w);
        }
    }

    // Everything above was done between the palette's numbers as they are written, which are
    // display values. The frame is sRGB, so the hardware is about to apply the display curve
    // itself; this takes it off first, once, on the one colour that came out.
    return vec4<f32>(to_light(colour), 1.0);
}

// A display value to the light it stands for: the sRGB transfer curve, inverted.
fn to_light(colour: vec3<f32>) -> vec3<f32> {
    let low = colour / 12.92;
    let high = pow((colour + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, colour <= vec3<f32>(0.04045));
}
