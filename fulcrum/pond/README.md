# pond

A pond at dusk that answers the pointer slowly and asks nothing:
rings, a wake, and a lantern that walks when you leave it alone.

```sh
cargo run -p pond --release
```

The note for whoever downloads the executable is [`Windows/pond.md`](../../Windows/pond.md).
This file is for whoever opened the source:
how the piece is built and why it is built that way.

## What it is for

The brief was a game for relaxation, with no twitch in it and no goal that could produce anxiety, worked with the mouse, discoverable by playing, and beautiful if possible.
This is the answer:
a surface of water that responds to the pointer, and a light that walks across it when the pointer is still.

Three things make it restful rather than merely quiet:

- **It answers, but slowly.** Everything you do produces a visible result, which makes it worth touching, and every result takes seconds to unfold, which makes it restful.
  A ring spreads at about fifty pixels a second.
  The lantern arrives about a second after the pointer.
  There is no input the piece answers quickly.
- **Nothing can go wrong.** There is one object, the water, and every input is a disturbance of it.
  Disturbances add, so nothing you do conflicts with anything else, and every one of them is damped, so the surface always returns to level on its own.
  There is no state to get into and no state to get out of.
- **Left alone, it does something worth watching.** After ten seconds without the pointer moving, the lantern sets off on a slow walk from side to side.
  That is bilateral stimulation, the piece of EMDR that survives being taken out of a therapist's office:
  a light crossing the visual field and crossing back, at a pace the eye follows without effort.
  It is not presented as a treatment and is not one.
  It is the thing the piece does when you stop doing things, and it means the piece is not waiting on you.

## How it is built

The crate is split the way the others here are:
[`game.rs`](src/game.rs) is the simulation and knows nothing about pixels or colour, [`screen.rs`](src/screen.rs) and [`pond.wgsl`](src/pond.wgsl) are the picture, [`look.rs`](src/look.rs) is the palettes, and [`main.rs`](src/main.rs) wires them together and reads the keys.
The simulation runs headless for the tests, and the picture renders headless for the still.

### The water

The surface is a height field on a grid of cells, two pixels to a cell, run as the two-dimensional wave equation:
each cell's vertical speed is nudged by how far it sits above or below the average of its four neighbours, and its height moves by its speed.
That is the whole rule, and it makes a dent become a ring:
the dent rebounds, overshoots, and the overshoot travels outward as a crest with a trough behind it.

Three things are added to the bare equation, each for a reason you can see:

- **Damping**, so a ring fades.
  Without it the pond would fill with old rings and stay full.
- **An absorbing shore**: the outer band of cells damps harder the nearer the edge, so a ring reaching the edge is eaten rather than turned around.
  A reflecting edge fills the pond with interference within a minute, which is busy, and busy is the one thing this must not be.
- **A slow return to level**, because the wave equation conserves the mean height and a pond that has been pressed on would otherwise stay pressed.

The scheme is explicit, so it holds together while a wave travels less than a cell over root two per tick and comes apart past that.
`the_scheme_is_stable` in the tests holds the wave speed under that, and `a_ring_travels_at_the_wave_speed` checks that the constant means what it says.

### The sources

Everything you can do is a way of disturbing the surface:

- **A tap** is a dent, pressed in at once, most of the way to a small bowl.
- **A held press** pulls the surface toward a bowl shape whose radius grows over a couple of seconds.
  Toward a shape rather than by a force, so holding it does not keep deepening forever:
  the bowl has a floor.
  On release nothing is done;
  the water rebounds on its own, and a bigger bowl rebounds into a bigger ring.
- **The lantern's wake** is a push ahead of the lantern and a lift behind it, in proportion to its speed.
  The two cancel, so the net displacement is nothing and what is left is the two arms of a wake.
  A push alone leaves a trough along the whole path, which reads as a shadow the lantern drags behind it.
  That was the first version.
- **Rain**: a small dent somewhere, every five to eleven seconds.
  It keeps the pond from being quite still, and it shows what a press does without a hint having to say so.

### The lantern

The lantern is a point pulled toward a target by a critically damped spring, with a cap on its speed.
Critically damped means it neither snaps nor overshoots;
the cap means that a pointer arriving from the other side of the display does not make it flash across.
It is not placed anywhere.
It travels.

What the spring pulls toward is all that changes.
While the pointer is moving, or has moved within the last ten seconds, the target is the pointer.
After that the target is a point on a walk:
a sine across the middle of the window, with a slow rise and fall on top so it is a path rather than a rail.
The walk starts from where the lantern is, heading the way it was going, so setting off is not a jump either.
`phase_through` works out the phase for that and `a_walk_sets_off_from_where_the_lantern_is` checks it.

The pace of the walk is four seconds a cycle to begin with, which is fifteen crossings a minute, and `up` and `down` slide it between two and eight.
Clinical bilateral stimulation is faster;
this is meant to be slower.

### The picture

The height field goes to the GPU as a texture, one float a cell, in the layout the simulation keeps it in, and a fragment program does the rest at every pixel:

1. Read the heights around the pixel and get the slope, which gives a surface normal.
2. Tilt the line of sight by the normal to find the point on the **bed** the eye is looking at through the water.
   The bed is a slow gradient of the palette with a little movement in it.
3. Shift the palette position by the height:
   a crest is coloured from higher up the palette and a trough from lower down.
   This is where a ring gets its bands.
4. Add a highlight where the normal turns the light back toward the eye.
5. Add the lantern as a glow on the bed.
   Because it is on the bed and the bed is seen through the surface, a ring crossing the lantern makes it wobble.
6. Darken toward the corners, so the middle is where the eye settles.

The palettes are written as display values and the shader takes the display curve off once, at the very end, so every blend on the way is between the colours as written.
All six are analogous rather than complementary:
a narrow band of hues lets the eye rest anywhere.

`examples/pond_still.rs` renders one frame on a headless device through the same pass the window uses and writes it to a PNG.
That is how the constants in `screen.rs` were chosen:
by looking.

### What the keys do not do

Nothing on the keyboard changes the water except `r`, which flattens it.
The palette, the drift, the hint and fullscreen are matters of taste and live in `main.rs`;
the pace of the walk and the lantern's errand are simulation state and live in `game.rs`, where the determinism test can script them.

## Tests

- `tests/water.rs`:
  the scheme is stable, a dent becomes a ring and the ring dies away, a ring travels at the wave speed, the shore does not reflect, a resize keeps the water, a press is a bowl and a tap is a dent, the lantern follows without jumping, and left alone it walks and a move calls it back.
- `tests/determinism.rs`:
  same seed and same scripted input twice, bit-identical both times, including a resize part way through.
- `tests/screen.rs`:
  every uniform is in the slot the shader reads it from, and every palette climbs from dark to light.
