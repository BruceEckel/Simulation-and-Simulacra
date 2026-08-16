# Thunderhead

A flat desert under enormous drifting thunderheads, drawn one physical pixel at a time,
in flat colour with a line around everything.

```
cargo run -p thunderhead --release
```

The window opens borderless over the whole display.
`F11` puts it back in a window.

The five pieces are the same sky by five different methods:
[`thunderhead`](../thunderhead/README.md) grows cloud bitmaps on the CPU and blits them,
[`nimbus`](../nimbus/README.md) raymarches a real volume on the GPU,
[`ligne`](../ligne/README.md) evaluates a two-dimensional field per pixel and draws it with a line,
[`parrish`](../parrish/README.md) glazes that same field the way Maxfield Parrish glazed a sky,
and [`moebius`](../moebius/README.md) computes no cloud at all: it draws overlapping circles and keeps the arcs.

## The idea

Moebius drew weather the way nobody else did:
huge clouds built out of flat areas of colour,
each area closed by a clean unvarying line,
no gradients and no texture anywhere.
That is a drawing technique, but it is also a *data structure*,
and this piece is what happens when you take it literally.

Every pixel of the window holds one byte,
and that byte is not a colour.
It is a **material**: a band of sky, the sun, a band of sand,
that same band with a cloud's shadow across it,
rock on the horizon, a band of cloud at one of three distances, or ink.
There are sixty-six of them.
The picture is composed entirely in those bytes,
and only at the last moment does every byte go through a sixty-six entry table
to become a colour on the screen.

Two things fall out of that, and both of them are the point:

- **The drawing is flat by construction.** There is no shading in the output,
  because there is nothing in the output that *can* shade.
  A region is one material, so it is one colour, so it has a hard edge.
- **A palette is sixty-six colours.** Pressing `P` recolours a four-million-pixel picture
  in the time it takes to fill a small array. Five palettes are on offer.

## How a cloud is grown

Each cloud is a bitmap of its own, grown once in `Forge` and then only ever moved.
Four steps:

1. **Anatomy.** A few dozen ellipses are laid out as a real cumulonimbus:
   a flat base, a bank of lobes sitting on it,
   a tower boiling up and leaning downwind as it climbs,
   shoulders where the tower is fattest,
   and at the top the anvil, spreading sideways under the ceiling it cannot pass.
   Then each lobe sprouts smaller lobes around its rim, mostly upwards, and some of those sprout again.
2. **Union, not sum.** The lobes are combined by taking the *larger* of two values rather than by adding them.
   This is the single decision that makes the result read as a drawn cloud instead of a rock.
   Added together, two overlapping lobes bulge where they meet
   and the outline of the pile comes out as one smooth blob.
   Taken larger-of, the outline is the union of the lobes and stays a chain of arcs,
   which is how a cloud is drawn with a pen.
   A lobe buried inside a bigger one leaves no mark at all,
   so the inside of the cloud stays calm however much detail is heaped onto its edge.
3. **Light, then steps.** The field is read as a height map and lit off its own slope,
   measured across a span that scales with the cloud so a far one is shaded like a near one,
   plus a straight vertical term for the fact that a thunderhead's own bulk darkens its underside.
   The result is quantised into ten bands.
   Quantising is what turns lighting into drawing:
   every band boundary is a contour of the lobes, with a flat colour between one and the next.
4. **Ink.** A pass over the bands draws the line:
   two or three texels around the silhouette, one along every third contour inside it.
   Every third, because every one is lace.

Growing a cloud costs some ten million texel writes,
which is a dropped frame if it is done between two of them.
So it is done on an allowance instead: a budget of texels per tick,
stopping wherever it runs out and picking up on the next tick.
The cloud being replaced stays on screen and intact the whole time,
because the new one is grown in a buffer of its own and swapped in only when it is finished.
A cloud that has drifted off the left edge comes back in on the right wearing a new shape,
and it has its own width to cross before any of it shows,
which is far longer than growing one takes.
So the sky never repeats and never stutters.

## Why the numbers are what they are

- **One cell per physical pixel.** The clean line is the whole style,
  and a line is only clean if it is drawn at the resolution it is displayed at.
  Nothing in the piece is scaled or resampled:
  a cloud's bitmap is blitted one texel to one pixel,
  so the ink stays two pixels wide on a laptop panel and two pixels wide on a wall.
- **Clouds are measured in skies, not in pixels.** A cloud's height is a fraction of the height of the sky above the horizon,
  resolved when the window reports its size.
  A fixed pixel size would make "enormous" mean *enormous on a 1080p panel* and *modest on a 4K one*.
  When the window changes shape, every cloud goes back into the queue and is regrown at the new size,
  a tenth of a second each, while the old ones keep sailing.
- **The horizon sits at 76% of the frame.** The piece is about what is above it.
- **The clouds are fitted to their bitmaps after they are laid out.**
  Nothing in the anatomy above knows how far the puffs will actually reach,
  so the finished cloud is measured, then moved and scaled once to stand inside its bitmap with its base on the floor.
  Without that step an anvil spreads past the edge and comes back sawn off square,
  which is exactly what the first version did.
- **Three distances, and everything about a tier moves together.**
  Nearer clouds are bigger, faster, ride higher, are drawn with a heavier line,
  and cast their shadow further down the sand.
  On top of that each tier is faded into the haze by a fixed amount:
  three tiers of cloud on one flat sky read as three clouds at one distance,
  and the same three hazed read as weather going back for miles.
- **The desert is drawn once.** The viewer stands still and only the weather moves,
  so the sky's bands, the sun, the rock on the horizon, the dune contours and the stones
  are drawn when the window changes shape and never again.
  Every tick copies that and lays the moving parts over it,
  which is why a picture this size costs a few million byte writes a frame instead of a few hundred million.
- **The shadows are not projections.** The piece has no third dimension to project from.
  A cloud's shadow is its own underside, squashed flat, tapered at both ends and dragged away from the sun.
  That is enough to make the desert darken and clear again as a thunderhead goes over,
  which is the thing worth having.

## Working it

| | |
|---|---|
| `up` / `down` | the pace, from a twentieth of the drift to twenty-four times it |
| `Space` | hold the sky still |
| `P` | palette: arzach, verdigris, ember, nocturne, mineral |
| `H` | hide the readout |
| `F11` | leave fullscreen |

## Seeing it without a window

The picture is decided in plain code, so it can be drawn with no GPU anywhere near it:

```sh
cargo run -p thunderhead --release --example thunderhead_still -- still.png 2560 1600 0 7
```

The arguments are the file, the size in pixels, which palette, and the seed.
This is the tool the look was tuned with,
and it works for the same reason the tests do:
`game.rs` composes a field of materials, `look.rs` says what a material is,
and the window is only ever the thing that shows the answer.
