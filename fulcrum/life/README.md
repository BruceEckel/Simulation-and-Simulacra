# life

Conway's Game of Life, and forty-three of its relatives, from cells you could count with your
finger down to one cell per physical pixel.

This is the source note: how the piece is built and why it is built that way. What the keys do,
and what to look at, is in [`Windows/life.md`](../../Windows/life.md).

## One engine, three families

Life is one line of a table. What makes the rest of the published zoo reachable from the same
code is that they are all the same shape of rule: **count the live cells in a neighbourhood, and
let that count decide what the middle cell does next**. Four things vary.

| | |
|---|---|
| which counts give birth | `B3` for Life; `B34678` for Wanderers; a band of `34..45` for Bosco's rule |
| which counts let a cell survive | `S23` for Life; nothing at all for Seeds |
| how many states a cell has | two for Life; three for Brian's Brain; two hundred and fifty-five for Modern Art |
| how wide the neighbourhood is | eight for Life; four for von Neumann; four hundred and forty for radius ten |

Vary the first two and you have the **Life-like** rules, of which twenty-two are here. Add the
third and you have **Generations**, where a cell that fails its survival test does not die but
ages through further states, taking up room without counting as a neighbour — which is the whole
of why nothing in Brian's Brain ever holds still. Fifteen of those. Vary the fourth and you have
**Larger than Life**, where the thresholds become bands rather than sets of digits and what comes
out looks organic rather than mechanical. Seven of those.

`rules.rs` is that table, and the rulestrings in it are the published ones — from Mirek
Wojtowicz's lexicon behind [MCell](https://mcell.ca/), [LifeWiki](https://conwaylife.com/wiki/),
[Golly's Larger than Life documentation](https://golly.sourceforge.io/Help/Algorithms/Larger_than_Life.html),
and Wikipedia's table of notable Life-like rules. Nothing in it is invented.

One trap when reading them elsewhere: MCell writes a Generations rule survival-first, as
`345/2/4`; Golly and LifeWiki write it birth-first, as `B2/S345/4`. Same rule. This uses the
birth-first form throughout.

Every rule also carries **how it likes to be started**, because the right first frame is not the
same for all of them. Seeds sown at a third full detonates and is over before you have looked at
it. Gnarl wants exactly one live cell. Day & Night is unchanged by swapping live for dead, so it
wants a soup at one half — the only density that does not quietly prefer one of its two phases.

## Down to one cell a pixel

The resolution control is the reason for most of the decisions below. At its finest a cell is one
physical pixel, so a full display is several million cells and the rule is being evaluated at the
finest grain the screen can show.

**The field goes to the GPU as a texture, not as sprites.** The engine draws sprites and a grid
of tinted squares is how the other grid pieces here work, but a sprite per cell was never going
to reach millions. Two bytes a cell go up instead — what the cell is, and how long it has been
that — and a fragment program turns a pixel into a cell with one division and picks a colour.
`textureLoad` at integer coordinates, with no sampler bound at all, so a cell is one flat colour
with a hard edge at sixty-four pixels a cell and at one.

**Counting is separated from applying**, and counting is done three ways.

- *Eight neighbours*, the case that has to be fast. Reading each cell's eight neighbours directly
  touches every cell nine times. Instead each row is summed into threes once, and a cell's count
  is three of those sums added: five additions a cell, both passes walking memory forwards, and
  the inner loop written with no edge test in it so that it vectorises.
- *A wide neighbourhood*. Radius ten is four hundred and forty-one cells and counting those one
  at a time is not affordable at any resolution, so a summed-area table makes every count four
  lookups whatever the radius. It is built over a copy of the field padded by the radius, which
  is what lets the wrapped edge and the walled edge share one code path with no special cases.
- *Four neighbours*, small enough to read directly.

All three leave the middle cell in the count and `apply` takes it out again, so Larger than Life's
rules — which do want it counted — need no separate path.

**The rule is answered before the loop that asks it.** Every count it could ever be given is
looked up once into two small tables. What that gets rid of is not the arithmetic, which is a bit
test, but the branch on *which kind of rule this is*, which is the same answer every time and is
in the way of everything else.

Measured on an Intel Arc integrated GPU, 2560 × 1440 at one cell per pixel — 3,686,400 cells:

```
Life        20.8 ms a generation   (48 a second)
Bugsmovie   19.7 ms a generation   (radius ten, so 441 cells decide every cell)
the frame    0.9 ms
```

**A tick's work is bounded in cells, not in generations and not in time.** A small field runs up
to twelve generations per tick and a display-sized one runs one. A time budget would have been
easier and would have destroyed the replay: a tick that watched the clock runs a different number
of generations on a fast machine than on a slow one. The readout shows the pace asked for and the
pace achieved separately, so the difference is never hidden.

## Age and trails

Two things are kept per cell that no rule ever reads back: how long a live cell has been alive,
and how lately a dead one was alive. Neither can change what happens. They are here because they
are histories that have to be updated in step with the field, and they are in the simulation
rather than in the renderer because the renderer sees one frame at a time and these are about
time.

What they buy is a picture that shows *when* as well as *what*. Age makes a newly born cell
arrive bright and settle to the palette's own colour, which in Life means the busy edges glow and
the still lifes sit quiet — you can see where the work is being done. Trails turn a field of
noise at one cell to the pixel into something with a direction. Both are on a key, and turning
them off costs nothing: the shader is told to ignore them rather than the simulation stopping
keeping them.

## Noticing that it has stopped

Every generation is folded into a checksum as it is written, and the last thirty-two are kept. A
field that matches one of them has stopped producing anything new, and the distance back is its
period: one for a still field, two for a garden of blinkers, fifteen for a pulsar. That is what
the readout means by *still* or *period 2*, and it is what the self-restart watches. A glider
crossing a torus is not caught until it has come all the way round, which is right — it really
has not settled.

## Colour

Six colours to a scheme and twelve schemes, in `look.rs`. Nothing here is a light value: these
are the numbers that land on the screen, and a cell is in a state and a state is a colour.

They are written as **display** values, the numbers you would type into a paint program, and the
sRGB curve is taken off at the very end of the shader, on the one colour it arrived at. Doing it
there rather than to the palette on the way in matters: it means every blend between two of these
colours happens between the numbers as written, so a ramp from a red to a near-black passes
through the dark reds a person expects rather than through the ones that adding light would give.

## Fullscreen, and why the field survives it

`F11` calls `set_fullscreen` on the window and does nothing else. The event loop keeps running,
the fixed tick keeps firing, and the only thing the simulation ever hears about it is the resize
that arrives a moment later on the replayable command channel — the same message a dragged window
edge sends.

There is one thing on the drawing side that a resize has to be careful about, and it caught all
eight of the pieces here that compute their own pixels. The frame a pass draws into cannot be
rebuilt behind the same asset handle: the engine's sprite renderer caches one bind group per
texture id and builds it once, so a replacement is never noticed and the picture freezes on the
last frame before the resize while the simulation carries on underneath. That is
[`simulacra-frame`](../../crates/simulacra-frame)'s whole reason for existing, and the long
version of the explanation is in its module documentation.

The field is then reshaped, and the pattern on it is **copied across with its middle on the new
middle, and not scaled**. That is the only honest answer. A glider blown up by a factor of six is
a six-by-six blob and is no longer a glider, so a resample would quietly destroy the thing you
were watching in order to keep it the same size on the glass. Going finer therefore leaves the
pattern where it was, at its own size, with more field around it; `R` fills the new room.

## Tests

`tests/life.rs` is in two halves. The first checks the famous facts: a block sits still, a
blinker has period two, a glider is one cell down and one right after four generations, an
R-pentomino is still busy at a thousand, a diehard is gone at exactly a hundred and thirty,
Gosper's gun has fired four gliders by a hundred and twenty. If any of those come out wrong then
whatever this is, it is not Life.

The second half is the one that matters more. The fast counters are the kind of code that can be
wrong in a way that still looks plausible on screen, so **every rule in the table, on both
boundaries, is run against a naive count written the way the definition reads**, and the two must
agree cell for cell for three generations. A symmetric soup is also run under every rule and must
stay symmetric, which catches an asymmetry in an edge case at once.

`tests/determinism.rs` drives the whole of it headless — the rules, the resolution, the pace, the
mouse, the sowing, the boundary, and three window resizes — twice, and the digest covers every
cell and both of its histories.

## Looking at it without a window

```sh
cargo run -p life --release --example life_still -- still.png 2560 1440 0 0 0 400 3
```

The arguments are the file, the size, which rule, which cell size, which palette, how many
generations to run first, and how the field is read as three bits. It renders through exactly the
same pass the window does, on a device with no window attached, and times both halves on the way,
so it is also where the numbers above came from.
