# fractal

Ten fractals, two families, and a progressive viewer to zoom into them with.

## The idea

Press `1` through `0` to pick one, or `Tab` to walk along the row.

The first five (Mandelbrot, Julia, Burning Ship, Newton, Phoenix) are **escape-time** fractals.
Every point of the plane is fed to a rule over and over, and the picture is how long each one
took to run away. The last five (Barnsley fern, Sierpinski triangle, dragon curve, fractal tree,
Koch snowflake) are **chaos games**: a handful of affine maps applied in a random order to a
single wandering point, which lands on the shape and nowhere else. Two quite different things
are both called fractals, which is most of the reason they are in the same program.

Both families draw progressively under a fixed per-tick budget, sampling one cell in eight
first, then one in four, one in two, and finally every cell. So you see the whole picture
immediately and it sharpens over the next few ticks rather than arriving a stripe at a time. The
budget counts iterations rather than watching the clock, which is what makes a hard view cost
sharpness instead of frame rate.

Deeper in, raise the iteration limit with `E`. What looks like solid black at 220 iterations is
usually not solid at 2000. The zoom stops at about 2·10⁻¹³ across, which is where `f64` runs out
of mantissa.

## Working it

| | |
|---|---|
| `1`-`0`, `Tab` | pick a fractal |
| arrows | pan |
| `Z` / `X` | zoom in and out |
| wheel or left click | zoom in at the pointer; right click zooms out |
| `Q` / `E` | fewer or more iterations |
| `R` | back to where this fractal started |
| `Space` | hold everything still |
| `P` | palette (seven of them, each a ring rather than a ramp) |
| `C` | stop and start the colours cycling |
| `H` | put the readout away |

Zooming at the pointer keeps whatever is under it under it, so you can chase a filament down
without losing it. Leaving a fractal remembers where you were.

Source, and the long version, in [`fulcrum/fractal`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/fractal).
