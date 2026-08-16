# jig

A dancing skeleton: shake the hips, and eighteen pendulums work out the rest.

## The idea

The pelvis is the only thing in this program that is *told* what to do. It traces a small closed
curve, two sine waves with three numbers each, at whatever tempo you set. Every other bone is a
rigid rod hanging off a joint, and a rod hanging off a joint is a pendulum. The dance reaches a
bone only as the acceleration of the joint it hangs from, arriving as a fictitious force, in the
way a bus pulling away tips you over.

Every bone in a body has things hanging off it, and they pull back. A forearm flung outwards
tugs on the upper arm, which tugs on the collar, which rocks the chest. So this is not eighteen
separate sums: the whole skeleton is one system, and every step solves for all eighteen angles
at once. Two pendulums coupled like that are the standard example of a chaotic system; this one
has eighteen, which is why the dance never comes round again.

Nothing here is animated. By the time the hips' tidy little circle has reached the fingers,
there is nothing left of it.

## Working it

| | |
|---|---|
| `up` / `down` | tempo, from 200 beats a minute all the way down to no music at all |
| `left` / `right` | tone: how hard the joints hold the pose |
| `1`-`5` | the step the hips are dancing |
| `R` | stand up straight again |
| `P` | palette, `M` mute, `H` hide the readout |

Source, and the long version, in [`fulcrum/jig`](https://github.com/BruceEckel/Simulation-and-Simulacra/tree/main/fulcrum/jig).
