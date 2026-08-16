# parrish

Maxfield Parrish clouds: a live cloud field over still water, painted in transparent coats over a
white ground.

## The idea

Parrish did not mix paint. He laid down a white ground, then covered it with thin transparent
films of a single pigment, varnishing between coats and building up as many as thirty of them.
The light you see has gone down through the colour, off the white, and back out through it again.
That is why the blue is that deep and still looks lit from within, and why the shadows are
saturated blue rather than the grey that mixing gives you.

This is that, computed. There are no colours anywhere in the program. Every entry in a palette is
a **transmittance**: what one coat lets through. A colour is never chosen, only arrived at, by
deciding how many coats of what stand between the eye and the ground.

## What follows from it

Coats multiply rather than add, so two coats are the tint squared. Colour deepens and saturates
along a curve instead of sliding towards whatever it is being mixed with, which is the difference
between a sky that gets darker and a sky that gets bluer.

And a glaze can only darken: nothing laid over white comes out brighter than white. So every
luminous passage here is paint that was not laid, or paint taken back off. The glow around the sun
is the blue of the sky scrubbed back towards the bare ground. The bright edge of a cloud with the
light behind it is its own shadow coats lifted off again. Both stop at the ground. The sun's disc
is the one thing in the frame allowed past it.

Distance works the same way, and it is the clearest case. Fading towards a haze colour goes milky
and grey; a painter in glazes gives the far range *fewer* coats and one thin blue over the top, so
it comes out paler and more saturated than the near one at the same time.

## The clouds

A cloud deck is a horizontal plane at an altitude. A ray meets it once, and a sheet of tiling
noise is read at the point where it lands, so one texture fetch is a whole cloud field. Three
decks are drawn back to front.

A plane seen from underneath is foreshortened, so a cloud painted on one is a six-to-one smear.
What stands it back up is height: a ray meets the top of a tall cloud well before it meets the
plane underneath, and asking how high the cloud is here and re-intersecting at that height is what
turns a pattern on a ceiling into a heap with a side to it. The light then reads the same field as
a height field and wraps a little way past the shoulder, the way light actually does in something
that is not solid.

Below about eight degrees the foreshortening wins and no amount of that helps, so the clouds fade
out into the band above the horizon. Every one of these paintings leaves that band clear and
luminous anyway.

## Below the sky

A Parrish is lit by contrast against something dark and near, not by anything in it being white.
So there are three silhouettes stacked into the distance, each a shade deeper than the one behind
it: a far range of hills, a nearer one, and a rock across the bottom of the frame.

Between the hills and the rock is water, and the water is the sky again. The lower part of the
frame runs the whole sky a second time with the ray turned upside down, a little blurrier, and
then glazes it. Far water is a mirror and near water is a hole, so the reflection is clean at the
horizon and deep at your feet.

## Working it

It opens fullscreen and takes the entire display. `F11` gives a normal window back.

| | |
|---|---|
| `up` / `down` | the pace of the wind |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `P` | palette: daybreak, cobalt, hilltop, twilight, ecstasy |
| `H` | hide the readout |
| `F11` | leave fullscreen |

There are three companion pieces with the same sky by other methods: `thunderhead.exe` grows cloud
bitmaps and blits them, `nimbus.exe` raymarches a real cloud volume, and `ligne.exe` draws the same
two-dimensional field this one glazes, in flat colour with a clean line around everything.

Source, and the long version of the explanation, in [`fulcrum/parrish`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/parrish).
