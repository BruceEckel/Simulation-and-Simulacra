# flutter

A swarm of moths around a lamp: add moths, take them away, and run it at any pace.

## The idea

Two dials, and both go a long way. The swarm runs from nothing to thirty thousand moths, and
the pace from a twentieth of real time to eight times it, while the thing stays one simulation
you can pause and pick a single moth out of.

Every moth is one sprite playing an eight-frame wingbeat, tinted from six dusty coats and shaded
by its own place in the palette, so no two are quite alike. Big moths beat slower and draw in
front of small ones, which is what turns a flat pile of sprites into a swarm with a front and a
back.

A moth's path depends on nothing but that moth. Its wander is read out of a seeded function of
the simulated clock rather than accumulated from random draws, so ten thousand moths can arrive
mid-flight without disturbing the ones already flying.

## Working it

| | |
|---|---|
| `up` / `down` | more moths, fewer moths (hold it: the swarm scales, so it climbs fast) |
| `left` / `right` | slower, faster |
| `0` | back to normal speed |
| `space` | hold everything still |
| move the pointer | the lamp goes with it |
| `l` | put the lamp out, and the swarm comes apart |
| `r` | throw this swarm away and draw a new one |

`w`/`s` and `a`/`d` do the same as the arrows. The readout in the corner has the count, the
pace, the simulated clock, and the frame rate, which is the number that tells you when you have
asked for too many moths.

Source, and the long version, in [`fulcrum/flutter`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/flutter).
