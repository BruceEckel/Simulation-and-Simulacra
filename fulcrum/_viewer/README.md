# _viewer

The front door: every simulation in the set, what it is, and a way to start it.

This is the source note. What the keys do is in [`Windows/_viewer.md`](../../Windows/_viewer.md).

## Why it is here and why it is called that

A release is a folder of twenty-two executables, and a folder of executables tells you nothing
about which of them you would like to look at. The underscore is the whole of the naming
decision: `_` sorts before every letter, so this is the first thing in the folder, which is where
something that explains the rest belongs.

It is under `fulcrum/` with the simulations rather than in `crates/` for a practical reason:
`make release` builds everything under `fulcrum/` and collects it into `dist/`, so being there is
what makes it ship. It is not a simulation, and the Makefile's wording says "executables" rather
than "simulations" for that reason.

## The catalogue writes itself

Every simulation already says what it is, once, in the `description` of its own `Cargo.toml`.
That is the line cargo shows, and the line both README tables carry. So the viewer does not get a
third copy to fall out of date: `build.rs` reads them where they are written and emits the table,
which is compiled in.

Compiled in rather than read at runtime for the same reason the assets are: this has to work as a
downloaded executable with no source tree within reach.

The effect worth having is that **a simulation added to the set appears in the viewer with no
edit to this crate at all** — and `tests/catalogue.rs` checks exactly that, by reading `fulcrum/`
itself and asserting the compiled-in list matches it. A viewer that quietly missed a simulation
would be worse than no viewer, because it would look complete.

That test is also what caught `rts-slice` describing itself as "Fulcrum milestone game 4", a
leftover from the repository this was ported from that both READMEs had already corrected and the
manifest had not.

## Finding the executables

It looks in the directory it is in, and that single answer is right both ways round: a release is
unpacked into one directory with every executable side by side, and a development build puts them
all in `target/release`, side by side again.

Anything not found is still listed with its description and a note that it is not here, which is
the useful behaviour in both of the situations it happens in — a partial download, and a source
tree where you have not run `cargo build --workspace --release` yet.

## Laying it out, and the mistake worth writing down

The first version of this laid the list out against *assumed* font proportions, and was wrong in
both axes at once. The engine's built-in font is **exactly square** — one pixel of character
advance and one of line height per pixel of text size — where an ordinary typeface is nothing
like. Assuming otherwise made the rows run off the right-hand edge, the heading run off the top,
and, worst of all, put the highlight bar on a different row from the one that would be launched:
the bar sat on `boids` while `avalanche` was selected, so clicking `boids` ran `avalanche`.

So the font is measured, once, with `GlyphCache::measure`, and everything is laid out against
what it says. That is `Metrics`, and its `Default` is not a guess — it is what the measurement
returns.

The second half of the fix matters more, because measuring correctly would only have made that
bug rarer rather than impossible. **The selection is not a bar drawn behind the list.** It is the
chosen row's own text, written again in its own colour at the list's own anchor, with the blank
lines above it spelled out (`overlay`). The engine then lays it out with exactly the arithmetic
it laid the list out with, so the two cannot disagree, and nothing in this crate has to know
where in a line a baseline sits. `tests/catalogue.rs` holds `overlay` to putting the text on its
own row and no other.

The size itself is whichever limit binds first — the height, so every row is on screen at once,
or the width, so a row of about `AIM_COLUMNS` characters fits across. That is an aim rather than
a requirement: the longest description is half again the length of the typical one, and sizing
everything down for the sake of one line is a bad trade, so `clip` shortens whatever still does
not fit and marks where it cut. Nothing is lost by that — the notes under the list always show
the chosen one in full.

Two small things that are only noticeable when they are wrong, and both are commented where they
happen. The pointer only changes the choice while it is actually moving, so a still mouse left
lying over the list does not drag the selection back every tick and make the arrow keys useless.
And a click only counts over the list itself, so clicking the heading does not start whatever
happened to be chosen.

## Closing it

`Esc` calls `std::process::exit`. The engine has no way for a game to ask the event loop to stop,
and there is nothing here worth winding down — no simulation, no state, nothing to save — so for
this one program that is the honest answer rather than a workaround.
