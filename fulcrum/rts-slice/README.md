# rts-slice

The phase-4 milestone game: an RTS slice exercising every power feature at once.

- **Data-driven units** — stats live in `units/*.unit.ron`, discovered through the layered VFS;
  `mods/sample_mod` adds a third unit type without touching game code (see its README).
- **Spatial + pathfinding** — a `NavGrid` built from the tilemap; right-click orders compute one
  flow field per command that a whole group shares, with `query_circle` separation steering and
  `nearest` target acquisition.
- **Lua as a game-logic tool** — attacker waves are directed by `mods/waves/scripts/init.lua`
  in the base game, not Rust.
- **Command-level replays** — selection and camera are local/cosmetic; only `move` commands and
  events enter the simulation. Press **R** to save the battle so far as `rts-slice.freplay`;
  watch it back with `cargo run -p rts-slice --release -- --replay rts-slice.freplay`.

## Controls

| Input            | Action                                  |
| ---------------- | --------------------------------------- |
| Left drag        | Select units (click selects under cursor) |
| Right click      | Order selected units to move            |
| WASD/arrows/edge | Pan camera                              |
| Scroll           | Zoom                                    |
| R                | Save replay of the battle so far        |

`FULCRUM_AUTOPILOT=1` scripts a selection + orders for screenshot-driven verification.

## Baselines

`cargo test -p rts-slice --release -- --nocapture` prints the living perf numbers: a 230-unit
battle simulates at ~0.4 ms/tick (budget: 8 ms), and the 2,000-tick scripted battle records and
plays back hash-clean.
