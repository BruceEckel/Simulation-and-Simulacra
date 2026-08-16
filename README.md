# Simulation and Simulacra

Experiments in simulation.

Twenty pieces, each one a program that runs a rule and draws what the rule does.
Some are simulations in the ordinary sense: sand piling up until it slides, a gas carrying heat between two walls, a flock with three rules and no leader.
The rest are simulacra, which is the other half of the title: a Moebius sky, a Parrish sky, a Van Gogh painting run as a fluid.
Those simulate a way of drawing rather than a piece of the world.

```sh
make            # what all of this does
make sims       # the list
make run SIM=moebius3
```

## The engine

These are built on [Fulcrum](https://github.com/jcerise/fulcrum), a deterministic 2D engine.
It is **imported rather than copied, and used unmodified**: `Cargo.toml` names it as a git dependency, cargo fetches it into its own cache, and no copy of it lives in this tree.
There is no fork, no patch table, and nothing vendored.
`jcerise/fulcrum` is the only thing this repository depends on.

`Cargo.lock` records the commit it resolved to.
That is deliberate: a build does not drift under you because somebody pushed to the engine this morning, and a tag from a year ago still rebuilds into the same executables.
New engine work is taken on purpose:

```sh
make engine     # move the pin to the newest commit, then build and test against it
```

If the new commit breaks something, `git checkout Cargo.lock` puts the pin back.

### One file, and what it takes

A simulation here is meant to be one file.
You hand somebody `moebius3.exe` and it runs: no directory to keep beside it, nothing to unzip, no path from your machine baked into it.

The engine has no notion of that.
`AssetServer::new(root)` takes a directory, and a game written the ordinary way passes `concat!(env!("CARGO_MANIFEST_DIR"), "/assets")`, which is an absolute path fixed at build time.
That is right while the source tree is there and wrong everywhere else.

So the assets ride along inside the binary, and `crates/simulacra-assets` is the seam that makes them readable.
It is this repository's own code and it does not patch the engine: it is built on `AssetServer::new` and `AssetServer::mount`, both of which the engine already offers.
Each `main` writes `assets!()` and gets an `AssetServer` back.

What that macro does depends on which directory is really there:

- **the source tree is present**, which is every `cargo run` and every `cargo test`: its `assets` directory is mounted and nothing else happens, so hot reload behaves exactly as the engine intends and nothing is written anywhere;
- **it is not**, which is what a downloaded executable finds: the compiled-in copy is written out to a directory under the system temp, once per version, and mounted instead.

Either way an `assets` directory sitting next to the executable is mounted on top, so anything dropped there wins without a rebuild.

The copy is written out rather than read from memory because every read in the engine ends at `std::fs::read` and every listing at `std::fs::read_dir`, with no mount that is not a directory.
Given the choice between changing the engine and handing it a directory, this hands it a directory.
The unpacked copy is a cache: delete it whenever you like and the next run writes it again.

## Releases

Nothing builds this in the cloud.
The set is compiled here and pushed up with the `gh` CLI, so what people download is what you can run.
Every simulation is one file with its assets inside it, plus `SHA256SUMS.txt`.

```sh
make check                     # format, clippy, tests, determinism in release
git commit -am "..." && git push
make release                   # 20 executables into dist/
make publish VERSION=v0.1.0    # builds, tags, pushes, uploads
```

The executables are **not** committed here.
They are built from this repository and published to a [release](https://github.com/BruceEckel/Simulation-and-Simulacra/releases), which is the only place they exist.
A binary committed beside its own source is a stale copy of something git stores badly.

## The pieces

| | |
|---|---|
| `avalanche` | A table of sand with one rule, and the power law that falls out of it. |
| `boids` | Reynolds flocking on Fulcrum's deterministic spatial grid. |
| `flutter` | A swarm of moths around a lamp: add moths, take them away, and run it at any pace. |
| `fractal` | Ten fractals, two families, and a progressive viewer to zoom into them with. |
| `heatflow` | Statistical heat flow: a hard-disk gas conducting heat between two thermal walls. |
| `jig` | A dancing skeleton: shake the hips, and eighteen pendulums work out the rest. |
| `ligne` | Ligne claire clouds: a live two-dimensional cloud field, drawn in flat colour with a clean line around everything. |
| `lullaby` | A field of light that cools, settles, dims to black, and goes on breathing after you close your eyes. |
| `mesmerize` | A slow curl-flow field of light that breathes at five and a half breaths a minute. |
| `moebius` | Moebius clouds: cloud outlines built from circular arcs, filled flat, one colour to an enclosed region. |
| `moebius2` | Moebius clouds, adjustable: the line weight, the number of arcs an element is built from, and twenty palettes. |
| `moebius3` | Moebius clouds with shading: hatched on the shaded side, up to two dozen arcs to an element, and a man on a horse crossing the desert. |
| `nimbus` | Real-time volumetric clouds over a desert: a raymarched cloud layer, lit by a marched sun. |
| `parrish` | Maxfield Parrish clouds: a cloud field painted the way he painted, in transparent coats over a white ground. |
| `popped` | Hot-air balloons full of cheerful animals, and a mouse pointer. |
| `rally` | Pong as a simulation: autonomous paddles, a court that keeps adding balls and paddles. |
| `rts-slice` | An RTS slice: selection, flow-field movement, combat, mods, replays. |
| `spectacle` | A fireworks show over dark water, put on for no reason but the watching. |
| `starry` | Van Gogh's Starry Night as a fluid: brushstrokes carried by the current that drew them. |
| `thunderhead` | A flat desert under enormous drifting thunderheads, drawn one physical pixel at a time. |

Most carry a `README.md` of their own explaining how the piece is built and what the keys do.

## Layout

```
Cargo.toml          the workspace, and where the engine is imported from
Makefile            build, check, release, publish
crates/             this repository's own code
  simulacra-assets/ the assets!() macro: a simulation's assets, inside its executable
fulcrum/            the simulations, one package each
  moebius3/
    src/  assets/  tests/  examples/  README.md
```

The directory is called `fulcrum/` because that names the family: these are the pieces built on that engine.
A set built on something else would sit beside it under its own name and be added to `members` in `Cargo.toml`.
