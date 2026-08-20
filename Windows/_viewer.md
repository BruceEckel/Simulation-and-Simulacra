# _viewer

The front door: every simulation in the set, what it is, and a way to start it.

## What it is

A release is a folder of twenty-two executables with names like `moebius3.exe` and `heatflow.exe`,
and nothing in a folder tells you which of them you would like. This is the one that does. It
lists all twenty-one simulations with the line each one describes itself with, and starts
whichever you pick.

It is called `_viewer` for one reason only: the underscore sorts it to the top of a directory
listing, so it is the first thing you see when you open the folder, which is where a thing like
this belongs.

## Working it

| | |
|---|---|
| `up` / `down` | choose |
| the mouse | choose whatever it is over |
| `Enter` | run the chosen one |
| a click | run whatever you clicked on |
| `Esc` | close this |

Starting a simulation leaves this open, so you can come back and start another. Closing this does
not close anything it started, and closing something it started does not close this.

## What it does when something is not there

It looks for the simulations **beside itself**, and it says under the list where it found the one
you have chosen. Anything it cannot find is still listed, with what it is and a note that it is
not in this directory.

That happens in two ordinary situations. In a release, if you downloaded some of the executables
and not others — the missing ones are still described, so you can see what you are missing. And
while working on the source, where they only exist once they have been built:

```sh
cargo build --workspace --release
```

## Where the descriptions come from

Each simulation says what it is in its own package manifest, once. The viewer reads those at
build time and compiles the result in, so nothing here is a second copy of anything, and a
simulation added to the set appears in this list with no edit to the viewer at all.

Source in [`fulcrum/_viewer`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/_viewer).
