# moebius

Moebius clouds: cloud outlines built from circular arcs, filled flat, one colour to an enclosed
region.

## The idea

The other four cloud pieces in this folder all answer the same question — how much cloud is
there at this point — and then shade the answer, or step it into bands, or glaze it, or draw a
contour along it. This one never asks.

A Moebius cloud is a construction, and a simple one. Overlapping circles are laid down in a row,
and the edge of their union is the outline, so the outline is made of circular arcs meeting at
cusps. More circles go on top, and where a later one covers an earlier one, the earlier arc is
not there any more. The whole cloud is that, and the arcs left over are the drawing.

So the program builds circles rather than a field. Each cloud is a **mass** of four to seven big
circles overlapping heavily along a spine, which on their own give a smooth heap; a **fringe** of
a dozen small ones walked around the upper rim, each standing a little proud of the mass, which
is where the scallops come from; and a few **crests** laid over the finished body afterwards.

A crest is the point of the whole thing. It is drawn after the body, so its fill covers the
body's outline wherever the two overlap and its own arc survives on top. A crest that straddles
the edge puts a bump into the silhouette and leaves an arc running on into the mass; one that
falls inside closes on itself, a second cloud sitting on the first. That is a pen and an eraser,
and it is why a cloud here looks built rather than sampled.

The one edge in a cloud that is not an arc is the base. A cumulus has one, it is straight, and
leaving it out makes the whole heap read as a bag of bubbles.

## Flat, and only flat

Nothing in the picture is shaded. There is no light direction used for anything except placing
the sun, no normal, no slope, no cast shadow, no haze and no tone. An enclosed region is one
colour and the line around it says where it ends.

That leaves depth with nothing to work with but position and order, which is what a comic artist
has as well. Three bands of cloud are drawn far to near: the low band along the horizon is small,
slow and drawn out sideways until it is more streak than cloud, the way a cumulus looks when it
is far enough away to be seen nearly edge on; the band overhead is large, quick and few. Each
band has its own flat colour, with a little swapping between neighbours so that two clouds side
by side need not match.

The sky and the sand are treated the same way: five flat colours are chosen for one and three for
the other, and a band lands on one of them rather than somewhere between two. A stepped gradient
and a separation look alike in a diagram and nothing alike on the screen.

## The line

Every line in the picture is the same colour and the same weight — the silhouettes, the arcs
inside them, the sky's bands, the desert's, the horizon, the rock and the ring around the sun.
Uniform line weight is not a saving here, it is the style.

The clouds get their width for free. A circle knows its own size as an angle, a pixel knows what
angle it covers, and the distance from a pixel to the edge of a union of circles is the smallest
of those distances. Two comparisons against it give the fill and the line, and that is the whole
of the cloud drawing.

## What makes it move

The entire sky is a function of one number, the clock, so nothing accumulates and nothing drifts.
A cloud lives in a slot for two or three minutes; when it is done, the slot starts another one
somewhere else with a different shape. The handover is invisible because a cloud grows out of a
puff in the middle, stands at its full width for four fifths of its life, and pulls back into a
puff again, with every circle arriving and leaving on its own schedule.

## Working it

It opens fullscreen and takes the entire display. `F11` gives a normal window back.

| | |
|---|---|
| `up` / `down` | the pace of the weather |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `P` | palette: arzach, verdigris, ember, nocturne, mineral |
| `H` | hide the readout |
| `F11` | leave fullscreen |

There are four companion pieces with the same sky by other methods: `thunderhead.exe` grows
cloud bitmaps and blits them, `nimbus.exe` raymarches a real cloud volume, `ligne.exe` evaluates
a two-dimensional field per pixel and draws it with a line, and `parrish.exe` glazes that same
field in transparent coats over a white ground.

Source, and the long version of the explanation, in [`fulcrum/moebius`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/moebius).
