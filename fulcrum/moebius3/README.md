# Moebius 3

[`moebius2`](../moebius2/README.md) with three things added:
the ceiling on the arcs raised from six to two dozen,
shading, drawn as hatch strokes on the shaded side of every element and across the flat bottom of every cloud that has one,
and a man on a horse crossing the desert underneath it all, over and over.

```
cargo run -p moebius3 --release
```

The window opens borderless over the whole display.
`F11` puts it back in a window.

## What is the same

The construction, and everything the first two READMEs explain about it.
Overlapping circles are laid down in a row; the edge of their union is the outline, so the outline is made of circular arcs meeting at cusps.
More circles go on top, and where a later one covers an earlier one the earlier arc is not there any more.
Each cloud is a **mass** of big circles along a spine, a **fringe** of small ones walked around the upper rim, and a few **crests** laid over the finished body.
No element is ever a single circle.
Three bands of cloud are drawn far to near, an enclosed region is one flat colour, and the whole sky is a function of one number.

## Two dozen arcs

The second version stopped at six because six was as many as a crest could use before the drawing filled in, which turned out to be a guess rather than a measurement.
Nothing in the construction wants a ceiling at all: an element is a union of circles, and a union takes as many as you like.
What sets the number is the frame, since every arc is a circle the shader tests against every pixel the element covers.

So the ceiling is now a measurement.
At two dozen arcs a full compass of weather comes to about six thousand circles, of which a frame holds a quarter, and the drawing costs about five milliseconds a frame more than it does at three on the integrated part that this was tuned on.
The `moebius3_still` example takes the setting on the command line and prints the frame time, which is how that was found.

The look changes more than the count suggests.
At three arcs an element is a bump with a cusp or two.
At twenty-four it is a rosette, and since the middle lobe of every cloud carries the same arcs a crest does, the clouds themselves swell into the great billowed heaps that the arc count is really a knob for.
`N` and `M` walk it.

## Shading, and the argument for it

The first two versions shade nothing and say so in as many words: no light direction used for anything except placing the sun's disc, no normal, no slope, no tone.
This one breaks that, and it is worth saying exactly how far.

Hatching is how a pen shades, and a pen has one colour, so nothing here is a tone: a pixel is either ink or the flat colour underneath it.
What the light decides is a region, and the region is filled with lines.
That is the same kind of decision the palette makes when it puts one flat colour halfway up the sky rather than the average of the two ends.

There are three levels, and two questions pick between them.

**Is this part of the cloud turned away from the sun?**
If it is not, it is left as flat colour.
A cloud here is a union of balls, and the distance loop already finds the nearest circle at every pixel, which is the billow that part of the cloud belongs to.
Keeping that circle costs a comparison in a loop that was already comparing, and it gives a normal per billow rather than per cloud.
That is the whole difference between shading that follows the form and shading that sits down one side of a silhouette:
every lobe of the mass, every scallop of the fringe, and every arc of a crest gets its own lit half and its own shaded half.

**Is it turned towards the ground?**
If it is, a second set of lines crosses the first.
A face turned away from the sun is unlit, but a face turned *down* cannot be lit by anything at all, because there is nothing under the horizon to light it.
That is why the undersides of a cumulus are its darkest value, and the flat base most of all.
The base is not a billow: where the half-space cut is the nearer surface, the normal is the plane's own, turned over, so it points at the ground.
And it is not put to the light at all.
Asking the sun about it was a bug and looked like one: the cut is tipped along with the cloud, so a cloud high in the sky towards a low sun caught it, and the flat bottom came out the colour it was filled with.
A cumulus with a clean bottom is a balloon.
Whatever the sun is doing and wherever the cloud is, the base is hatched, and it is crossed, and it is the darkest thing in the drawing.
A thunderhead in this sky has a dark bottom for the same reason it has one outside the window.

Three flat levels chosen by a decision are the same thing the sky does with its five flat bands.
What they are not is a ramp.

Turn to put the sun behind you and the clouds come nearly clean, keeping only the dark strip along their bases.
Turn to face it and they go dark but for a lit rim.
Stand side on and every billow is lit on one flank and shaded on the other.

One rule keeps it pen work rather than texture: the lines are spaced across the element rather than across the screen,
so a small cloud gets three and a big one gets a dozen, and they hold still while it drifts instead of sliding under it like a screen door.

## The strokes are pulled, not ruled

Evenly spaced parallel lines of one width are a fill pattern, and the eye names one on sight.
Five things are wrong with each stroke on purpose, and none of them is noise:

- it sits off its even place by up to a third of the spacing, so no two gaps are the same width
- it is drawn at its own weight, from two thirds of the pen to a little under half again
- it wanders as it is pulled, on a wave about nine spacings long with two shorter ones riding on it, and every stroke has its own phase, so a set does not wander in step
- the pressure comes and goes along the pull, so it is heavier in one place than another
- it stops short of the outline by a distance that changes from stroke to stroke, instead of leaving a white band of even width inside the edge

The second set crosses the first a little off the square, because two sets meeting at ninety degrees are a grid.

All five come off where the stroke is on the element rather than off a clock or a screen position, so the wobble holds still while the cloud drifts and a still of any moment is the drawing that was on the screen at that moment.
It costs about three per cent of the frame.

`S` turns it off and on.
`V` and `B` space the lines, from one line through the shade to a hatch fine enough to read as grey from across the room.
The lines are the same weight as the outline, so `Z` and `X` move both together: that is one pen, and it is the piece's oldest rule.

## The traveller

A Moebius desert is not empty.
There is a figure in it, usually one, usually small, going somewhere the panel does not say.
The emptiness is there because something is crossing it, and until there is a figure the desert is a backdrop rather than a distance.

He is drawn the way everything else here is drawn.
Every part of him is a segment with a radius, which is the shape a pen leaves and unions as cleanly as the circles a cloud is built from, and the distance that comes out is filled where it is negative and inked where it is near nought.
He gets the rock's flat colour, because he stands on the same horizon the rock does.
His legs are the only part that moves: two diagonal pairs half a stride out of step, with no knees, because a knee at his size is one pixel of argument.

He rides a circle around you rather than a line across the sand.
A line would take him out of the world and leave the desert empty until it brought him back.
A circle keeps him at one distance, which means one size.

The circle is the width of the frame, not the width of the compass.
He walks out of the right of the picture and in at the left, so there is a figure in the desert at every moment and wherever you have pointed your head.
A circle the whole way round is the honest one and it is the wrong one for a panel: he would be out of the picture for most of an hour at a time, and a desert with nothing crossing it is a backdrop.
He crosses in about eight minutes at pace one, and the pace knob carries him, since he comes off the same clock the sky does.

He is drawn three times over, a frame apart, of which at most two are ever on the screen.
That is what makes the fold a walk: the half of him leaving one edge is the half arriving at the other, rather than a jump the width of the picture the instant his middle crosses.
The width the fold uses is the frame's bottom edge, which is the narrow one.
A view tilted up at the sky is not a rectangle on the compass, the top edge covering more bearing than the bottom, and he rides along the bottom: folded into the middle width he is past the corner and off the picture for a quarter of a minute every lap.
`tests/rider.rs` runs him round a lap through the same projection the shader builds its rays from and asks whether he is on the screen.

One thing about him is a lie, and it is on the record in `tests/rider.rs` rather than buried.
He is drawn nine times life size.
The frame is pointed up at the clouds, so the desert in it runs from the horizon down to a tenth of a radian below, which is everything further off than about four hundred metres.
At those distances a true-sized man on a horse is four pixels, and he has to hold a line around him at the same weight as every other line in the picture, so anything under about forty pixels comes out as a stick of ink with no colour left inside it.
A comic artist draws the figure at the size it has to read at and lets the horizon look after itself.

## Working it

| | |
|---|---|
| `up` / `down` | the pace of the weather, and of the horse |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `Z` / `X` | the weight of the line, outlines and hatching together |
| `N` / `M` | how many arcs an element is built from, two to twenty-four |
| `S` | shading on and off |
| `V` / `B` | how far apart the hatch lines run |
| `P` / `O` | palette, forwards and back |
| `H` | hide the readout |
| `F11` | leave fullscreen |

## Seeing it without a window

```sh
cargo run -p moebius3 --release --example moebius3_still -- still.png 1920 1200 0 900 40 3 1.8 0.10
```

The arguments are the file, the size in pixels, which palette, how far into the weather to look in seconds,
which way to face in degrees, how many arcs an element is built from, how wide the line is in pixels,
and how far apart the hatch lines run as a fraction of an element's radius.
A hatch of nought turns the shading off.
It builds a headless device, renders through exactly the same pass the window does,
copies the result back and writes a PNG, timing eight frames on the way.
Every number in this piece was set by looking at one of those, and the arc ceiling was set by reading the frame time under it.
