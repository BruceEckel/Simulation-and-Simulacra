# Windows

Notes on each simulation, one `.md` per executable, written for whoever downloads one.

The executables are **not** here. They are built from this repository and published to a
[release](https://github.com/BruceEckel/Simulation-and-Simulacra/releases), which is the only
place they exist. A binary committed beside its own source is a stale copy of something git
stores badly, and it was never any use to anybody else: it is out of date the moment the source
moves on.

The notes do go up with them. Every release carries these twenty-two files alongside the
executables, so somebody who has downloaded one `.exe` and nothing else can find out what it is
and which keys it answers to without coming back here.

| Executable | Description |
|---|---|
| [`_viewer.exe`](_viewer.md) | The front door: every simulation in the set, what it is, and a way to start it. |
| [`avalanche.exe`](avalanche.md) | A table of sand with one rule, and the power law that falls out of it. |
| [`boids.exe`](boids.md) | Reynolds flocking on a deterministic spatial grid. |
| [`flutter.exe`](flutter.md) | A swarm of moths around a lamp: add moths, take them away, and run it at any pace. |
| [`fractal.exe`](fractal.md) | Ten fractals, two families, and a progressive viewer to zoom into them with. |
| [`heatflow.exe`](heatflow.md) | Statistical heat flow: a hard-disk gas conducting heat between two thermal walls. |
| [`jig.exe`](jig.md) | A dancing skeleton: shake the hips, and eighteen pendulums work out the rest. |
| [`life.exe`](life.md) | Conway's Game of Life and forty-three of its relatives, from blocky cells down to one per pixel. |
| [`ligne.exe`](ligne.md) | Ligne claire clouds: a live two-dimensional cloud field, drawn in flat colour with a clean line around everything. |
| [`lullaby.exe`](lullaby.md) | A field of light that cools, settles, dims to black, and goes on breathing after you close your eyes. |
| [`mesmerize.exe`](mesmerize.md) | A slow curl-flow field of light that breathes at five and a half breaths a minute. |
| [`moebius.exe`](moebius.md) | Moebius clouds: cloud outlines built from circular arcs, filled flat, one colour to an enclosed region. |
| [`moebius2.exe`](moebius2.md) | The same clouds with the drawing on keys: the weight of the line, how many arcs an element is built from, and twenty palettes. |
| [`moebius3.exe`](moebius3.md) | Those clouds with shading in them: hatched on the shaded side, up to two dozen arcs to an element, and a man on a horse crossing the desert. |
| [`nimbus.exe`](nimbus.md) | Real-time volumetric clouds over a desert: a raymarched cloud layer, lit by a marched sun. |
| [`parrish.exe`](parrish.md) | Maxfield Parrish clouds: a cloud field painted the way he painted, in transparent coats over a white ground. |
| [`popped.exe`](popped.md) | Hot-air balloons full of cheerful animals, and a mouse pointer. |
| [`rally.exe`](rally.md) | Pong as a simulation: autonomous paddles, a court that keeps adding balls and paddles. |
| [`rts-slice.exe`](rts-slice.md) | An RTS slice: selection, flow-field movement, combat, mods, replays. |
| [`spectacle.exe`](spectacle.md) | A fireworks show over dark water, put on for no reason but the watching. |
| [`starry.exe`](starry.md) | Van Gogh's Starry Night as a fluid: brushstrokes carried by the current that drew them. |
| [`thunderhead.exe`](thunderhead.md) | A flat desert under enormous drifting thunderheads, drawn one physical pixel at a time. |

## Getting one

Take it from a [release](https://github.com/BruceEckel/Simulation-and-Simulacra/releases). Every
simulation is one file, with its assets compiled in, so there is nothing to unzip and nothing to
keep beside it: save it wherever you like and run it.

The binaries are unsigned, so Windows SmartScreen warns the first time you run one: click **More
info**, then **Run anyway**. Or clear the download mark first with `Unblock-File .\moebius3.exe`.
Each release carries a `SHA256SUMS.txt` if you would rather check than trust.

Releases are Windows only. For macOS or Linux, clone the repository and `cargo build --workspace
--release`; nothing in the engine is platform-specific.

## Which to open first

**`_viewer.exe`.** It is named with an underscore so it sorts to the top of the folder, and it is
the one that explains the other twenty-one: it lists them with what each one is, and starts
whichever you pick. Everything below is still true, and the viewer will tell you most of it.

`thunderhead.exe`, `nimbus.exe`, `ligne.exe`, `parrish.exe`, `moebius.exe`, `moebius2.exe` and
`moebius3.exe` are the ones to know about before double-clicking. Each opens borderless over the
whole display and has no visible close button; `F11` gives a normal window back.

Those seven are one sky by seven methods, and they are worth looking at in that order:
`thunderhead.exe` grows cloud bitmaps and blits them, `nimbus.exe` raymarches a real cloud volume,
`ligne.exe` evaluates a two-dimensional field per pixel and draws it with a line, `parrish.exe`
glazes that same field in transparent coats over a white ground, and `moebius.exe` computes no
cloud at all: it draws overlapping circles and keeps the arcs that are left. `moebius2.exe` is
that last one again with the line weight, the arc count and the palette on keys, and
`moebius3.exe` is that one with shading in it, drawn as hatch strokes, and a man on a horse
crossing the sand underneath.

`life.exe` is the one most people will already know the name of, and it opens in an ordinary
window like everything else here. It is worth knowing that it is not only Conway's rule: `Tab`
walks between three families of them, and `X` held down takes the field from cells you could
count with your finger to one cell per physical pixel.

## Building them yourself

```sh
make release
```

That builds every simulation, collects one executable each into `dist/`, copies these notes in
beside them, and writes the `SHA256SUMS.txt` that goes up with them. `dist/` is ignored by git.
`make publish VERSION=v0.1.0` does the same build, tags the commit, and uploads the lot to a
GitHub release with the `gh` CLI. The released binaries are built on a Windows machine by hand
rather than in CI, so they are the same files whoever publishes them can run.

Every simulation in [`fulcrum/`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum) has a note here, and `make release` refuses to build
a set where one is missing.
