# Flutter

A room full of moths circling a lamp, with two dials: how many moths there are, and how fast it
all runs. Both go a long way. The swarm runs from nothing to thirty thousand, and the pace from
a twentieth of real time to eight times it, while the thing stays one simulation you can pause
and pick a single moth out of.

Every moth is one sprite playing an eight-frame wingbeat, tinted from six dusty coats and shaded
by its own place in the palette, so no two are quite alike. Big moths beat slower and draw in
front of small ones, which is what turns a flat pile of sprites into a swarm with a front and a
back.

## Starting it

```
cargo run -p flutter --release
```

Release is worth it once you hold `up` for a few seconds. The room keeps its proportions
whatever shape the window is, so a wide window gets bars rather than a stretched swarm.

## Doing things to it

| | |
| --- | --- |
| `up` / `down` | more moths, fewer moths (hold: the swarm scales, so it climbs fast) |
| `left` / `right` | slower, faster |
| `0` | back to normal speed |
| `space` | hold everything still |
| move the pointer | the lamp goes with it |
| `l` | put the lamp out, and the swarm comes apart |
| `r` | throw this swarm away and draw a new one |

`w`/`s` and `a`/`d` do the same as the arrows. The readout in the corner has the count, the
pace, the simulated clock, and the frame rate, which is the number that tells you when you have
asked for too many moths.

## How the dials work

**Population is a target, not an event.** `Flock::target` says how many moths there should be;
one system moves the target and another makes the world agree with it. Holding a key scales the
target by 3.5% a tick rather than adding a fixed number, which is the only way one key is useful
both at forty moths and at twenty thousand — and it never scales by less than one, so a tap is
always worth exactly one moth.

Moths are spawned with an ordinal and leave by ordinal, highest first, so the swarm is always
ordinals `0..count`. "Take a thousand away" therefore takes away the same thousand every time,
which is what makes the population dial replayable.

**Speed is a step, not a tick rate.** The engine's tick rate is fixed — that is the determinism
promise — so speed here is a multiplier on how far one tick advances. Every system reads
`Step::seconds` instead of `Time::fixed_delta`, wingbeats included, so a moth at 4x flies four
times as far *and* flaps four times as fast, and pausing is simply a step of zero.

**A moth's path depends on nothing but that moth.** Its wander is read out of a seeded function
of the simulated clock rather than accumulated from random draws, so no moth's flight depends on
which other moths exist or on the order the swarm happens to be stored in. That is what lets ten
thousand moths arrive mid-flight without disturbing the ones already flying — there is a test
that cuts a running swarm in half and checks every survivor is exactly where it would have been.

## Tests

```
cargo test -p flutter
```

`tests/swarm.rs` covers the two dials and the lamp; `tests/determinism.rs` is the usual
same-seed-twice gate, scripted to change the population and the pace while it runs.
