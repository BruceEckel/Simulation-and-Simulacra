# nimbus

Real-time volumetric clouds over a desert: a raymarched cloud layer, lit by a marched sun.

## The idea

Nothing here is drawn in advance and nothing is a picture. There is a cloud *volume*, and every
frame a few hundred thousand rays are walked through it.

For every pixel, a ray leaves the eye and crosses a shell of air wrapped around a planet,
between 1500 and 5400 metres up. It takes a few dozen steps through that shell. At each step it
asks how much cloud is at that point, and where the answer is more than nothing it takes a
second, shorter walk towards the sun to find out how much cloud is in the way of the light. What
comes back is added up front to back, and when the ray has run out of light to gather it stops
early, because nothing behind it can be seen.

So there is no cloud object anywhere in the program. There is a function of a point in the sky,
and everything you are looking at is that function integrated along a line, several hundred
thousand times a frame.

## What makes it look like weather

The function is two three-dimensional noise volumes, built when the program starts and never
touched again: a large one that decides where clouds are, and a small one sampled twenty times
finer that eats into their edges and turns a lump into cauliflower. On top of those sits a
weather map, read flat and very wide, which gives the sky districts: places where the coverage
runs high and the tops tower, and places where it stays thin, with the wind carrying those
districts past as well.

Sunlight through cloud is not one exponential. One octave of Beer's law gives a cloud whose
inside is black, because single scattering is not how light gets into a cloud. Three octaves,
each absorbing less and scattering wider than the last, stand in for light that has bounced its
way in, and that is the difference between a storm cloud and a lump of coal.

Under the layer there is flat sand, lit by how high the sun is and shadowed by a short walk
along the ray towards it, so the desert darkens and clears again as a cloud goes over. Under a
low sun the sand goes dark while the clouds above it stay lit, which is right: they are catching
the same light side-on.

## Flat colour

`B` steps the whole picture into six tones and draws a line where they meet, which is
`thunderhead.exe` done to a volume instead of to a bitmap. The line is a contour of the volume
rather than an outline of a sprite, so it moves as the cloud boils.

## Working it

It opens fullscreen and takes the entire display. `F11` gives a normal window back.

| | |
|---|---|
| `up` / `down` | the pace of the wind |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `P` | palette: arzach, noon, monsoon, nocturne, mineral |
| `B` | step the picture into flat colour and ink it |
| `1`-`5` | how many rays the march runs |
| `H` | hide the readout |
| `F11` | leave fullscreen |

The march is the whole cost of the piece and it is paid per pixel, so by default it aims at
about 850,000 rays whatever the size of the display and lets the finishing pass fill the window.
The number keys take that decision off the program and give it to you, which is the honest way
round: how many rays a frame can afford is a fact about your machine and not about your sky. The
readout shows how many are being walked and how long a frame is taking.

There are two companion pieces with the same sky by other methods: `thunderhead.exe` grows cloud
bitmaps and blits them, and `ligne.exe` evaluates a two-dimensional field per pixel and draws
it.

Source, and the long version of the explanation, in [`fulcrum/nimbus`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/nimbus).
