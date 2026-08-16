# Nimbus

Real-time volumetric clouds over a desert.
The same sky as [`thunderhead`](../thunderhead/README.md), by the opposite method:
nothing is drawn in advance and nothing is a picture.
There is a cloud *volume*, and every frame a few hundred thousand rays are walked through it.

```
cargo run -p nimbus --release
```

The window opens borderless over the whole display.
`F11` puts it back in a window.

The five pieces are the same sky by five different methods:
[`thunderhead`](../thunderhead/README.md) grows cloud bitmaps on the CPU and blits them,
[`nimbus`](../nimbus/README.md) raymarches a real volume on the GPU,
[`ligne`](../ligne/README.md) evaluates a two-dimensional field per pixel and draws it with a line,
[`parrish`](../parrish/README.md) glazes that same field the way Maxfield Parrish glazed a sky,
and [`moebius`](../moebius/README.md) computes no cloud at all: it draws overlapping circles and keeps the arcs.

## What is actually happening

For every pixel, a ray leaves the eye and crosses a shell of air wrapped around a planet,
between 1500 and 5400 metres up.
It takes a few dozen steps through that shell.
At each step it asks how much cloud is at that point,
and where the answer is more than nothing it takes a *second*, shorter walk towards the sun
to find out how much cloud is in the way of the light.
What comes back is added up front to back, with the transmittance falling as it goes,
and when the transmittance runs out the ray stops early because nothing behind it can be seen.

So there is no cloud object anywhere in the program.
There is a function of a point in the sky, and everything you see is that function,
integrated along a line, several hundred thousand times a frame.

## The volume

The function is two three-dimensional noise textures, built on the CPU at startup and never touched again:

- **shape**, 128³, holds a perlin-worley field that decides where clouds *are*,
  plus three frequencies of inverted worley that erode it.
- **detail**, 32³, is sampled twenty times finer and eats into the edges,
  which is what turns a lump into cauliflower.

Both tile exactly, because the lattice they are built on wraps.
That is not a nicety: they are sampled with repeat addressing over tens of kilometres of sky,
so a seam would show up as a ruled line across the weather every few hundred metres.
`tests/noise.rs` holds them to it.

Two decisions in there earned themselves:

- **Every channel is stretched over the byte it is stored in.**
  Both fields come out of the machinery bunched: the perlin-worley mixture lives between about 0.64 and 0.87.
  Stored raw it would use sixty of its two hundred and fifty-six available values,
  which is four times the quantisation it needs.
  Worse, coverage is a *threshold against these numbers*,
  so with the field bunched at the top the sky went from clear to solid over a few hundredths of a turn of the dial.
  The first version of this piece rendered a completely overcast sky and I read it as a bug in the march;
  it was a bug in the histogram, and the test that prints the percentiles found it in one run.
- **The volume is squeezed vertically when it is read.**
  It is isotropic, and the layer it is read through is four kilometres deep and tens of kilometres wide,
  so read straight it gives features as wide as they are tall, which the vertical profile then flattens into pancakes.

## The march

Two speeds.
Most of a sky is empty air, and a march that samples empty air at the resolution it samples cloud
spends nine tenths of its budget establishing that there is nothing there.
So it strides until it touches something, backs up one stride, and creeps;
after a few fine steps in clear air it goes back to striding.
Same picture, about a third of the samples.

The step length has a floor and a ceiling.
Near the horizon a ray is inside the layer for seventy kilometres,
and dividing that by the step budget gives half-kilometre steps that walk straight through whole clouds.
The ceiling trades reaching the far end for not doing that, and the haze covers the rest.

Where a ray starts inside the layer is dithered per pixel with interleaved gradient noise.
An ordered four-by-four table was the first thing here and it wrote its own weave across every cloud:
sixteen offsets repeating on a grid is a pattern, and a pattern in the sampling is a pattern in the picture.
The dither is fixed per pixel rather than animated, because animated noise needs somewhere to accumulate
and there is no history buffer in this piece.

## The light

Sunlight through cloud is not one exponential.
One octave of Beer's law gives a cloud whose inside is black,
because single scattering is not how light gets into a cloud.
Each of the three octaves here absorbs less and scatters wider than the last,
which is a cheap stand-in for light that has bounced its way in,
and it is the difference between a storm cloud and a lump of coal.

Two things about the phase function are worth writing down, since both of them were bugs first:

- Henyey-Greenstein is written here **without** the `1 / 4π` that normalises it over the sphere.
  Normalised, it is a probability density that averages to one,
  and multiplying a sunlight radiance by it leaves every cloud not directly in front of the sun
  lit at about a fiftieth of what it should be: a sky of flat grey lumps.
- The octaves are **divided by their own weights**,
  so a sample with nothing between it and the sun is lit by one sun rather than by one and three quarters.
  Without that, every thin cloud in the sky comes back overexposed and the whole field goes white.

Above the cloud layer there is a sky gradient and a sun;
below it there is flat sand, lit by how high the sun is and shadowed by four samples along the ray towards it,
so the desert darkens and clears again as a cloud goes over.
Under a low sun the sand goes dark while the clouds over it stay lit,
which is correct: they are catching the same light side-on.

## The flat-colour finish

`B` steps the whole picture into six tones and draws a line where they meet.
That is the other piece in this repo done to a volume instead of to a bitmap,
and it is the reason the renderer is two passes rather than one:
the march runs at whatever resolution the machine can afford,
and the finish runs at the size of the window, where the ink belongs.

The line is a contour of the volume rather than an outline of a sprite,
so it moves as the cloud boils.

## Why the numbers are what they are

- **The march's resolution is a pixel budget, not a fraction.**
  A fraction that is comfortable on a laptop panel is four times too expensive on the wall-sized one next to it.
  The default aims at 850,000 rays, which a modest integrated part can walk inside a frame with room to spare;
  the number keys take the decision off the piece and give it to the viewer,
  which is the honest way round, since how many rays a frame can afford is a fact about their machine and not about their sky.
- **The layer is wrapped around a planet of the real radius.**
  A flat slab of cloud has a hard edge at the horizon.
  A curved one runs away into the haze the way a real sky does.
- **The clouds are as tall as they are wide.**
  A density that climbs steeply with height squeezes everything the coverage test can find into a thin band,
  and a thin band of cloud seen from underneath is a field of pancakes.
- **Distant clouds dissolve rather than shrink.**
  Their light goes to the colour of the air and their silhouette opens up,
  so a cloud thirty kilometres off joins the horizon instead of standing there
  as a hard little shape with everything the near ones have.
- **A palette carries where the sun is.**
  The colour of the light and the angle it arrives at are one decision:
  a low sun that is not warm looks broken, and a warm sun overhead looks like a mistake.

## Working it

| | |
|---|---|
| `up` / `down` | the pace of the wind |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `P` | palette: arzach, noon, monsoon, nocturne, mineral |
| `B` | step the picture into flat colour and ink it |
| `1`–`5` | how many rays the march runs |
| `H` | hide the readout |
| `F11` | leave fullscreen |

## Seeing it without a window

```sh
cargo run -p nimbus --release --example nimbus_still -- still.png 1920 1200 0 70 0 0
```

The arguments are the file, the size in pixels, which palette, the internal scale as a percentage,
whether to finish in flat bands, and how far the wind has carried the weather.

It builds a headless device, renders through exactly the same two passes the window does,
copies the result back across the bus and writes a PNG.
It also times eight frames and prints the milliseconds, so it doubles as the benchmark.

This is not a convenience.
A shader is the one part of a program you cannot read your way to correctness in:
the only question worth asking about a cloud is what it looks like, and the answer is a picture.
Every number in this piece was set by looking at one.
