# moebius3

Moebius clouds with shading in them: hatched on the shaded side, up to two dozen arcs to an
element, and a man on a horse crossing the desert.

## The idea

`moebius.exe` builds a cloud out of overlapping circles and keeps the arcs that are left.
`moebius2.exe` puts the line weight, the number of arcs an element is built from and the palette
on keys. This one adds the two things those both refused, and raises the ceiling on the third.

Nothing about the construction has changed. Each cloud is still a **mass** of big circles along a
spine, a **fringe** of small ones around the upper rim, and a few **crests** laid over the
finished body, all of it flat colour with a line around each enclosed region.

## Two dozen arcs

The second version stopped an element at six circles. Nothing in the construction wanted a
ceiling there: a union takes as many circles as you like, and what sets the number is how many the
drawing can walk per pixel and stay a picture you can turn your head in. So the ceiling is now a
measurement rather than a guess, and it is four times higher.

The look changes more than the count suggests. At three arcs an element is a bump with a cusp or
two. At twenty-four it is a rosette, and since the middle of every cloud carries the same arcs,
the clouds themselves swell into great billowed heaps. `N` and `M` walk it.

## Shading

The first two versions shade nothing anywhere and say so. This one shades, and the whole of the
argument for it is that hatching is what a pen does. A pen has one colour, so no pixel here is a
tone: it is ink or it is the flat colour underneath. What the light decides is a region, and the
region is filled with lines.

Three levels, picked by two questions about the piece of cloud under the pixel. Is it turned away
from the sun? Then it is in shadow and gets a set of strokes. Is it turned towards the ground?
Then nothing can light it, because there is nothing below the horizon to do it, so a second set
crosses the first. That is why the undersides of these clouds are their darkest value, the way
they are out of the window. The flat bottom of a cumulus is not asked at all: it faces the ground,
so it is always hatched and always crossed, whatever the sun is doing.

The strokes are pulled rather than ruled. No two are the same distance apart, no two are the same
weight, each one wanders as it is drawn and presses harder in some places than others, and they
stop short of the outline by a distance that changes from stroke to stroke. Evenly spaced parallel
lines of one width are a fill pattern and the eye names one on sight.

A cloud is a union of balls, and the shading is worked out per ball rather than per cloud, so
every billow of the mass and every scallop around it has its own lit half and its own shaded half.
Turn to put the sun behind you and the clouds come nearly clean, keeping the dark strip along their
bases. Turn to face it and they go dark but for a lit rim. Stand side on and the form comes out.

`S` turns it off and on, `V` and `B` space the lines. The lines are the same weight as the
outlines, so `Z` and `X` move both together.

## The traveller

A Moebius desert is not empty. There is a figure in it, usually one, usually small, going
somewhere the panel does not say, and the emptiness is there because something is crossing it.

He is a man on a horse, walking, drawn with the same two rules as everything else: one flat colour
with a line around it. He walks out of one side of the picture and in at the other, so there is a
figure in the desert at every moment and wherever you have pointed your head: the frame is a loop
and he goes round it. He takes about eight minutes to cross, and the pace keys carry him along
with the weather.

He is drawn nine times life size, which is a thing worth admitting. At the distances this frame
shows the ground, a true-sized rider is four pixels.

## Working it

It opens fullscreen and takes the entire display. `F11` gives a normal window back.

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

`moebius.exe` and `moebius2.exe` are the two before it, and the four other cloud pieces answer the
same brief by other methods: `thunderhead.exe` grows cloud bitmaps and blits them, `nimbus.exe`
raymarches a real cloud volume, `ligne.exe` evaluates a two-dimensional field per pixel and draws
it with a line, and `parrish.exe` glazes that same field in transparent coats over a white ground.

Source, and the long version of the explanation, in [`fulcrum/moebius3`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/moebius3).
