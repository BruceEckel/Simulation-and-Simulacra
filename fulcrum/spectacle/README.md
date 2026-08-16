# Spectacle

A fireworks show over dark water, put on for no reason but the watching.

Shells go up from the far shore, break at the top of their climb, and come down as light on the
water. There is a town along the horizon with its windows lit, a breeze that changes its mind
every minute or so, and a programme that runs from a sparse overture through a cascade to a
finale, then quiets down and begins again. It never repeats and it never ends.

There is nothing to win and nothing to lose. You can leave it running in the corner of a
screen, or you can sit in front of it and fire shells yourself.

## Starting it

```
cargo run -p spectacle --release
```

Release is worth it here: a finale puts tens of thousands of burning stars in the sky at once.

A window opens and the show starts after a beat of dark. Close the window when you have had
enough. You can resize the window to any shape, including full screen, and the shore, the
water and the sky all follow it.

## Watching it

Everything happens on its own. The keys are there if you want them.

| | |
| --- | --- |
| click the sky | fire a shell that breaks where you pointed |
| `f` | bring the finale forward |
| `c` | change the colors |
| `m` | turn the noise off and on |
| `space` | hold everything still |
| `up` / `down` | slow it down, speed it up |
| `0` | back to normal speed |

The hint at the bottom fades away after a few seconds and comes back whenever you touch
anything.

## The programme

The show is a round of five passages, and it repeats:

**Overture.** Single shells with room around them, mostly plain spheres. This is where the eye
learns how big a firework is and how long it takes to fall.

**Rise.** More of them, closer together, bigger, and the whole repertoire comes out.

**Cascade.** Breaks overlapping breaks, with salutes and mines mixed in.

**Hush.** Almost nothing: a willow or two, and the smoke clearing.

**Finale.** Everything at once, until it stops.

The hush is the part that is easy to leave out and the one that matters most. Fireworks at a
constant rate stop registering after a minute, and the quiet before the finale is what makes
the finale land.

## What is in the show

Nine families, and they differ in physics rather than in decoration, because that is how the
real ones differ:

- **peony** the plain sphere of stars, and the shape everything else is a variation on
- **chrysanthemum** a peony whose every star draws a tail
- **willow** heavy stars with low drag: they hang, then droop most of the way to the water
- **ring** a circle seen at an angle, so it reads as a hoop rather than as a drawn O
- **palm** a handful of thick fronds thrown from a common center
- **crossette** stars that fly out, pause, and split into small crosses
- **salute** a hard white flash and a crack, with almost nothing left to look at
- **strobe** a slow cloud of stars blinking out of step with each other
- **mine** not a break at all: a fan of stars fired straight off the water

## The bang arrives late

Sound is scheduled rather than played. A break puts its report on a queue with a delay of the
distance divided by the speed of sound, and the queue lets it go when the wave would have
reached the shore. A shell that breaks high and far off flashes about a second before you hear
it, and the boom that arrives is quieter and lower than one overhead.

That one delay does more for the sense of distance than anything in the picture. It is also the
reason the show feels like it is happening somewhere rather than on a screen.

## Under it

The simulation in `src/game.rs` is pure logic: no sprites, no sound, no reading of anything the
renderer owns. It runs headless, which is what the determinism test drives. The binary in
`src/main.rs` paints it and turns the queued reports into noise.

The art and the sounds are generated, not drawn or recorded:

```
python3 tools/gen_spectacle_art.py
```

That writes the four glows and the four sounds into `assets/`. The sounds are synthesised from
noise and one filter, because a firework is not a pitched instrument: it is a pressure step
followed by decaying noise.

## Tests

```
cargo test -p spectacle
```

`tests/determinism.rs` is the gate: the same seed and the same input twice, bit-identical both
times. `tests/show.rs` holds the rules to their promises, including that a shell breaks at the
height it was aimed at, that the sound never arrives before the light, and that the sky stays
inside its budget through a finale.
