# giraffe

A spatial prisoner's dilemma in which some of the players say what they need,
and a dial for how often needs truly conflict.

## The idea

A board of small tiles, sixteen thousand of them.
Every tile plays a round of the prisoner's dilemma with each of its eight neighbours:
share, and two sharers each get something;
take, and a taker gets more from a sharer, who gets nothing;
two takers get nothing.
Then every tile looks at the nine scores around it, its own included, and takes up the strategy of whichever scored highest.
That is Nowak and May's model from 1992, and on its own it makes patterns of sharers and takers that keep shifting.

This board adds one thing to it.
A tile also has a voice.
Most of them are jackals, in Marshall Rosenberg's word for it:
what they say tells the other party nothing about what they need.
A giraffe says what it needs, and pays a little for it every time, whether anyone is listening or not.
When two giraffes meet, they find out which game they are in.
Some of the time their needs truly conflict and they play the dilemma they would have played in silence.
The rest of the time the needs were compatible all along, and both come away with more than sharing would have given them:
the whole orange each, instead of half.

How often needs truly conflict is the dial under the scroll wheel.
It starts at half.
Turn it down and the world is Rosenberg's, where most conflicts are misunderstandings, and a patch of giraffes grows until it has all but taken the board.
Turn it up and speaking is a cost with nothing behind it, and the same patch is eaten by its neighbours.
Somewhere between is the line, and the piece does not say where the real world sits on that dial.
It lets you set it, plant a patch of either kind, and watch.

The colours are the same in every palette.
The warm tiles say what they need;
the cool ones do not.
The lighter of each pair shares, the darker takes.
So the lightest, warmest colour on the board is always the sharer who says what it needs.

There is no score, no timer, and no way to lose.
Every generation takes most of a second and blooms across the board rather than flipping.
A change of pace, of temptation or of the dial is a slide, not a jump.
Now and then a small patch of something appears on its own, so the board does not quite settle, and so a giraffe turns up eventually whether or not you plant one.

## Working it

Everything worth doing is done with the mouse, and you can find all of it by playing.

| | |
|---|---|
| move | a lamp goes with the pointer; under it, the tiles that are scoring are the bright ones |
| press | plant a patch of giraffes; hold for a bigger patch, drag to paint a swathe |
| right press | plant a patch of jackals the same way |
| scroll | the dial: how often needs truly conflict |

The pointer is hidden over the board.
The lamp is where the pointer is.

| | |
|---|---|
| `up` / `down` | hold for quicker or slower generations |
| `left` / `right` | hold for less or more temptation to take |
| `space` | pause, and go on |
| `c` | the next palette: dusk, lagoon, ember, moss, moon, aurora |
| `r` | a new board |
| `h` | the hint, on or off; it goes away on its own after a while |
| `F11` | the whole display, and back to a window |

It opens in an ordinary window.
`F11` takes the display with no border, and `F11` again gives the window back.

Source, and the long version with the argument in it, in [`fulcrum/giraffe`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/giraffe).
