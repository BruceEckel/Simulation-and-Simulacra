# Moebius

Clouds drawn the way they are drawn on paper:
an outline built from circular arcs, filled with one flat colour, with the next cloud drawn over the top of it.

```
cargo run -p moebius --release
```

The window opens borderless over the whole display.
`F11` puts it back in a window.

The five pieces are the same sky by five different methods:
[`thunderhead`](../thunderhead/README.md) grows cloud bitmaps on the CPU and blits them,
[`nimbus`](../nimbus/README.md) raymarches a real volume on the GPU,
[`ligne`](../ligne/README.md) evaluates a two-dimensional field per pixel and draws it with a line,
[`parrish`](../parrish/README.md) glazes that same field the way Maxfield Parrish glazed a sky,
and **`moebius`** does not compute a cloud at all.
[`moebius2`](../moebius2/README.md) is this one again with the line weight, the number of arcs an element is built from, and twenty palettes on keys.

## Circles, and what is left of them

The other four pieces all answer the same question: how much cloud is there at this point.
Then they shade the answer, or step it into bands, or glaze it, or draw a contour along it.
This one never asks.

A Moebius cloud is a construction, and it is a simple one.
Overlapping circles are laid down in a row; the edge of their union is the outline, so the outline is made of circular arcs meeting at cusps.
More circles go on top, and where a later one covers an earlier one the earlier arc is not there any more.
The whole cloud is that, and the arcs left over are the drawing.

So the program builds circles rather than a field.
Each cloud is a **mass**, four to seven big circles overlapping heavily along a spine, which on their own give a smooth heap;
a **fringe** of a dozen small ones walked around the upper rim of that mass, each standing a little proud of it, which is where the scallops come from;
and two to five **crests**, single circles laid over the finished body afterwards.

A crest is why this piece exists.
It is drawn after the body, so its fill covers the body's outline wherever the two overlap and its own arc survives on top.
A crest that straddles the body's edge puts a bump into the silhouette and leaves an arc running on into the mass.
One that falls inside closes on itself: a second cloud sitting on the first, which is what the brief asked for.
That is a pen and an eraser, and it is the reason a cloud here looks built rather than sampled.

The one edge in a cloud that is not an arc is the base.
A cumulus has one, it is straight, and leaving it out makes the whole heap read as a bag of bubbles,
so most clouds are cut back to a half-space before they are drawn.

## Flat, and only flat

Nothing in this picture is shaded.
There is no light direction used for anything except placing the sun's disc,
no normal, no slope, no cast shadow, no haze over distance and no tone anywhere.
An enclosed region is one colour, chosen from a list, and the line around it is what says where the region ends.

That leaves depth with nothing to work with except position and order,
which is what a comic artist has as well.
Three bands of cloud, drawn far to near: the low band along the horizon is small, slow, many, and drawn out sideways
until it is more streak than cloud, the way a cumulus looks when it is far enough away to be seen nearly edge on;
the high band overhead is large, quick and few.
Each band has its own flat colour off the palette, with a little swapping between neighbours,
so that two clouds side by side need not be the same colour.
That is a separation an artist would make, not a light value.

The sky and the desert are treated the same way.
Five flat colours are chosen for the sky and three for the sand, and a band lands on one of them rather than somewhere between two.
A stepped gradient and a separation look alike in a diagram and nothing alike on the screen:
the middle band of a stepped gradient is the average of the two ends, and the middle band of a separation is a decision.

## The line

Every line in the picture is the same colour and the same weight:
the cloud silhouettes, the arcs inside them, the sky's bands, the desert's, the horizon, the rock and the ring around the sun.
Uniform line weight is not a saving here, it is the style.

The clouds get their width for free.
A circle knows its own size in radians, a pixel knows how many radians it covers,
and the distance from a pixel to the edge of a union of circles is the smallest of `length(dir - centre) - radius`.
That is the chord across the sphere rather than the arc along it,
which over the size of a cloud is the same number to a fraction of a percent and is a subtraction instead of an arc cosine.
Two comparisons against that distance give the fill and the line, and they are the whole of the cloud drawing.

How many radians a pixel covers is taken from the projection rather than from a screen-space derivative,
which is what lets the cloud loop skip a group it does not touch.
A derivative would have to be told what all four pixels of a quad are doing, and here they are all doing something different.

The desert and the sky do use derivatives, because a band on the ground is a value rather than a shape
and the only way to give it a line of constant width is to ask how fast it is moving.
The one thing that needs is a test for when the answer is meaningless:
a line is only a line while the thing it follows is bigger than a pixel,
and where a value swings through several bands between neighbouring pixels the honest answer is that there is nothing there to draw.

## What makes it move

The whole sky is a function of one number, the clock.
Nothing accumulates, so there is no state to drift and a still can be rendered at any moment in the weather without playing up to it.

A cloud lives in a slot.
The slot's clouds each last two or three minutes, and when one is done the slot starts another somewhere else in the sky with a different shape.
The handover has to be invisible, and the way it is made invisible is that a cloud grows out of a puff in the middle,
stands at its full width for four fifths of its life, and pulls back into a puff again, with every circle on its own schedule.
Every schedule is held inside the first and last few percent of the life, so the slot is empty when it is handed on.

That leaves one trap, and it is worth writing down because it took a test to find.
The shape of a cloud comes out of a stream of hashed numbers, taken in order.
A loop that decides something before it has finished drawing from the stream —
skipping the rest of a turn when a circle has shrunk to nothing, say —
takes fewer numbers on that turn than on its neighbours, and every later circle in the cloud shifts by one number.
Nothing about that is visible in a still.
In motion it is a cloud that twitches, once, at the instant one of its circles reaches zero.
Every loop in `cloud.rs` now draws everything it needs before it decides anything,
and `tests/clouds.rs` watches for the twitch by tracking how much circle there is in the sky from one tenth of a second to the next.

## Only what is in front of you

The three bands together come to two or three hundred outlines around the whole compass, of which the frame holds about a fifth.
Throwing the rest away costs a dot product each on the CPU and saves the GPU from making the same test a few million times.
What is left is sent as two uniform buffers and walked per pixel, cheapest test first:
a group cannot reach further from its centre in height than its radius, and height alone rejects most of the sky for one subtraction.

Order is the whole of the depth here, so the culling drops groups out of the middle and leaves the rest in the order they were built.
`tests/clouds.rs` holds it to that, and to the margin between what a frame comes to and what the buffer has room for.

## Working it

| | |
|---|---|
| `up` / `down` | the pace of the weather |
| `left` / `right` | turn your head |
| `Space` | hold the sky still |
| `P` | palette: arzach, verdigris, ember, nocturne, mineral |
| `H` | hide the readout |
| `F11` | leave fullscreen |

## Seeing it without a window

```sh
cargo run -p moebius --release --example moebius_still -- still.png 1920 1200 0 900 40
```

The arguments are the file, the size in pixels, which palette, how far into the weather to look in seconds, and which way to face in degrees.
It builds a headless device, renders through exactly the same pass the window does,
copies the result back and writes a PNG, timing eight frames on the way.
Every number in this piece was set by looking at one of those.
