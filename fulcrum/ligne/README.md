# Ligne

A live two-dimensional cloud field over a desert,
drawn in flat colour with a clean line around everything.

*Ligne claire* is the drawing style: uniform line weight, flat areas of colour, no hatching and no shading.
This is that style computed rather than drawn,
at one sample per pixel, sixty times a second.

```
cargo run -p ligne --release
```

The window opens borderless over the whole display.
`F11` puts it back in a window.

It is the third of five pieces about the same sky:

| | how the clouds are made |
|---|---|
| [`thunderhead`](../thunderhead/README.md) | bitmaps, grown on the CPU and blitted |
| [`nimbus`](../nimbus/README.md) | a real volume, raymarched on the GPU |
| **`ligne`** | a two-dimensional field, evaluated per pixel |
| [`parrish`](../parrish/README.md) | the same field, glazed instead of drawn |
| [`moebius`](../moebius/README.md) | no field at all: overlapping circles, and the arcs left over |

## Two-dimensional, and still volumetric

A cloud deck here is a horizontal plane at a given altitude.
A ray is intersected with it **once** and a sheet of tiling noise is read at the point where it lands.
There is no volume, nothing is marched, and one texture fetch is a whole cloud field.

The depth comes from three things stacked on that:

- **The sample is lifted onto the cloud's own top.**
  A deck is a plane, but the cloud standing on it is not,
  and a ray meets the top of a cloud some way before it meets the plane underneath.
  Two rounds of asking how high the cloud is here and re-intersecting at that height
  converge on the top for anything but a cliff.
  It is the difference between a pattern painted on a ceiling and a heap with a side to it:
  near the horizon the clouds stand up and show their flanks, and overhead they bulge towards you.
- **The field is lit as a height field.**
  Its slope, measured over a long enough step to find the lobe rather than the grain, gives a normal, and a normal is all the light needs.
- **A short walk towards the sun casts a shadow.**
  Three taps ask whether the cloud over there stands higher than the sun ray does by the time it gets there.
  It puts one cloud's shadow on the shoulder of the next, which is what makes a flat field read as a heap.

Three decks at three altitudes, drawn back to front, complete the depth:
lower is nearer, so the low deck carries the big shapes overhead
and the high ones stack up behind it towards the horizon.

## The line

Every line in the picture is analytic, and the whole style rests on one function:

```wgsl
fn line_width(value: f32, pixels: f32) -> f32 {
    return max(fwidth(value), 1e-7) * pixels * 0.5;
}
```

`fwidth` says how much a value changes from one pixel to the next,
so dividing a pixel count by it converts a width *on the screen* into a width in whatever that value measures.
A contour drawn this way is the same weight everywhere:
on a cloud overhead, on one at the horizon, on the same cloud after the window is resized,
and on a four-thousand-pixel display where a texel-wide line would have vanished.

The whole shader is branch-free so that this is legal:
`fwidth` is only meaningful where all four pixels of a quad are doing the same thing,
so the sky, the desert, the rock and all three decks are computed for every pixel and then selected between.
That costs a little and buys a line that never thickens, never thins and never breaks.

Everything is drawn with it: the cloud silhouettes (heavier),
the tone contours inside them (lighter, the way a pen models a shape after outlining it),
the sky's bands, the desert's bands, the horizon, the rock, and the ring around the sun.

There is one more thing the line needs, which took a while to find.
A line is only a line while the thing it follows is bigger than a pixel.
Where a value swings through several bands between neighbouring pixels,
which is what happens to every field in this picture as it approaches the horizon,
asking for a line of constant width asks for a line wider than the shape it draws,
and the honest answer is that there is nothing there to draw.
Without that test the bottom of the sky fills in solid.
The same test unflattens the colour bands where their steps have gone sub-pixel:
a staircase sampled below its own tread size is not a staircase, it is moiré.

## Flat, and interesting

Nothing in the palette is a radiance and nothing is tone mapped:
these are the colours that land on the screen.
The light decides *which* band a patch of cloud is in and the band decides the colour,
so a cloud is five flat areas and four lines, and a sky is nine flat areas and eight.

That makes a palette a real decision rather than a tint.
Eleven colours each: four cloud stops from a rain-dark base to a lit crown,
two ends of sky, two of sand, a rock, and a line.
`tests/sky.rs` holds each one to being a whole sky —
the cloud ramp has to brighten all the way up,
the ink has to be the darkest thing in the palette,
and what distance fades into has to be what the horizon already is,
or the horizon comes out as a seam between two different skies.

## Why the numbers are what they are

- **The frame points up.** A deck of cloud is a horizontal plane,
  so a cloud on it is squashed on the screen by the sine of the angle you are looking up at:
  at ten degrees a round cloud is a six-to-one smear.
  Everything that reads as a cloud rather than as an oil slick is above twenty degrees,
  so that is where the frame is aimed, with the horizon along the bottom edge to measure the clouds against.
- **The two cloud fields are scrolled apart.**
  Two fields moving together are a picture being slid across the window.
  Two fields moving at different speeds are a picture being redrawn:
  the sum is not a translation of anything, so clouds grow, lean, split and close again as they cross,
  and the sky never repeats.
- **The sheet carries its own mip chain.**
  A deck seen edge-on near the horizon is minified by a factor of hundreds,
  and wgpu will not build the levels for you.
  The shader picks the level analytically from the world footprint of a pixel,
  rather than from screen-space derivatives, because a plane projection near the horizon makes those meaningless.
- **Every channel of the sheet is stretched over the byte it is stored in.**
  Stacked octaves of gradient noise pile up around the middle of their range and leave both ends empty,
  and every threshold in the shader is a threshold *against these numbers*:
  a channel using half its range gives a coverage dial with half its travel and twice the terracing.
- **One sample per pixel, no internal resolution.**
  The volumetric piece next door can afford to march fewer rays than the window has pixels and upscale.
  This one cannot: the style is a line a pixel and a half wide, and there is nowhere to hide a soft one.
  It does not need to. A frame is one triangle and about twenty texture fetches a pixel.

## Working it

| | |
|---|---|
| `up` / `down` | the pace of the wind |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `P` | palette: arzach, verdigris, ember, nocturne, mineral |
| `H` | hide the readout |
| `F11` | leave fullscreen |

## Seeing it without a window

```sh
cargo run -p ligne --release --example ligne_still -- still.png 1920 1200 0 3000
```

The arguments are the file, the size in pixels, which palette,
and how far the wind has carried the sky in metres.
It builds a headless device, renders through exactly the same pass the window does,
copies the result back and writes a PNG, timing eight frames on the way.
Every number in this piece was set by looking at one of those.
