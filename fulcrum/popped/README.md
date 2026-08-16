# Popped

Hot-air balloons drift up across the sky. In the baskets are small round animals, having a
lovely time. When the pointer comes near, they wave at you.

You can pop the balloons.

## Starting it

```
cargo run -p popped --release
```

Click a balloon. Everything else is optional: `m` mutes it, `space` holds it still, `up` and
`down` change the pace, `0` puts the pace back.

## What happens

Nothing rewards you for popping a balloon and nothing asks you to. Here is what you get anyway:

**A beat.** The balloon goes, and for about half a second nobody falls. There is an exclamation
mark, a pair of very wide eyes, and both arms straight up. Then gravity is allowed to start.
That pause is the entire joke; without it the animals merely fall, which is not funny.

**Screaming.** All the way down, at a pitch set by the animal: bears are funnier low, frogs are
funnier high.

**A landing.** They bounce (frogs best, bears worst), sit down hard, and see stars for a couple
of seconds. Then they get up, dust themselves off, shake a fist in your general direction, and
walk off the edge of the screen.

**Occasionally, dignity.** One animal in eight lands on its feet and takes a bow. One in seven
is carrying a parachute, screams for a second, remembers it, and comes down at a stroll.

**Chain reactions.** A falling animal pops any balloon it happens to pass through, which is how
one click sometimes empties a quarter of the sky. This is the funniest thing in the piece and
nobody wrote it.

**A running note of what you have done.** The line at the top starts at "a lovely day for a
balloon ride" and does not stay there.

Nobody is hurt. That matters to the joke: the comedy is the indignity, and indignity requires
everybody to be fine. They are all fine. They are furious, but they are fine.

## How it is put together

The simulation in `src/game.rs` is pure logic, with no idea what any of this looks like. An
animal is a small state machine with a timer on it: riding, the beat, falling, parachuting,
dazed, bowing, and walking off. Popping a balloon does nothing but cut the basket loose and
start everybody's beat; the rest follows from that.

The binary builds each animal out of parts every frame: a body, a head, two ears, a muzzle, four
limbs and a face. That is why one set of shapes gives five species in eight colours pulling four
expressions, and why the same rabbit can wave, realise, scream, bounce and stomp off without a
single frame of animation being authored anywhere. The arms turn about the shoulder, with the
left arm's angle mirrored from the right, which is the difference between waving and appearing
to swat yourself in the face.

The art and the noises are generated, not drawn or recorded:

```
python3 tools/gen_popped_art.py
```

That writes twenty-three shapes and six sounds into `assets/`. The sounds are all pitch curves:
a pop is a pitch that falls off a cliff, a boing is one that bounces, a scream is a wailing saw
with heavy vibrato, and the noise a burst balloon makes going round the sky is a square wave
with the flutter of escaping air.

## Tests

```
cargo test -p popped
```

`tests/determinism.rs` is the gate: the same seed and the same input twice, bit-identical both
times. `tests/flight.rs` holds the piece to its promises, including that a rig is counted
exactly once, that a click pops exactly the balloon under it, that **nobody falls until they
have had their moment**, that everybody who goes up is accounted for, and that everybody who
comes down lands and walks it off.
