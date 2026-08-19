# life

Conway's Game of Life and forty-three of its relatives, from cells you could count with your
finger down to one cell per physical pixel.

## The idea

Life is four rules on a grid of squares, and John Conway published them in 1970. A dead cell with
exactly three live neighbours comes to life. A live cell with two or three stays. Everything else
dies, of loneliness or of crowding. That is the whole of it, and out of it come still lifes,
blinkers, gliders that walk across the field, and a gun that fires them forever.

The interesting part is that Life is one member of a very large family, and the family is
reachable by turning four dials.

**Which counts give birth, and which let a cell survive.** Write them as `B3/S23` and you have
Life. `B36/S23` is HighLife, which has a pattern that copies itself. `B2/S` is Seeds, where
nothing survives its own generation and everything detonates. `B3/S012345678` is a world where
nothing ever dies. There are two hundred and sixty thousand of these and a few dozen have been
found worth naming; twenty-two of them are here.

**How many states a cell has.** In Life a cell is alive or empty. Give it more, and a cell that
fails its survival test does not die — it ages through further states first, and while it does it
is in the way but is not a neighbour. That single addition is the difference between Life, where
most things eventually stand still, and Brian's Brain, where nothing holds its place for one
generation and the whole field is wavefront. Fifteen of those.

**How wide the neighbourhood is.** Life counts eight neighbours. Widen it to a radius of five and
you are counting a hundred and twenty-one, the thresholds become bands rather than digits, and
what comes out stops looking mechanical: Bosco's rule has hollow crawling *bugs* where Life has
gliders. Seven of those.

## What to look at

Open it, leave it on Life, and watch the field settle: the busy places glow because a cell born
this generation is drawn bright and fades to the palette's own colour as it holds. When it has
stopped going anywhere it says so — *still*, or *period 2* — and then sows itself again.

Then, in roughly this order:

- `Tab` twice, to **Brian's Brain**. Nothing in it ever stops.
- `M` once more, to **Star Wars**: the same rule with something to stand on, so guns hold their
  ground and fire photons across the field.
- `Tab` again, to **Bugs** — Bosco's rule, at radius five. Hollow things that crawl.
- `4`, for a **symmetric** start. Every rule here treats the four reflections alike, so a field
  that starts symmetric stays symmetric for as long as it runs. It costs nothing and it makes any
  of these rules beautiful.
- `8`, for **Gosper's glider gun**, and then `Space` and `S` to walk it one generation at a time.
- `X` held down, to go to **one cell per pixel**. The rule does not change; the grain does. A
  full display of Life is around four million cells and it is a different thing to look at.

## Working it

It opens in an ordinary window. `F11` takes the whole display with no border, and `F11` again
gives the window back. It keeps running either way, and the pattern on it survives the change.

| | |
|---|---|
| `N` / `M` | the rule before / after this one — hold either down to walk |
| `Tab` | jump to the next family |
| `1` – `0` | how the field is started (below) |
| `R` | sow it again |
| `C` | empty it |
| `Space` | run / hold |
| `S` | one generation, while it is held |
| `up` / `down` | faster / slower, from a quarter of a generation a second to four hundred and eighty |
| `Z` / `X` | bigger / smaller cells, from sixty-four pixels down to one — the wheel does it too |
| `O` / `P` | the colour scheme before / after this one, of twelve |
| `A` | colour live cells by how long they have been alive |
| `G` | ghost trails, where cells have recently been |
| `E` | the line between cells, when they are big enough to have one |
| `T` | torus / walls: whether the left edge is the right edge's neighbour |
| `K` | whether a field that has settled sows itself again |
| `H` | hide the readout |
| `F11` | fullscreen, and back |
| left mouse | draw live cells |
| right mouse | rub them out |

The number keys:

| | |
|---|---|
| `1` | whatever this rule likes — every one of them carries its own |
| `2` | a soup over the whole field |
| `3` | a soup in a square in the middle |
| `4` | a soup mirrored into all four quadrants, which stays symmetric forever |
| `5` | one live cell |
| `6` | the R-pentomino: five cells, and eleven hundred generations of consequences |
| `7` | the acorn: seven cells that take five thousand generations to settle |
| `8` | Gosper's glider gun |
| `9` | the diehard: seven cells that vanish completely after a hundred and thirty generations |
| `0` | nothing — draw your own |

Those last four are Life's patterns, but they are stamped whatever rule is loaded. An acorn under
Day & Night is not an acorn for very long, and watching what becomes of it is a fair way to learn
what a rule does.

Changing the rule does not clear the field. The same pattern carries straight on under the new
law, which is worth doing on purpose.

## The rules

**Life-like** — Life, HighLife, Day & Night, DryLife, Pseudo Life, 2x2, Move, Long Life, Maze,
Mazectric, Coral, Flakes, Coagulations, Assimilation, Walled Cities, Diamoeba, Amoeba, Anneal,
Seeds, Serviettes, Replicator, Gnarl.

**Generations** — Brian's Brain, Star Wars, Fireworks, Faders, Frogs, Prairie on Fire, Lava,
Burst, Rake, Caterpillars, Bloomerang, Wanderers, Swirl, Banners, Xtasy.

**Larger than Life** — Bugs (Bosco's rule), Majority, Waffle, Globe, Bugsmovie, Modern Art, and
Gnarl over the four orthogonal neighbours.

The rulestrings are the published ones, from Mirek Wojtowicz's lexicon behind
[MCell](https://mcell.ca/), [LifeWiki](https://conwaylife.com/wiki/), Golly's Larger than Life
documentation, and Wikipedia's table of notable Life-like rules. The readout prints the rulestring
of whatever is running, so anything you find here can be looked up.

## What the readout says

The rule and how it is written, then which of the forty-four it is and which family. Then the
generation, the population and what fraction of the field that is, how many cells were born and
died on the last generation, and whether it has fallen into a period. Then the size of the field
in cells and how many pixels one cell is drawn at. Then the pace asked for and the pace actually
achieved, which are not the same number once the field is large: a generation at one cell to the
pixel is millions of cell updates, and asking for four hundred a second does not make a machine
able to do them.

Source, and the long version of the explanation, in
[`fulcrum/life`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/life).
