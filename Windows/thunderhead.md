# thunderhead

A flat desert under enormous drifting thunderheads, drawn one physical pixel at a time.

## The idea

Moebius drew weather the way nobody else did: huge clouds built out of flat areas of colour,
each one closed by a clean unvarying line, no gradients and no texture anywhere. That is a
drawing technique, but it is also a data structure, and this is what happens when you take it
literally.

Every pixel of the window holds one byte, and that byte is not a colour. It is a **material**: a
band of sky, the sun, a band of sand, that same band with a cloud's shadow across it, rock on
the horizon, a band of cloud at one of three distances, or ink. There are sixty-six of them. The
picture is composed entirely in those bytes, and only at the last moment does every byte go
through a sixty-six entry table to become a colour.

Two things fall out of that, and both are the point. The drawing is flat by construction,
because there is nothing in the output that *can* shade: a region is one material, so it is one
colour, so it has a hard edge. And a palette is sixty-six numbers, so `P` recolours a
four-million-pixel picture in the time it takes to fill a small array.

## The clouds

Each thunderhead is grown once as a bitmap of its own and then only ever moved. A few dozen
ellipses are laid out as a real cumulonimbus: a flat base, a bank of lobes sitting on it, a
tower boiling up and leaning downwind as it climbs, and at the top the anvil, spreading sideways
under the ceiling it cannot pass. Then each lobe sprouts smaller lobes around its rim.

The one decision that matters is how those lobes are combined. Added together, two overlapping
lobes bulge where they meet and the outline of the pile comes out as one smooth blob, which
reads as rock. Taken larger-of, the outline is the union of the lobes and stays a chain of arcs,
which is how a cloud is drawn with a pen. A lobe buried inside a bigger one then leaves no mark
at all, so the inside of the cloud stays calm however much detail is heaped onto its edge.

The field is lit, quantised into ten bands, and inked: two or three pixels around the
silhouette, one along every third contour inside it. Every third, because every one is lace.

A cloud that drifts off the left edge comes back in on the right wearing a shape that is grown
while it is still out of frame, so the sky never repeats and never stutters.

## Working it

It opens fullscreen and takes the entire display. `F11` gives a normal window back.

| | |
|---|---|
| `up` / `down` | the pace, from a twentieth of the drift to twenty-four times it |
| `Space` | hold the sky still |
| `P` | palette: arzach, verdigris, ember, nocturne, mineral |
| `H` | hide the readout |
| `F11` | leave fullscreen |

There are two companion pieces with the same sky by other methods: `nimbus.exe` raymarches a
real cloud volume, and `ligne.exe` evaluates a two-dimensional field per pixel and draws it.

Source, and the long version of the explanation, in
[`fulcrum/thunderhead`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/thunderhead).
