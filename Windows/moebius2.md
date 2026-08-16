# moebius2

Moebius clouds with the drawing on keys: the weight of the cloud line, how many arcs an element
is built from, and twenty palettes.

## The idea

`moebius.exe` builds a cloud out of overlapping circles and keeps the arcs that are left. Every
decision about how that gets drawn is fixed in the source: one line weight for the whole picture,
one circle to a crest, five palettes. This is the same construction with three of those decisions
handed over.

Nothing about the sky itself has changed. Each cloud is still a **mass** of big circles along a
spine, a **fringe** of small ones walked around the upper rim, and a few **crests** laid over the
finished body afterwards, all of it flat colour with a line around each enclosed region, and the
whole sky a function of one number.

## The line

Every line in `moebius.exe` is one weight, and that was the style. Here the clouds keep their own
weight and everything else keeps the old one. `Z` and `X` move it between six tenths of a pixel
and seven while you hold them.

The rest of the panel is deliberately left alone. The sky's bands, the desert's, the horizon, the
rock and the ring around the sun are the paper the drawing stands on, and thickening those along
with the clouds changes the paper rather than the drawing. At six pixels the clouds read as a
woodcut against a sky still ruled with a fine pen.

## No element is a circle

A crest that falls inside a cloud closes on itself, and in `moebius.exe` it closed as a circle,
because a crest was one circle. A circle inside a cloud reads as a hole punched in the paper
rather than as a second cloud sitting on the first.

An element here is never one circle. A crest is a union of two to six, and `N` and `M` set which.
The extra arcs are placed against the main circle at a distance worked out from the two radii:
any closer and one swallows the other, any further and they come apart, and anywhere between them
is a union with a cusp in it. The same treatment reaches two other places where a circle used to
show up: the first seconds of a cloud's life, when the middle of the mass has arrived and the
rest is still on its way, and the ends of the long stretched clouds along the horizon, which used
to trail separate circles off into the sky.

Turning the setting is meant to add arcs and do nothing else, so every arc an element could have
is worked out whether it is used or not. Turning it down takes the last arc off and leaves the
rest where they were.

## Twenty palettes

Every area in the picture is filled with one of a palette's fifteen colours, so a palette that
does not hold together is a picture that does not. Four rules make that checkable: the line is
the darkest thing there is, the sky darkens from the horizon upwards, the sand lightens into the
distance, and the four cloud colours are four rather than two. A fifth keeps the near clouds
visible against all five sky colours. The fifteen new ones were written against those rules, and
the tests caught four of them putting a white cloud on a white sky before any reached the screen.

`P` walks forwards through them and `O` walks back.

## Working it

It opens fullscreen and takes the entire display. `F11` gives a normal window back.

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

`moebius.exe` is the original, and the four other cloud pieces answer the same brief by other
methods: `thunderhead.exe` grows cloud bitmaps and blits them, `nimbus.exe` raymarches a real
cloud volume, `ligne.exe` evaluates a two-dimensional field per pixel and draws it with a line,
and `parrish.exe` glazes that same field in transparent coats over a white ground.

Source, and the long version of the explanation, in [`fulcrum/moebius2`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/moebius2).
