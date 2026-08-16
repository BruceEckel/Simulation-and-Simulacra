// Maxfield Parrish clouds: three decks of live two-dimensional cloud field over still water,
// painted in transparent coats over a white ground, in one pass.
//
// The cloud machinery is the cheap two-dimensional kind. A deck is a horizontal plane at a given
// altitude; a ray is intersected with it once, and a sheet of tiling noise is read at the point
// where it lands. There is no volume and nothing is marched.
//
// What makes the picture is what happens after that. Nothing here is a colour being mixed with
// another colour. Every value starts as the white ground and is then seen through a stack of
// transparent coats, each one a transmittance raised to the number of coats of it. That is how
// these paintings were actually made -- thin glazes of a single pigment over a white ground,
// varnished between coats, never mixed on the palette -- and it is why the blue can be that deep
// and still look lit from behind, and why the shadows come out saturated instead of grey.
//
// One consequence runs through everything below: a glaze can only ever darken. Nothing laid over
// white gets brighter than white. So every bright passage in this picture is paint *not* laid or
// paint taken back off, which is what `glaze` with a negative depth is for.
//
// There is no screen-space derivative anywhere in here. Every softness is worked out from a
// pixel's own footprint on the thing it is looking at, and every mip level is chosen the same
// way. That is what lets the shader branch: the lower part of the frame takes a different path
// through the same functions, looking at the sky upside down in the water.

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
    // xyz: how far the wind has carried the sky, in metres. w: how far the second field has been
    // carried past the first, which is what makes the clouds boil.
    wind: vec4<f32>,
    // Three decks, near one first: altitude, metres to one tile of the sheet, coverage
    // threshold, how many coats of distance it already stands behind.
    deck_a: vec4<f32>,
    deck_b: vec4<f32>,
    deck_c: vec4<f32>,
    // how tall a cloud stands in metres, how much relief the light sees, shadow reach as a
    // fraction of a tile, shadow strength.
    puff: vec4<f32>,
    // coats: light, shadow, deep, cast.
    coats: vec4<f32>,
    // rim lift, silver tightness, sun disc cosine, crown height.
    edge: vec4<f32>,
    // zenith coats, horizon coats, glow coats, glow tightness.
    dial: vec4<f32>,
    // coats of distance per metre, water coats, mirror floor, edge contrast.
    air: vec4<f32>,
    // ledge rise, ridge rise, ripple height, light wrap.
    land: vec4<f32>,
    // Every one of these is a transmittance: what one coat of it lets through.
    ground: vec4<f32>,
    sky_high: vec4<f32>,
    sky_low: vec4<f32>,
    glow: vec4<f32>,
    cloud_light: vec4<f32>,
    cloud_shadow: vec4<f32>,
    cloud_deep: vec4<f32>,
    distance: vec4<f32>,
    water: vec4<f32>,
    ridge_far: vec4<f32>,
    ridge_near: vec4<f32>,
    ledge: vec4<f32>,
    // width, height, 1/width, 1/height.
    screen: vec4<f32>,
    // planet radius, furthest anything is drawn, water tile in metres, sheet size in texels.
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
// coats
// ---------------------------------------------------------------------------------------

// One glaze over what is already there.
//
// This is the whole piece in one line. A glaze is a transparent film with no body of its own:
// the light goes down through it, off the white ground, and back up through it again, so what
// comes back has been through the tint twice. `depth` is how many coats, and coats multiply
// rather than add, which is why the colour deepens and saturates along a curve instead of
// sliding towards whatever it is being mixed with.
//
// A negative depth lifts coats back off, towards the bare ground. That is the only way anything
// in this picture gets brighter, and it is what the light around the sun and along the edge of a
// backlit cloud are made of: not paint added, paint taken away.
fn glaze(under: vec3<f32>, tint: vec3<f32>, depth: f32) -> vec3<f32> {
    return under * pow(max(tint, vec3<f32>(0.015)), vec3<f32>(2.0 * depth));
}

// What the ground can give back is at most white, and the sun is brighter than that.
//
// Rolled off rather than clipped, so the disc keeps its shape instead of spreading into a flat
// white blob, and so the two or three places where the lift overshoots come back down gracefully
// instead of leaving a hard edge in the middle of the sky.
fn shoulder(c: vec3<f32>) -> vec3<f32> {
    let knee = vec3<f32>(0.72);
    let over = max(c - knee, vec3<f32>(0.0));
    return min(c, knee) + (vec3<f32>(1.0) - knee) * (vec3<f32>(1.0) - exp(-over / (vec3<f32>(1.0) - knee)));
}

// `over`, the painter's operator: what is in front, over what is behind.
fn over(under: vec3<f32>, top: vec4<f32>) -> vec3<f32> {
    return mix(under, top.rgb, top.a);
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
    );
    // The fine channel comes off that second read for nothing, since it is the same fetch. It
    // goes into the sum at a tenth, which is enough to put knuckles on the outline of a cloud
    // without putting any into the shading: the silhouette in one of these paintings is fussy
    // and everything inside it is smooth, and that is the whole difference in one number.
    let f = first * 0.56 + second.g * 0.34 + second.a * 0.10;
    // Both halves come back: the sum is the cloud, and the first field on its own is what the
    // shading and the shadow walk are read from. Those want the shape of the lobe rather than
    // the fine grain on it, and taking them off one read instead of two is a third of the
    // texture traffic of the whole frame.
    return vec2<f32>(f, first);
}

// The slow half of the field, for the six reads a deck's shading needs.
fn deck_base(p: vec2<f32>, deck: vec4<f32>, lod: f32) -> f32 {
    return textureSampleLevel(sheet, sheet_sampler, (p + u.wind.xz) / deck.y, lod).r;
}

// Where the weather is: a reading of the same sheet over a few tiles at once, so it varies over
// the whole width of the sky and not over one cloud. It decides which districts are cloudy, and
// it is what leaves the big holes of open blue these pictures need.
//
// Three tiles rather than the nine you might reach for. The whole world here is ninety
// kilometres across, and a weather map with a district larger than that has no districts in it:
// the sky comes out uniformly covered, everywhere, always.
fn weather(p: vec2<f32>, deck: vec4<f32>, lod: f32) -> f32 {
    let q = (p + u.wind.xz) / (deck.y * 3.2);
    return textureSampleLevel(sheet, sheet_sampler, q, max(lod - 1.68, 0.0)).b;
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

// Colour and coverage of one deck along `dir`, already glazed.
fn draw_deck(dir: vec3<f32>, deck: vec4<f32>, pixel: f32, bias: f32) -> vec4<f32> {
    let up = max(dir.y, 0.004);
    var reach = min(shell_exit(dir, u.world.x + deck.x), u.world.y);
    var p = u.origin.xz + dir.xz * reach;

    // How much of the plane one pixel covers, and from that which level of the sheet to read. A
    // deck seen edge-on near the horizon is minified enormously, and reading the top level there
    // gives a band of sparkling confetti instead of distant cloud.
    let footprint = reach * pixel / up;
    // Softened with distance on top of that, which is the same decision as the thinning glaze
    // further down and belongs with it. A painter working towards the horizon does not paint the
    // same cloud smaller, he paints fewer marks: the far bank is two or three strokes and the
    // near one is twenty. Read at the level the footprint asks for and the far deck comes back
    // as a mat of ten-to-one ribbons, every one of them resolved and none of them legible.
    let soften = clamp(deck.w + reach * u.air.x, 0.0, 0.85);
    let lod = max(log2(footprint / deck.y * u.world.w), 0.0) + bias + soften * 2.6;

    // Stand the sample on the cloud's own top.
    //
    // A deck is a plane, but the cloud standing on it is not, and a ray meets the top of a cloud
    // some way before it meets the plane underneath. Asking how high the cloud is here and then
    // re-intersecting at that height is the difference between a pattern painted on a ceiling
    // and a heap with a side to it: near the horizon the clouds stand up and show their flanks,
    // and overhead they bulge towards you and cover the ones behind.
    //
    // Two rounds, and each one damped to a little over half. Undamped, at a grazing angle a
    // hundred metres of height moves the answer a kilometre along the ground, so the guess walks
    // off across the sky, lands under a different cloud entirely and brings that one's height
    // back instead. What you get for it is a picture of clouds pulled out into ribbons. Damped,
    // the two rounds settle rather than swing, and the height that comes back belongs to the
    // cloud the ray is actually pointing at.
    var height = 0.0;
    for (var round = 0; round < 2; round = round + 1) {
        let here = deck_base(p, deck, lod);
        let want = clamp((here - deck.z) * u.puff.x, 0.0, deck.x * 1.1);
        height = mix(height, want, 0.55);
        reach = min(shell_exit(dir, u.world.x + deck.x + height), u.world.y);
        p = u.origin.xz + dir.xz * reach;
    }

    let both = deck_field(p, deck, lod);
    // Squeezed, so that the outline of a cloud arrives in a couple of pixels rather than twenty.
    // Gradient noise is soft everywhere by construction, and a soft field thresholded gives a
    // woolly edge; these clouds want an edge that looks cut.
    let f = smoothstep(0.5 - u.air.w, 0.5 + u.air.w, both.x);
    let base = both.y;
    let local = deck.z * (1.25 - 0.62 * weather(p, deck, lod));
    let thickness = f - local;

    // The slope of the field, in field units per metre, taken from two more reads a small step
    // away. This is what stands in for a volume: a two-dimensional field read as the height of a
    // cloud top has a normal, and a normal is all the light needs.
    //
    // Read over a long step on purpose. The field has octaves down to a few hundred metres, and
    // a slope measured across sixty of them is the slope of the smallest thing in it: the light
    // would then follow the grain instead of the shape. This measures the lobe.
    let step = max(deck.y * 0.050, footprint);
    let along = (deck_base(p + vec2<f32>(step, 0.0), deck, lod) - base) / step;
    let across = (deck_base(p + vec2<f32>(0.0, step), deck, lod) - base) / step;
    let normal = normalize(vec3<f32>(-along * u.puff.y, 1.0, -across * u.puff.y));
    // Wrapped past the terminator, because a cloud is not an opaque solid: light entering the
    // lit side scatters through and comes back out some way round the shoulder. Without it, the
    // flat top of a deck under a sun a few degrees up is as dark as its underside and the light
    // lands only on whichever flanks happen to face the sun, which is lace rather than weather.
    let raw = dot(normal, u.sun.xyz);
    let lambert = clamp((raw + u.land.w) / (1.0 + u.land.w), 0.0, 1.0);

    // Cast shadow, the same way: walk a few steps towards the sun along the deck and ask whether
    // the cloud over there stands higher than the sun ray does by the time it gets there. Three
    // taps is not many, but it is enough to put one cloud's shadow on the shoulder of the next,
    // which is the thing that makes a flat field read as a heap.
    let flat_sun = normalize(vec2<f32>(u.sun.x, u.sun.z) + vec2<f32>(1e-5, 0.0));
    let climb = max(u.sun.y, 0.05) / u.puff.x;
    let reach_shadow = deck.y * u.puff.z;
    var blocked = 0.0;
    for (var i = 1; i <= 3; i = i + 1) {
        let s = f32(i) / 3.0 * reach_shadow;
        let there = deck_base(p + flat_sun * s, deck, lod);
        blocked = max(blocked, there - base - s * climb);
    }
    let shadow = clamp(blocked * u.puff.w, 0.0, 1.0);

    // How much of a cloud is standing here, and how much light comes straight through it.
    let crown = smoothstep(0.0, u.edge.w, thickness);
    let towards = max(dot(dir, u.sun.xyz), 0.0);
    let rim = pow(towards, u.edge.y) * (1.0 - crown);

    // Distance is painted as *less paint*, not as more.
    //
    // Aerial perspective the usual way is a mix towards a haze colour, which goes milky and grey.
    // A painter working in glazes does the opposite: the far range gets fewer coats and a single
    // thin blue over the top, so it comes out paler *and* more saturated than the near one. That
    // is the whole reason distance reads the way it does in these pictures.
    let fade = clamp(deck.w + reach * u.air.x, 0.0, 0.85);
    let near = 1.0 - fade;

    let deep = u.coats.z * crown * (1.0 - lambert) * (1.0 - lambert) * near;
    let cool = (u.coats.y * (1.0 - lambert) + u.coats.w * shadow) * near;
    let warm = u.coats.x * lambert * near;

    var c = u.ground.rgb;
    c = glaze(c, u.cloud_deep.rgb, deep);
    c = glaze(c, u.cloud_shadow.rgb, cool);
    c = glaze(c, u.cloud_light.rgb, warm);
    // The edge with the light behind it: coats taken back off, never more than were laid on, and
    // a breath of warmth put in their place. A cloud in one of these paintings is at its
    // brightest where it is thinnest, and this is that.
    let scrub = min(deep + cool, u.edge.x * rim);
    c = glaze(c, u.cloud_shadow.rgb, -scrub);
    c = glaze(c, u.glow.rgb, scrub * 0.5);
    c = glaze(c, u.distance.rgb, fade);

    // The silhouette. Its width comes from the field's own slope against the footprint of a
    // pixel: how much `thickness` changes across one pixel, worked out from the two taps the
    // normal already needed rather than from a screen-space derivative. That is what leaves the
    // shader free to branch.
    let slope = max(length(vec2<f32>(along, across)), 1e-7);
    let width = max(slope * footprint * 0.8, 1e-6);
    var alpha = smoothstep(-width, width, thickness);
    // And faded out into the band along the horizon.
    //
    // Not a fudge: it is the one place where the geometry of a cloud deck made of a plane runs
    // out. A cloud on a horizontal plane is squashed on the screen by the sine of the angle you
    // are looking up at, so at five degrees a round cloud is a ten-to-one ribbon, and a skyful of
    // them is a mat of noise with the horizon behind it. Every one of these paintings does the
    // same thing here for a different reason: the band above the horizon is left clear and
    // luminous, and the clouds begin some way up. So they begin some way up.
    alpha = alpha * smoothstep(0.030, 0.230, dir.y);
    return vec4<f32>(c, alpha);
}

// ---------------------------------------------------------------------------------------
// the sky behind them
// ---------------------------------------------------------------------------------------

// Coats of blue over a white ground, thinning towards the horizon, with the coats scrubbed back
// off again around the sun.
fn draw_sky(dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(dir.y, 0.0, 1.0);
    var c = u.ground.rgb;

    // The warm band along the horizon is the ground left nearly bare. In one of these paintings
    // that band is the light source, whether or not the sun is in the frame.
    c = glaze(c, u.sky_low.rgb, u.dial.y * pow(1.0 - up, 5.0));

    // And the blue, piling up towards the zenith. The exponent is well under one, so the blue
    // arrives fast off the horizon and then deepens slowly: a sky glazed linearly reads as a
    // gradient, and this one has to read as a *colour* with a light band under it.
    let coats = u.dial.x * pow(up, 0.32);
    c = glaze(c, u.sky_high.rgb, coats);

    // Around the sun the blue is scrubbed back off and a warm coat goes on in its place. Never
    // more than was laid on, so the sky there reaches the bare ground and stops.
    //
    // Not a `min`, though, which would leave a crease running clear across the sky along the
    // line where its two arguments cross. This is the two of them folded together instead: half
    // their harmonic mean, which is the smaller of the two nearly everywhere and bends rather
    // than corners where they meet.
    let towards = max(dot(dir, u.sun.xyz), 0.0);
    let wanted = u.dial.z * pow(towards, u.dial.w);
    let lift = coats * wanted / (coats + wanted + 1e-5);
    c = glaze(c, u.sky_high.rgb, -lift);
    c = glaze(c, u.glow.rgb, lift * 0.55);

    // The disc: the one place in the picture that is brighter than the ground it is painted on.
    let disc = smoothstep(u.edge.z, u.edge.z + 0.00006, dot(dir, u.sun.xyz));
    return mix(c, u.ground.rgb * 1.7, disc);
}

// ---------------------------------------------------------------------------------------
// the land, and the water it stands in
// ---------------------------------------------------------------------------------------

// The two ranges of hills, as heights in the same units as `dir.y`.
//
// Read around a ring in the sheet rather than off an angle, so there is no seam behind you where
// the angle wraps: any closed curve laid on a tiling sheet gives a profile that closes on itself.
fn ridges(dir: vec3<f32>) -> vec2<f32> {
    let flat = normalize(vec2<f32>(dir.x, dir.z) + vec2<f32>(1e-5, 0.0));
    let ring = flat * 0.37 + vec2<f32>(0.5, 0.5);
    let broad = textureSampleLevel(sheet, sheet_sampler, ring, 2.0).b;
    let rough = textureSampleLevel(sheet, sheet_sampler, ring * 2.7 + vec2<f32>(0.13, 0.41), 1.0).a;
    let other = textureSampleLevel(sheet, sheet_sampler, ring * 1.6 + vec2<f32>(0.61, 0.07), 1.0).r;
    let far = (broad * 0.68 + rough * 0.32 - 0.44) * u.land.y;
    let near = (other * 0.70 + rough * 0.30 - 0.50) * u.land.y * 1.7;
    return vec2<f32>(far, near);
}

// The top of the rock across the bottom of the frame, as a depression below the horizon.
fn ledge_top(dir: vec3<f32>) -> f32 {
    let flat = normalize(vec2<f32>(dir.x, dir.z) + vec2<f32>(1e-5, 0.0));
    let ring = flat * 0.21 + vec2<f32>(0.5, 0.5);
    let broad = textureSampleLevel(sheet, sheet_sampler, ring, 1.0).a;
    let rough = textureSampleLevel(sheet, sheet_sampler, ring * 3.1 + vec2<f32>(0.27, 0.53), 0.0).r;
    // A third turn of the same crank, nine times faster. The near rock is the only thing in the
    // picture close enough to have a texture, and a foreground silhouette that curves smoothly
    // reads as a hill fifty miles off rather than as a rock at your feet.
    let grit = textureSampleLevel(sheet, sheet_sampler, ring * 9.7 + vec2<f32>(0.71, 0.19), 0.0).g;
    return -u.land.x * (0.42 + 0.86 * (broad * 0.62 + rough * 0.26 + grit * 0.12));
}

// Everything above the water line: the sky, the three decks in it, and the hills standing in
// front of the lot. Called twice for every pixel in the lower part of the frame, the second time
// with the ray turned upside down.
fn scene(dir: vec3<f32>, pixel: f32, bias: f32) -> vec3<f32> {
    var c = draw_sky(dir);

    // Far deck first. A nearer deck is a lower one: at any angle above the horizon, the lower
    // plane is the closer of the two.
    c = over(c, draw_deck(dir, u.deck_c, pixel, bias));
    c = over(c, draw_deck(dir, u.deck_b, pixel, bias));
    c = over(c, draw_deck(dir, u.deck_a, pixel, bias));

    // The hills go over the clouds, not under them: they are thirty kilometres away and the
    // cloud bank behind them is further. A Parrish distance is a stack of silhouettes, each one
    // a flat shape a shade deeper than the one behind it, and this is two of the three -- the
    // ledge across the bottom of the frame is the third.
    let hills = ridges(dir);
    let soft = pixel * 0.7;
    c = mix(c, glaze(u.ground.rgb, u.ridge_far.rgb, 1.0), 1.0 - smoothstep(-soft, soft, dir.y - hills.x));
    c = mix(c, glaze(u.ground.rgb, u.ridge_near.rgb, 1.0), 1.0 - smoothstep(-soft, soft, dir.y - hills.y));
    return c;
}

// The water: the same scene again, upside down, seen through a coat of water.
fn draw_water(dir: vec3<f32>, pixel: f32) -> vec3<f32> {
    let down = min(dir.y, -1e-4);
    let reach = min(-u.origin.y / down, u.world.y);
    let p = u.origin.xz + dir.xz * reach;
    let lod = max(log2(reach * pixel / (-down) / u.world.z * u.world.w), 0.0);

    // A ripple is a tilt on the surface, and a tilt on the surface is a bend in the reflected
    // ray. Scaled down with distance, because a wave a metre across subtends less and less until
    // the far water is a mirror: still water in one of these paintings is glass at the far shore
    // and broken only at your feet.
    let wave = textureSampleLevel(sheet, sheet_sampler, p / u.world.z, lod).a - 0.5;
    let fine = textureSampleLevel(
        sheet,
        sheet_sampler,
        p / (u.world.z * 0.23) + vec2<f32>(0.31, 0.77),
        lod,
    ).r - 0.5;
    let close = 1.0 / (1.0 + reach * 0.004);
    let tilt = (wave * 0.7 + fine * 0.3) * u.land.z * close;

    let mirrored = normalize(vec3<f32>(dir.x, -dir.y + tilt, dir.z));
    // Blurred by one mip level, which is both cheaper and truer: a reflection off water is never
    // as sharp as the thing it reflects.
    let image = scene(mirrored, pixel, 1.0);

    // Water at a grazing angle is a mirror and water underfoot is a hole. So the coats of water
    // over the reflection are heaviest where you are looking most steeply into it.
    let grazing = pow(1.0 - min(-down, 1.0), 3.0);
    let mirror = mix(u.air.z, 1.0, grazing);
    return glaze(image, u.water.rgb, u.air.y * (1.0 - mirror));
}

// ---------------------------------------------------------------------------------------
// the frame
// ---------------------------------------------------------------------------------------

@fragment
fn fs_main(in: Varying) -> @location(0) vec4<f32> {
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let dir = normalize(
        u.forward.xyz
            + u.right.xyz * ndc.x * u.origin.w * u.forward.w
            + u.up.xyz * ndc.y * u.origin.w
    );
    // How much of the world one pixel covers, as an angle. Every footprint in the shader is this
    // multiplied by a distance.
    let pixel = 2.0 * u.origin.w / u.up.w;

    // The horizon, with a band of a pixel or so across it where both halves are worked out and
    // blended. Everywhere else takes one path or the other, which is worth about a third of the
    // frame: the sky is not painted twice for the pixels that are not water.
    let soft = pixel * 0.6;
    var colour: vec3<f32>;
    if (dir.y > soft) {
        colour = scene(dir, pixel, 0.0);
    } else if (dir.y < -soft) {
        colour = draw_water(dir, pixel);
    } else {
        colour = mix(
            draw_water(dir, pixel),
            scene(dir, pixel, 0.0),
            smoothstep(-soft, soft, dir.y),
        );
    }

    // The rock across the bottom, over everything, at full weight. The darkest thing in the
    // picture, and the reason the sky above it reads as bright: these paintings are lit by
    // contrast against a near silhouette, not by having anything in them that is actually white.
    let top = ledge_top(dir);
    let rock = glaze(u.ground.rgb, u.ledge.rgb, 1.0);
    colour = mix(colour, rock, 1.0 - smoothstep(-soft, soft, dir.y - top));

    return vec4<f32>(shoulder(colour), 1.0);
}
