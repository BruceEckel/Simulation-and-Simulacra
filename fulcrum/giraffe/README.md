# giraffe

A spatial prisoner's dilemma in which some of the players say what they need,
and a dial for how often needs truly conflict.

```sh
cargo run -p giraffe --release
```

The note for whoever downloads the executable is [`Windows/giraffe.md`](../../Windows/giraffe.md).
This file is for whoever opened the source:
how the piece is built, and the argument it is built to try.

## What it is for

The brief was a game that puts game theory and Nonviolent Communication on one board, on the hypothesis that the two are connected.
The connection this piece proposes is that NVC is a strategy in a game of incomplete information, and that its parts map onto game-theoretic ones:

- **Observation without evaluation** is common knowledge of the state.
  Two parties who agree on what happened are playing the same game;
  two who have each evaluated it are playing different ones.
  On the board this is given:
  every cell sees the same neighbours and the same scores.
- **A feeling** is a costly signal.
  Saying what something did to you exposes you, and the exposure is the point:
  a signal that costs nothing is cheap talk and nobody need believe it, which is Spence's argument about job markets and Zahavi's about peacocks.
  On the board it is [`Terms::cost`](src/game.rs), paid in every encounter by everyone who speaks, heard or not.
- **A need** is the payoff function, disclosed.
  A prisoner's dilemma is a dilemma because each party knows its own payoffs and guesses at the other's, and guesses the worst.
  Two parties who say what they need have turned a game of incomplete information into one of complete information, and can see what game they are in.
  On the board, a meeting of two giraffes does that.
- **A request, not a demand,** is a proposal with no punishment attached.
  On the board it is the difference between the two speaking strategies:
  the giraffe who shares, and the one who says what it needs and still takes.
- **Rosenberg's claim** is that needs do not conflict, and that what conflicts is the strategies people have chosen to meet them:
  the two children who both want the orange, one for the peel and one for the juice.
  That is a claim about the world, and it is the one thing the piece does not assert.
  It is [`Terms::conflict`](src/game.rs), the dial under the scroll wheel:
  how often, when two parties say what they need, the needs turn out to truly conflict.

So the hypothesis the piece can try is narrower than "NVC works" and sharper:
that saying what you need is a strategy which pays for itself when needs rarely truly conflict and does not when they usually do, and that where the line falls depends on the terms.
You set the dial, plant a patch of one kind or the other, and watch which spreads.

The rest of the brief was the same as [`pond`](../pond)'s:
relaxing, no twitch in it, no goal that could produce anxiety, worked with the mouse, discoverable by playing, and beautiful if possible.
Three things serve that here.
Nothing ends abruptly: a generation blooms across the board over most of a second, a change of pace or of the dial slides, and a palette crossfades.
Nothing can go wrong: there is no score, and a board that has been swept by one kind is a board you can plant the other into.
And left alone it goes on: a small patch of some strategy appears on its own every few seconds, so the board does not freeze into a still picture, and so a giraffe turns up eventually whether or not you plant one.

## How it is built

The crate is split the way the others here are:
[`game.rs`](src/game.rs) is the board and knows nothing about pixels or colour, [`look.rs`](src/look.rs) is the palettes, and [`main.rs`](src/main.rs) draws the board and reads the keys that change nothing about it.
The board runs headless for the tests, and in release a thousand generations take a third of a second.

### The board

Nowak and May's lattice, unchanged:
a grid of cells, each of which plays the prisoner's dilemma with its eight neighbours and adds up what it got.
Two sharers each get one.
A taker gets the temptation from a sharer, who gets nothing.
Two takers get nothing.
Then every cell, all at once, takes up the strategy of whichever of the nine cells around it, itself included, scored highest.
A tie goes to the cell's own strategy, and between neighbours to the first consulted, in a fixed order, so that the rule has no randomness in it.
The board is a torus, so that no cell has fewer neighbours than another and the edge is not a special place.

That rule alone, at the right temptation, makes patterns of sharers and takers that do not settle:
a lone taker becomes a block of nine, the block's corners lose to the sharers beyond them, and what follows is a churn that goes on for ever.
The right temptation is a narrow band, and there is more than one.
On eight neighbours with no self-play, a silent board churns around 1.4, goes quiet with the sharers ahead between 1.5 and 1.58, churns again between 1.6 and 1.65, and from about 1.68 up is swept by takers and freezes.
[`examples/scan.rs`](examples/scan.rs) is how that was found, and 1.6 is where the piece starts.

### The four strategies

A cell is two choices.
A *stance* is what it does when there is something to take:
share, or take.
A *voice* is whether it says what it needs before the two of them act:
a giraffe does, a jackal does not.
Rosenberg's jackal is not silent, but what it says is judgement and demand, which tells the other party nothing about what it needs;
for the arithmetic that is silence.
The two choices make four strategies, and a cell copies both together when a neighbour does better.

[`payoff`](src/game.rs) is the whole of the model.
For what `me` gets from one encounter with `other`, with `k` the conflict dial, `b` the temptation, `g` the bounty and `c` the cost:

| | `other` is a jackal | `other` is a giraffe |
|---|---|---|
| `me` is a jackal | the contest | the contest |
| `me` is a giraffe | the contest, less `c` | `(1 - k)(1 + g)`, plus `k` times the contest, less `c` |

The contest is Nowak and May's table above.
When two giraffes meet they find out which game they are in:
with probability `1 - k` the needs were compatible, there is nothing to take, and each gets one plus the bounty whatever its stance;
otherwise they play the contest.
What is returned is the average, which is a decision worth explaining.
Drawing the outcome each time would make the board a lottery, where a patch grows or dies by luck and the patterns are noise.
Using the average keeps Nowak and May's crisp geometry and makes the dial a slide through a continuous family of games, and a tick of it changes every encounter on the board at once rather than one encounter's fortune.
The board is deterministic;
the seed decides where the takers start and where the drift patches fall, and a generation draws nothing.

What the terms come to, on the starting board:

- A patch of giraffes planted into the silent churn grows while the dial is under about three quarters, more slowly the higher it is set, and is eaten above that.
- A patch of jackals planted into a board of giraffes is held off until about eight tenths and spreads at eight and nine.
  At the very top of the dial it stalls instead, a frozen block in a board of sharers who are paying to say nothing useful:
  the lattice has thresholds of its own, and not every step up the dial is a step in the same direction.
- With all four kinds sprinkled evenly, the silent kinds die out anywhere under about nine tenths, and what is left is the two speaking kinds, at most settings churning between themselves much as the silent board did.
  That is the second thing the board says:
  saying what you need wins whenever conflicts are not nearly universal, and whether you also take is the old dilemma, still running inside the new language.
  At nine tenths and above the speakers die out and the silent churn is all there is.
- Speaking pays a tenth in every encounter, and the bounty is half of a share.
  Between them those put the line where it is;
  the dial starts at half because half is the one setting that expresses no opinion.

### The sources of change

Everything you can do is a way of putting a patch on the board:

- **A press** plants a disc of giraffes, the sharer kind.
  It starts small and grows while the button is held, over a couple of seconds, and it follows the pointer, so a drag paints a swathe.
  The right button does the same with silent takers.
  Planting works while the board is paused, so a still board can be arranged before it is let go.
- **Drift**: every six to fourteen seconds a small patch of some strategy, drawn at random from the four, appears somewhere.
  It keeps the board from settling and it means the giraffe question gets asked whether or not you ask it.
- **The keys** change the terms rather than the board.
  The scroll wheel sets where the dial is wanted and the board's own figure slides after it.
  The arrow keys slide the pace and the temptation while held, so that nothing jumps.

### The picture

Each cell is one small tile, drawn in its strategy's tone.
Every palette is laid out the same way, so that once you have read one you have read all six:
voice is hue, with the ones that speak warm and the silent ones cool;
stance is lightness, with the sharer the lighter of each pair.
The lightest, warmest colour on the board is therefore always the sharer that says what it needs, and the darkest is the silent taker.
[`tests/palettes.rs`](tests/palettes.rs) holds every palette to the rule.

Three things are added to the flat tone:

- A tile is a little brighter for what it scored last generation, so a cell doing well glows and a cell being taken from goes dull.
- Under the lamp that follows the pointer every tile is lifted, and the ones that are scoring are lifted more, so the lamp is a way of reading the board:
  hold it over a boundary and the winning side is the bright one.
- A tile that has just changed strategy is brighter still, fading over the generation, and every tile eases toward its colour rather than jumping to it, so a generation blooms across the board instead of flipping.

The lamp breathes, eleven seconds to a breath, which is five and a half breaths a minute.
Nothing asks you to breathe with it.

### What the keys do not do

Nothing on the keyboard touches the board except `r`, which sows a new one.
The palette, the hint and fullscreen are matters of taste and live in `main.rs`;
the pace, the temptation, the dial and pausing are simulation state and live in `game.rs`, where the determinism test can script them.

## Tests

- `tests/board.rs`:
  the payoffs say what this file says, a lone taker becomes a block of nine, a planted disc is a disc and wraps round the torus, the silent board keeps churning, giraffes spread when needs rarely conflict and die out when they always do, the dial is monotone for a planted patch, and every key that touches the board, and both buttons, do what the note says.
- `tests/determinism.rs`:
  same seed and same scripted input twice, bit-identical both times.
- `tests/palettes.rs`:
  in every palette the speaking tones are warmer than the silent ones and the sharing tones lighter than the taking ones, and the night is darker than any of them.

## Reading

- Nowak, M. A. and May, R. M., "Evolutionary games and spatial chaos", *Nature* 359 (1992), 826–829.
  The lattice, the synchronous best-of-nine rule, and the observation that the temptation decides everything.
- Axelrod, R., *The Evolution of Cooperation* (1984).
  The tournaments, and the four properties of a strategy that does well against many others:
  nice, retaliatory, forgiving, clear.
  The giraffe here is nice and clear;
  whether it is forgiving is a question the board does not ask, since every encounter is one round.
- Rosenberg, M. B., *Nonviolent Communication: A Language of Life* (2003).
  Observation, feeling, need, request;
  the jackal and the giraffe;
  and the claim about needs that the dial stands in for.
- Spence, M., "Job Market Signaling", *Quarterly Journal of Economics* 87 (1973), and Zahavi, A., "Mate selection: a selection for a handicap", *Journal of Theoretical Biology* 53 (1975).
  Why a signal must cost something to be believed.
