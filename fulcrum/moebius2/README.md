# Moebius 2

The same construction as [`moebius`](../moebius/README.md), with three of its fixed decisions put on keys:
the weight of the line around a cloud, the number of arcs an element is built from, and twenty palettes instead of five.
[`moebius3`](../moebius3/README.md) carries on from here: two dozen arcs to an element, hatching on the shaded side, and a man on a horse crossing the desert.

```
cargo run -p moebius2 --release
```

The window opens borderless over the whole display.
`F11` puts it back in a window.

## What is the same

Everything the drawing is made of.
Overlapping circles are laid down in a row; the edge of their union is the outline, so the outline is made of circular arcs meeting at cusps.
More circles go on top, and where a later one covers an earlier one the earlier arc is not there any more.
Each cloud is a **mass** of big circles along a spine, a **fringe** of small ones walked around the upper rim, and a few **crests** laid over the finished body afterwards.
Nothing is shaded anywhere, an enclosed region is one flat colour, three bands of cloud are drawn far to near, and the whole sky is a function of one number.
The first version's README explains all of that at length and none of it has changed.

## The line

Every line in the first version is one weight, and that was the style.
Here the clouds keep their own weight and everything else keeps the old one.
`Z` and `X` move it between six tenths of a pixel and seven, and they run while held rather than stepping,
because the useful range is a couple of pixels wide and the difference worth seeing is a tenth of one.

The rest of the panel is left alone on purpose.
The sky's bands, the desert's, the horizon, the rock and the ring around the sun are the paper the drawing stands on,
and thickening them along with the clouds changes the paper rather than the drawing.
At six pixels the clouds read as a woodcut against a sky that is still ruled with a fine pen, and that contrast is the whole reason the setting is worth having.

The line is centred on the edge of a shape, so widening it moves the outline outwards as much as inwards.
A fat setting does not eat the fill: the cloud keeps its size and grows a heavier border.

## No element is a circle

A crest that falls inside a cloud closes on itself, and in the first version it closed as a circle,
because a crest was one circle and the outline of one circle is a circle.
A circle sitting inside a cloud reads as a hole punched in the paper rather than as a second cloud sitting on the first.

So an element here is never one circle.
A crest is a union of two to six, and `N` and `M` set which.
The extra arcs are placed against the main circle at a distance worked out from the two radii:
any closer and one swallows the other, any further and they come apart, and anywhere between the two is a union with a cusp in it.
That is a guarantee rather than a tendency, so no setting and no moment in a cloud's life can produce a bubble.

Two more places needed the same treatment, and neither was obvious until the tests went looking.

A body is a union too, and for the first seconds of a cloud's life it is one circle:
every lobe arrives on its own schedule, the middle one arrives first, and it stands there at full size while the rest of the mass is still on its way.
The middle lobe now carries the same arcs a crest does, for the whole of the cloud's life,
so a cloud is several overlapping circles from its first frame and there is no moment at which its outline is one arc.
They are kept for the whole life rather than added while they are wanted and dropped after,
since a circle that comes and goes is a step in the drawing and the step is the size of the circle.

And a cloud drawn out sideways used to trail circles off its ends.
The far band is stretched to three times its width and tapered to a point, which puts the end lobes far apart and makes them small,
and small and far apart is a row of separate circles rather than a cloud thinning out.
Each lobe is now leaned on the one nearer the middle of the spine: pulled in until the two overlap, and never allowed to be the larger of the two.
The pulling keeps the mass in one piece; the capping keeps the *drawn* mass in one piece,
since a lobe under the smallest size worth drawing is dropped, and a small lobe with a big one beyond it would leave the big one hanging off nothing.

`tests/clouds.rs` holds all three to it: no group in the sky is one circle, and no circle in a group stands on its own,
at every setting and across a spread of moments.

## Adjustable, and not disruptive

The arc setting moves under a key while the sky is running, so turning it has to add arcs and do nothing else.
The shape of a cloud comes out of a stream of hashed numbers taken in order, so a loop that draws fewer numbers on one turn shifts every later circle in the cloud.
Every arc an element could have is therefore drawn from the stream whether it is used or not, and only the first few are laid down.
Turning the setting down takes the last arc off and leaves the rest where they were; turning it up is the reverse.
`tests/clouds.rs` checks that the number of elements and the first circle of each are the same at every setting.

## Twenty palettes

Every area in this drawing is filled with one of a palette's fifteen colours, so a palette that does not hold together is a picture that does not.
Four rules make that checkable, and `tests/sky.rs` holds all twenty to them:
the line is the darkest thing in the picture, the sky darkens from the horizon upwards,
the sand lightens into the distance, and the four cloud colours are four rather than two.
A fifth rule keeps the near clouds visible against all five sky colours, which is the one that fails most often,
since a near-white cloud on a near-white horizon is a shape you can only find by its outline.

That is what makes the twenty-first a matter of writing fifteen colours down and running the tests.
Fifteen of these were added that way, and the tests caught four of them putting a white cloud on a white sky before any of them reached the screen.

`P` walks forwards through them and `O` walks back.

## The sky down the bus

The first version sent the visible sky as a uniform buffer, which is held to 64 KB.
A crest here is a union of up to six circles rather than one, so a crowded frame at the widest setting carries several times the circles, and that ceiling is in the way.
It goes down as a read-only storage buffer instead.
Nothing else about the two differs from this shader's point of view: it walks the arrays and never writes to them.

## Working it

| | |
|---|---|
| `up` / `down` | the pace of the weather |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `Z` / `X` | the weight of the line around a cloud |
| `N` / `M` | how many arcs an element is built from |
| `P` / `O` | palette, forwards and back |
| `H` | hide the readout |
| `F11` | leave fullscreen |

## Seeing it without a window

```sh
cargo run -p moebius2 --release --example moebius2_still -- still.png 1920 1200 0 900 40 3 1.8
```

The arguments are the file, the size in pixels, which palette, how far into the weather to look in seconds,
which way to face in degrees, how many arcs an element is built from, and how wide the cloud line is in pixels.
It builds a headless device, renders through exactly the same pass the window does,
copies the result back and writes a PNG, timing eight frames on the way.
Every number in this piece was set by looking at one of those.
