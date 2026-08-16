# Fractal

Ten fractals, and as far into any of them as double precision will take you.

```
cargo run -p fractal --release
```

Build it in release. A debug build works and is roughly ten times slower, which turns a
picture that arrives in a twelfth of a second into one you watch arrive.

## What you get

Press `1` through `0` to pick one, or `Tab` to walk along the row.

| | | |
|---|---|---|
| `1` | **Mandelbrot** | Square, add, repeat. The black part is every `c` whose orbit never runs away. |
| `2` | **Julia** | The same rule with `c` held still and the plane as the starting point. `c` drifts. |
| `3` | **Burning Ship** | Mandelbrot's rule with both signs thrown away before each squaring. |
| `4` | **Newton** | Root-hunting on `z³ = 1`, colored by which of the three roots each point falls into. |
| `5` | **Phoenix** | A squaring with one step of memory, which folds it into wings. |
| `6` | **Barnsley fern** | Four affine maps. The stem is the one picked once in a hundred. |
| `7` | **Sierpinski triangle** | Jump halfway to a corner chosen at random, forever. |
| `8` | **Dragon curve** | Two maps, each a shrink and a half turn. A paper strip folded over and over. |
| `9` | **Fractal tree** | A trunk, two boughs 45 degrees apart, and a bud. |
| `0` | **Koch snowflake** | The Koch curve, on each of three sides, bulging outward. |

The first five are **escape-time** fractals. Every point of the plane gets fed to a rule over
and over, and the picture is how long each one took to run away. The last five are **chaos
games**: a handful of affine maps, applied in a random order to a single wandering point, which
lands on the shape and nowhere else. Two quite different things both called fractals, which is
most of the reason they are in the same program.

## Getting about

| | |
|---|---|
| arrows | pan |
| `Z` / `X` | zoom in and out |
| wheel, left click | zoom in at the pointer |
| right click | zoom out at the pointer |
| `Q` / `E` | fewer or more iterations |
| `R` | back to where this fractal started |
| `Space` | hold everything still |
| `H` | put the readout away |

Zooming with the wheel or a click keeps whatever is under the pointer under the pointer, so you
can chase a filament down without losing it. Leaving a fractal remembers where you were, so
wandering off to look at the fern does not cost you the corner you had found.

Deeper in, raise the iteration limit with `E`. What looks like solid black at 220 iterations is
usually not solid at 2000 — points near the boundary take arbitrarily long to escape, and the
limit is the only reason they were called members in the first place.

The zoom stops at about 2·10⁻¹³ across. That is where `f64` runs out of mantissa, and past it
the blockiness would be real rather than temporary.

## Colors

| | |
|---|---|
| `P` | next palette |
| `C` | stop and start the cycling |

Seven palettes: Nebula, Aurora, Ember, Spectrum, Ice, Candy, Peacock. Each is a **ring** rather
than a ramp, and that is not decoration. An escape count has no top, so a palette with two ends
would spend the whole interesting part of every picture parked against one of them. A ring never
runs out, which is where the bands come from.

Two other things are going on in the color:

- **The bands are spread by a square root.** Escape counts pile up towards the boundary — that
  is what a boundary is — so plotting them straight would cram every band but the first into the
  last pixel before the edge.
- **The interior is not empty.** A point that never escapes still has an orbit, and what is drawn
  is how close that orbit ever came to the origin. That is where the filaments and whorls inside
  the black come from. Points whose orbit passes near zero, like most of the period-2 bulb, come
  out genuinely dark; that is the measurement, not a gap in it.

Cycling turns the ring under a finished picture. It costs nothing — it changes how the samples
are colored, not what they are — so a shimmering picture recomputes exactly as often as a still
one, which is never.

## Watching the Julia set come apart

`2` opens on `c = -0.7885`, which is inside the Mandelbrot set, and the Julia set there is one
connected piece with a real interior. `c` then drifts slowly around the circle of radius 0.7885,
and almost all of that circle is *outside* the Mandelbrot set. Watch what happens as it leaves:
the set thins to a dendrite and then to dust — infinitely many points, no interior, nothing
joined to anything.

That is the whole relationship between the two sets, happening in front of you. The Mandelbrot
set is exactly the catalogue of constants whose Julia set holds together. `Space` stops the drift
wherever you like it.

## How it draws

The renderer draws sprites, not textures, so an escape-time picture is a grid of about 26,000
tinted squares and a cloud is 18,000 tinted specks. That is the sprite budget, and it is why the
grid is a few hundred cells across rather than a few thousand.

Both families draw **progressively under a fixed per-tick budget**. A fresh view is sampled at
one cell in eight first, then one in four, one in two, and finally every cell — so you see the
whole picture immediately and it sharpens over the next few ticks rather than arriving a stripe
at a time. Pan or zoom while it is working and it simply starts again, which is why moving about
shows a coarse picture and stopping sharpens it. The readout says which pass it is on.

The budget counts iterations rather than watching the clock. That is what makes a hard view cost
sharpness instead of frame rate, and it is also what keeps the whole thing replayable: the same
tick always does exactly the same amount of work, on any machine. The chaos games run under the
same arrangement, keeping only the points that land on screen — which is what makes an attractor
zoomable at all, since the walk visits all of it regardless and a narrow view simply takes longer
to fill.

Every home view in here was measured rather than guessed. A fractal viewer fails quietly: point
it at empty space and you get a perfectly valid picture of nothing.

## Tests

```
cargo test -p fractal --release
```

`tests/fractals.rs` checks the mathematics against things that are true independently of this
program — the Mandelbrot cardioid reaching exactly ¼ and the set stopping at -2, Newton finding
all three roots, a Julia set being connected exactly when its constant is in the Mandelbrot set,
more iterations only ever shrinking a set and never growing it — and then checks that every
fractal's opening view actually has the fractal in it. It also checks that the progressive passes
converge on precisely the picture a plain one-cell-at-a-time renderer would have produced.

`tests/determinism.rs` is the replay gate. The chaos games draw from the simulation RNG, so the
fern is only the same fern twice if the draws are.
