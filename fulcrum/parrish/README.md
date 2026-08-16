# Parrish

A live cloud field over still water,
painted the way Maxfield Parrish painted:
in transparent coats over a white ground.

```
cargo run -p parrish --release
```

The window opens borderless over the whole display.
`F11` puts it back in a window.

It is the fourth of five pieces about the same sky:

| | how the clouds are made |
|---|---|
| [`thunderhead`](../thunderhead/README.md) | bitmaps, grown on the CPU and blitted |
| [`nimbus`](../nimbus/README.md) | a real volume, raymarched on the GPU |
| [`ligne`](../ligne/README.md) | a two-dimensional field, in flat colour with a line round it |
| **`parrish`** | the same field, glazed |
| [`moebius`](../moebius/README.md) | no field at all: overlapping circles, and the arcs left over |

`ligne` and this one share their machinery almost exactly and could not look less alike,
which is the reason to have both.
One is a pen: flat areas of colour with a line of constant width around everything.
The other has no line anywhere and no flat area anywhere,
and every colour in it is arrived at rather than chosen.

## Glazing

Parrish did not mix paint.
He laid down a white ground,
then covered it with thin transparent films of a single pigment,
varnishing between coats and building up as many as thirty of them.
The light you see has gone down through the colour, off the white, and back out through it again.
That is why the blue is that deep and still looks lit from within,
and why the shadows are saturated blue instead of the grey that mixing gives you.

The whole piece is that, in one function:

```wgsl
fn glaze(under: vec3<f32>, tint: vec3<f32>, depth: f32) -> vec3<f32> {
    return under * pow(max(tint, vec3<f32>(0.015)), vec3<f32>(2.0 * depth));
}
```

`tint` is a transmittance: what one coat lets through, per channel.
`depth` is how many coats.
The exponent is doubled because the light passes through the film twice.

So [`look.rs`](src/look.rs) holds no colours at all.
Thirteen transmittances and three coat counts per palette,
and the picture is what you get by deciding how many coats of what
stand between the eye and the ground.
Each one was arrived at backwards:
pick the colour a thing should come out as, divide by the ground,
and take the root of however many coats are in front of it.
A tint on its own tells you nothing about what it will look like on the wall.

Two consequences run through everything else.

**Coats multiply.**
Two coats are the tint squared, not the tint at twice the strength.
Colour deepens and saturates along a curve instead of sliding towards whatever it is mixed with,
which is the difference between a sky that gets darker and a sky that gets *bluer*.

**A glaze can only darken.**
Nothing laid over white comes out brighter than white.
Every luminous passage in one of these pictures is paint that was not laid, or paint taken back off.
So `glaze` takes a negative depth too, and that is where the light in the picture comes from:
the sun's glow is the blue of the sky scrubbed back towards the bare ground,
and the bright edge of a backlit cloud is its own shadow coats lifted off again.
Both are capped at what was laid on, so they reach the ground and stop.
The sun's disc is the one thing in the frame allowed past it.

Aerial perspective falls out of the same idea and is the clearest case.
The usual mix towards a haze colour goes milky and grey.
A painter in glazes does the opposite: the far range gets *fewer* coats and one thin blue over the top,
so it comes out paler **and** more saturated than the near one.
That is why distance reads the way it does in these pictures,
and here it is a multiplier on three numbers.

## The clouds

The machinery is `ligne`'s, and it is cheap.
A deck is a horizontal plane at a given altitude.
A ray is intersected with it once and a sheet of tiling noise is read where it lands,
so one texture fetch is a whole cloud field.
Three decks, drawn back to front, lowest and nearest first.

Four things sit on top of that, and each of them fixed something that was visibly wrong.

**The cloud stands up, and the light thinks it is shorter than it is.**
A plane seen from underneath is foreshortened by the sine of the angle you look up at,
so a cloud painted on one is a six-to-one smear at twenty degrees.
What stands it back up is height: a ray meets the top of a tall cloud well before the plane underneath.
Asking how high the cloud is here and re-intersecting at that height is the whole trick,
and the piece keeps two separate numbers for it.
`TOWER` is how far a cloud stands off its deck, and it is large.
`RELIEF` is how much height the light thinks it is walking over when it takes the field's slope for a normal,
and it is smaller, because a normal built from the full tower is a cliff everywhere:
one blazing face, one black one, and no turn between them.
They were the same number for a while and the clouds had a hard metallic edge to every lobe.

**Two damped rounds, not one.**
That re-intersection is a fixed point, and at a grazing angle it is a badly behaved one:
a hundred metres of height moves the answer a kilometre along the ground.
Undamped, the guess walks off across the sky, lands under a different cloud
and brings that one's height back instead.
What you get is a picture of clouds pulled out into ribbons.
Damped to a little over half, two rounds settle rather than swing.

**The light wraps.**
A cloud is not an opaque solid.
Light entering the lit side scatters through and comes back out some way round the shoulder,
which is why a real cumulus has no hard terminator on it.
Without that, the flat top of a deck under a sun a few degrees up is as dark as its underside
and the light lands only on whichever flanks happen to face the sun.
That looks like lace, not like weather.

**The edge is squeezed and the inside is not.**
Gradient noise is soft by construction, and a soft field thresholded gives a woolly outline.
The field goes through a `smoothstep` before it is thresholded, which puts the whole transition
into a fifth of the range: the outline arrives in a couple of pixels and looks cut.
The shading is read off the unsqueezed channel over a long step,
so the inside of the cloud stays smooth while its edge stays fussy.
That split is most of what makes these read as painted rather than as noise.

There is one honest limit, and the piece gives in to it rather than fighting it.
Below about eight degrees the foreshortening wins:
a round cloud is a ten-to-one ribbon and a skyful of them is a mat.
So the decks fade out into the band above the horizon.
Every one of these paintings leaves that band clear and luminous anyway, for its own reasons.

## Below the sky

A Parrish is lit by contrast against something dark and near, not by anything in it being white.
So there are three silhouettes stacked into the distance,
each a shade deeper than the one behind it:
a far range of hills, a nearer one, and a rock across the bottom of the frame.
Each is one glaze at full depth over the ground, and `tests/sky.rs` holds them to deepening forward.

Between the hills and the rock is water, and the water is the sky again.
A flat mirror reflects the ray `(x, -y, z)`, so the lower part of the frame runs the same
sky-and-clouds function a second time with the ray turned over,
one mip level blurrier because a reflection never is as sharp as the thing it reflects,
and then glazed with a coat of water.
How many coats depends on the angle:
water at a grazing angle is a mirror and water underfoot is a hole,
so the far water is glass and the near water is deep.
A ripple is a tilt on the surface and a tilt is a bend in the reflected ray,
scaled down with distance until the far shore is still.

## No derivatives, so it can branch

There is no `fwidth` anywhere in this shader, and that is deliberate.

Every mip level is chosen from the world footprint of a pixel.
Every soft edge is worked out the same way:
the silhouette's width comes from the field's own slope against that footprint,
using the two taps the normal already needed.

Screen-space derivatives are only meaningful where all four pixels of a quad are doing the same thing,
which is why the piece next door is written branch-free
and computes its sky, its desert and all three decks for every pixel before choosing between them.
This one is free to take a different path through the frame,
and it does: below the horizon it looks at the sky upside down instead,
which is worth about a third of the frame's work.

## Why the numbers are what they are

- **The frame is aimed low and opened wide.**
  A low horizon with a band of water under it and a rock across the bottom is the composition these paintings use.
  It costs something: at this pitch most of the visible sky is at a shallow angle,
  where a deck is at its most foreshortened. Hence the tower, and hence the fade.
- **The weather map reads over three tiles, not nine.**
  It decides which districts of the sky are cloudy.
  The whole world here is ninety kilometres across, and a map with districts larger than that has no districts:
  the sky comes out uniformly covered, everywhere, always. It did, for two renders.
- **The two cloud fields are scrolled apart.**
  Two fields moving together are a picture being slid across the window.
  Two moving at different speeds are a picture being repainted:
  the sum is not a translation of anything, so clouds grow, lean, split and close again as they cross.
- **The wind is slow.**
  Half what the pieces next door use. These clouds are painted standing still,
  and a monument that scuds along reads as steam.
- **The sheet carries its own mip chain.**
  A deck seen edge-on is minified by a factor of hundreds, and wgpu will not build the levels for you.

## Working it

| | |
|---|---|
| `up` / `down` | the pace of the wind |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `P` | palette: daybreak, cobalt, hilltop, twilight, ecstasy |
| `H` | hide the readout |
| `F11` | leave fullscreen |

## Seeing it without a window

```sh
cargo run -p parrish --release --example parrish_still -- still.png 1920 1200 0 3400
```

The arguments are the file, the size in pixels, which palette,
and how far the wind has carried the sky in metres.
It builds a headless device, renders through exactly the same pass the window does,
copies the result back and writes a PNG, timing eight frames on the way.
Every number in this piece was set by looking at one of those,
and the four paragraphs above about what was wrong are all things that were only visible in one.
