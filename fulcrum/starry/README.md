# Starry Night

Van Gogh's Starry Night as a fluid: a few thousand brushstrokes carried by the current that drew
them.

The sky turns, the cypress sways, the village holds still. Drag the pointer through the paint
and it smears the way wet oil does, then finds its way back into the picture over the next few
seconds. Click and a new star appears in the sky, and the paint grows a halo into it one stroke
at a time.

Nothing is a copy of the painting. The composition, the swirls, the eleven stars and the moon
are all written down as functions, and everything you see is strokes obeying them.

## Starting it

```
cargo run -p starry --release
```

Release is worth it: there are seven thousand strokes on the canvas by default and you can ask
for more.

The canvas keeps the painting's proportions whatever shape the window is, so a wide window gets
bars rather than a stretched composition.

## Doing things to it

| | |
| --- | --- |
| drag the pointer | smear the paint along, like a palette knife |
| click the sky | hang a new star there |
| `x` | take the last star down |
| `h` | stop the paint healing, and leave your smears where they are |
| `c` | change the palette |
| `n` / `m` | more paint, less paint |
| `r` | lay the whole canvas down again |
| `space` | hold everything still, and it is a painting again |
| `up` / `down` / `0` | slower, faster, back to normal |

The hint at the bottom fades away after a few seconds and comes back whenever you touch
anything.

## How it works

**One scalar field is both the picture and the motion.** The sky comes from a stream function:
the great double swirl, a few long waves, and one small vortex per star. The brushstrokes are
carried along its contours, and the sky's light and dark bands are read off its value. Van Gogh
drew his sky as flow lines, so making the flow lines the drawing is not a trick, it is the same
statement twice. Move a swirl and the bands move with it, because they were never two things.

**The current is the curl of that field.** Velocity is `(dpsi/dy, -dpsi/dx)`, which is
divergence-free: no sources, no sinks, so the canvas cannot thin out in one place and pile up in
another however long it runs. There is a test that measures this.

**The picture is a function of position, and the paint remembers it.** `paint_at` answers, for
any point on the canvas, what belongs there: cypress, village, hill, halo, sky, and where in
that layer's range of colour it sits. Every stroke carries its own colour and drifts slowly back
toward what the point it is standing on ought to be. That one rule gives the piece its whole
character:

- smear the sky and it flows, holds the smear for a moment, and then recovers
- hang a star and its halo grows into the paint rather than appearing
- turn healing off and the canvas keeps whatever you did to it

**Only the sky is free.** Strokes below the skyline are on springs to where they were laid down,
so the village stays a village and the cypress sways like a flame instead of blowing away. The
stream function is faded to nothing at ground level, which keeps the current divergence-free
while it dies: the curl of anything at all is still divergence-free.

**No stroke is quite the colour it should be.** Each one keeps a fixed offset from what the
picture asks for, through every healing. Without that, every stroke in a patch is the same
colour and the patch is a wash; with it, the canvas has the weave of strokes of noticeably
different blues laid side by side, which is most of what the eye reads as paint.

## The palettes

The simulation never picks a colour. A stroke knows only its layer and its tone, and the binary
decides what cobalt is, which is why the same painting can be repainted at dawn or in ink
without a stroke changing its mind. `c` cycles: **night** (the painting), **dawn**, **ink**,
**fauve**.

## The brushes

The three textures are generated, not drawn:

```
python3 tools/gen_starry_art.py
```

The stroke is the important one. It is thick where the brush lands, curved, tapered to a tail,
and grooved along its length by the bristles. A field of ellipses reads as confetti; a field of
these reads as paint.

## Tests

```
cargo test -p starry
```

`tests/determinism.rs` is the gate: same seed and same input twice, bit-identical both times.
`tests/painting.rs` holds the rules to their promises, including that the current never piles
paint up, that the picture has all of its parts (cypress, village, lit windows, a crescent, rings
around the stars), that the sky is mostly dark, that a drag pushes paint and a ruined canvas
heals back to the picture, and that the village stays where it was put while the sky turns.
