# Jig

A skeleton dances. Nobody animated it: somebody moves its hips in a small circle, and eighteen
coupled pendulums work out the rest.

```
cargo run -p jig --release
```

## The idea

The pelvis is the only thing in this program that is *told* what to do. It traces a closed
curve — two sine waves, three numbers each — at whatever tempo you set. Every other bone is a
rigid rod hanging off a joint, and a rod hanging off a joint is a pendulum.

Take one bone with nothing hanging off it and it obeys exactly this and nothing else:

```
θ¨ = −(3 / 2L) · [ (g + a_y)·sin θ + a_x·cos θ ]
       ╰── a rod pivoted at one end ──╯
```

The first part is a uniform rod pivoted at one end, which is a pendulum of effective length
`2L/3`. The second is the only way the dance ever reaches a bone: `a` is the acceleration of the
joint it hangs from, and it arrives as a fictitious force, exactly the way a bus pulling away
tips you over. The pelvis shakes the spine, the far end of the spine shakes the collar bone, the
collar shakes the upper arm — and by the time it reaches the fingers there is nothing left of
the tidy little circle the hips were drawing.

Every other bone in a body has things hanging off it, and **they pull back**. A forearm flung
outwards tugs on the upper arm, which tugs on the collar, which rocks the chest. That is the
difference between a body and a mobile, and it is why this is not eighteen separate sums: the
whole skeleton is one system, and every step solves

```
M(θ) θ¨ = r(θ, θ˙)
```

for all eighteen angles at once. Two pendulums coupled like that are the standard example of a
chaotic system; this one has eighteen, and it is why the dance never comes round again.

## Working it

| | |
|---|---|
| `up` / `down` | tempo, from 200 beats a minute all the way down to no music at all |
| `left` / `right` | tone: how hard the joints hold the pose |
| `1`–`5` | the step the hips are dancing |
| `R` | stand up straight again |
| `P` | palette, `M` mute, `H` hide the readout |

**Tone is the interesting knob.** A standing spine is an *inverted* pendulum: gravity is not
holding it up, it is trying to fold it. What holds it is tone, the torsion spring at each joint,
and tone is measured here against exactly the torque gravity applies to the limb that joint
carries. So `1.0` is not a tuning constant. It is the point at which a joint can just balance its
own limb, and — the same balance seen the other way — the point at which the pose stops being a
thing the body is falling away from and starts being a thing it comes back to.

Hold `left` and watch the skeleton go through it. Above one it dances. Below one it cannot hold
itself up at all, and folds onto its own joint stops like a coat coming off a hook.

Tone also decides whether the dance repeats. Wind it up and the body finds a groove: a driven
system with somewhere to put its energy settles onto a cycle, and after a few bars the skeleton
is in the same place at the same point of every bar. Take the tone out and the cycle goes, and
two skeletons that started a ten-thousandth of a radian apart are doing visibly different dances
fifteen seconds later. Both of those are tested.

## The steps

Each is two sine waves and nothing else. Everything on screen that is not a small closed curve
is the skeleton's answer, not part of the question.

| | | |
|---|---|---|
| `1` | **Sway** | side to side, once a beat |
| `2` | **Bob** | straight up and down |
| `3` | **Figure of eight** | across once a beat, up and down twice — what hips actually do |
| `4` | **Round** | a plain circle, the two axes a quarter beat apart |
| `5` | **Shiver** | a hand's width, five times a beat |

The shiver is there for a specific reason. Bob a pendulum's pivot fast enough and *upside down*
stops being the way it falls and becomes somewhere it will sit: **Kapitza's pendulum**, one of
the stranger results in classical mechanics. The condition is

```
(A ω)² > 2 g ℓ,      ℓ = 2L/3
```

and what matters is `A·ω`, so a small fast bob buys it far more cheaply than a big slow heave.
The readout carries that number for an upper arm. On the shiver it goes over one, and if you
take the tone down at the same time — so that nothing is pulling the arms back down — the
skeleton throws them into the air and holds them there. Physics is doing that. There is no
choreographer anywhere in this program.

## The numbers

**Real gravity**, at 222 world units to the metre, which makes the skeleton a hair under six
feet. That is not decoration. A bone of an arm's length then swings with an arm's period,
because it is the same pendulum; fake the gravity and the whole dance goes syrupy or frantic and
no amount of fiddling with anything else puts it right. The proportions matter for the same
reason — a forearm swings at nearly twice the rate of a thigh, and it is that ratio of rates,
rather than the shapes, that makes the flailing read as a body flailing.

**Forty-eight substeps a tick**, about a kilohertz and a half. The drive reaches twenty-four
cycles a second on the shiver and its acceleration goes as the *square* of the tempo, so a
sixtieth of a second is far too coarse a step: the skeleton gains energy out of nowhere and
tears itself apart. The count is a constant rather than a measurement of how fast the machine
is, because a replay has to do the same arithmetic in the same order. The whole body costs about
0.09 ms a tick.

**A stiffer trunk than limbs.** A stack of inverted joints is weaker than any one of them,
because they can all bow the same way at once, and the trunk here is four bones in a row. So the
trunk's joints are given several times the tone of a limb's. That is a fact about standing up
rather than a fudge: a spine really is held far more firmly than a shoulder, and a body built
the other way round could do neither job.

**Ligaments, not walls.** Every joint has a range, and past the end of it there is a stiff spring
rather than a hard stop. A hard stop would be simpler and it would be wrong twice over: it erases
whatever the joint was doing, which quietly kills the chaos, and it lets a joint rest against its
limit and rattle at the substep rate. The knocks you hear are the moment a joint *arrives*.

## What is tested

`tests/pendulums.rs` checks the physics against physics rather than against last week's output:

- a bone left alone swings at the period the textbook gives a uniform rod, at four lengths;
- a bone whose joint is shaken hard enough stands on its head, and one shaken too gently falls
  over, at the speeds Kapitza's condition says on each side of it;
- **with the tone off — no spring, no holding, no friction — the body never gains energy.** That
  is the sharp instrument. A mass matrix with a sign wrong in it, or a missing centrifugal term,
  does not look wrong on screen. It looks *livelier*, which is worse;
- two limbs are coupled only through the body they share, and the mass matrix is symmetric;
- over one, tone holds the pose; under one, it cannot;
- a loose skeleton never dances the same bar twice, and a taut one does;
- and nothing, at any tempo or tone or step, is ever pushed far past a stop or torn apart.

`tests/determinism.rs` is the CI gate. Nothing here is random — the dance looks improvised
because a system of coupled pendulums has no short way of saying what it will do next, not
because anything rolled a die.
