# Avalanche

A table of sand with one rule.

**Any cell holding four grains or more gives one to each of its four neighbours.** Grains that go
off the edge are gone. That is all of it.

## Starting it

```
cargo run -p avalanche --release
```

The table feeds itself. A grain lands somewhere at random whenever the table is still, and
whatever happens next is measured. You do not have to touch it.

| | |
| --- | --- |
| hold the pointer | pour sand where you point |
| right-click | a handful |
| `f` | load every cell to three, one short of toppling everywhere |
| `r` | sweep the table clean |
| `x` | forget the measurements and start counting again |
| `t` | stop the table feeding itself |
| `c` | change the colours |
| `h` | hide the histogram |
| `space` `up` `down` `0` | still, faster, slower, normal |

## What to watch

**The number at the top left.** It is the average height of the pile, and it goes to about 2.1
grains a cell and stays there. The table starts at two, so you watch it climb. Press `f` and it
starts at three, so you watch it come down to the same place. Press `r` and it starts at zero and
climbs to the same place from the bottom. Nothing in the rule mentions 2.1. The pile picks it.

That is what "self-organised criticality" means, and the sandpile is the model Bak, Tang and
Wiesenfeld used to name it in 1987. The pile arranges itself, without being told to, into exactly
the state where it is most sensitive: one more grain can do nothing at all, or take a third of the
table with it, and there is no way to tell which from looking.

**The histogram at the top right.** How many avalanches there have been of each size, on log axes.
It comes out a straight line. A straight line on log axes means a power law, and a power law means
there is no typical size: no bump anywhere to point at and call an average avalanche. Earthquakes
do this. Forest fires do this. Nothing in the rule asked for it.

The fitted slope is printed above the panel. It moves around while the first few hundred
avalanches come in, and then settles.

Only undisturbed avalanches are counted. If you pour on the table while something is already
moving, that avalanche is watched but not measured, because a measurement of a pile that somebody
kept dropping sand on is not a measurement of anything.

**The pale front.** Cells that have just toppled flare and fade within a fifth of a second, so
what you see crossing the table is the leading edge of the avalanche rather than a stain where it
has been. Big ones take thousands of waves and several seconds to cross.

## Every number in it is an integer

There is no floating point anywhere in the simulation. Grains are counts, toppling is subtraction,
and the histogram bins come out of `ilog2`. Two runs of the same seed and the same input therefore
agree exactly, not to within rounding, and they would agree on a machine with a different floating
point unit or none at all. The curve fitting lives in the binary, where being approximate is
somebody else's problem.

That makes this the one piece in this repository whose determinism does not depend on the shape of
anybody's `f32`.

## The theorem worth knowing

Drop the same grains in a different order and the table ends up in **exactly** the same state. Not
approximately: identically, cell for cell. Drop them all at once before anything is allowed to
topple and you still get the same table. The pile is abelian, which is why it is called the
abelian sandpile, and `tests/pile.rs` checks it three ways round.

What does change is the picture on the way, which is why the toppling here is done a wave at a
time: every unstable cell goes at the same moment, then the ones that just became unstable go, and
so on. Same answer, watchable.

## The art

```
python3 tools/gen_avalanche_art.py
```

Two shapes and three sounds. There is almost nothing to draw: the picture is nineteen thousand
cells of flat colour, and what makes it read as sand rather than as a spreadsheet is a slight
grain in each cell and a palette that is mostly dark.

## Tests

```
cargo test -p avalanche
```

`tests/determinism.rs` is the gate. `tests/pile.rs` holds the rule to its promises: a full cell
gives one grain to each neighbour and nothing to the diagonals, every grain is accounted for on
the table or off the edge, the order the grains arrive in does not matter, the pile finds the same
level from above and from below, and the avalanche sizes fit a straight line on log axes.
