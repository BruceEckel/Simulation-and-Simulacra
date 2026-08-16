# ligne

Ligne claire clouds: a live two-dimensional cloud field, drawn in flat colour with a clean line
around everything.

## The idea

*Ligne claire* is the drawing style: uniform line weight, flat areas of colour, no hatching and
no shading anywhere. This is that style computed rather than drawn, at one sample per pixel,
sixty times a second.

A cloud deck here is a horizontal plane at a given altitude. A ray is intersected with it once,
and a sheet of tiling noise is read at the point where it lands. There is no volume, nothing is
marched, and one texture fetch is a whole cloud field. Three decks at three altitudes are drawn
back to front; lower is nearer, so the low deck carries the big shapes overhead and the high
ones stack up behind it towards the horizon.

## Two-dimensional, and still with a side to it

Three things stand on that single sample and between them give it depth.

The sample is **lifted onto the cloud's own top**. A deck is a plane, but the cloud standing on
it is not, and a ray meets the top of a cloud some way before it meets the plane underneath.
Asking how high the cloud is here and then re-intersecting at that height is the difference
between a pattern painted on a ceiling and a heap with a side to it: near the horizon the clouds
stand up and show their flanks, and overhead they bulge towards you.

The field is then **lit as a height field**, its slope measured over a step long enough to find
the lobe rather than the grain, because a slope read at the finest scale makes the light follow
the texture instead of the shape.

And a **short walk towards the sun casts a shadow**, asking whether the cloud over there stands
higher than the sun ray does by the time it gets there. That puts one cloud's shadow on the
shoulder of the next, which is what makes a flat field read as a heap.

## The line

Every line in the picture is analytic. The program knows how fast a value is changing from one
pixel to the next, so it can convert a width in pixels into a width in whatever that value
measures. A contour drawn that way is the same weight everywhere: on a cloud overhead, on one at
the horizon, on the same cloud after you resize the window, and on a display where a line
measured in anything else would have thickened or vanished.

Everything is drawn with it. The cloud silhouettes get a heavier line and the tone contours
inside them a lighter one, the way a pen models a shape after outlining it, and the sky's bands,
the desert's bands, the horizon, the rock and the ring around the sun are all the same line.

Nothing in the palette is a light value. These are the colours that land on the screen: the
light decides which band a patch of cloud is in, and the band decides the colour. A cloud is
five flat areas and four lines, and a whole sky is eleven colours.

## What makes it move

Two noise fields are added together and scrolled at different speeds. Two fields moving together
would be a picture being slid across the window; two fields moving apart are a picture being
redrawn, so clouds grow, lean, split and close again as they cross, and the sky never repeats.

## Working it

It opens fullscreen and takes the entire display. `F11` gives a normal window back.

| | |
|---|---|
| `up` / `down` | the pace of the wind |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `P` | palette: arzach, verdigris, ember, nocturne, mineral |
| `H` | hide the readout |
| `F11` | leave fullscreen |

There are two companion pieces with the same sky by other methods: `thunderhead.exe` grows cloud
bitmaps and blits them, and `nimbus.exe` raymarches a real cloud volume.

Source, and the long version of the explanation, in [`fulcrum/ligne`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/ligne).
