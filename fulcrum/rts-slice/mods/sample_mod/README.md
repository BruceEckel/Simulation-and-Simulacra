# sample_mod: Berserkers

A complete worked example of Fulcrum's two modding surfaces, in ~40 lines:

1. **Data**: `units/berserker.unit.ron` adds a third unit type. The game discovers unit
   definitions by listing `units/*.unit.ron` through the layered VFS, and this mod's files are
   mounted on top of the base game's — so the berserker shows up in the roster without the game
   knowing this mod exists. (A file with the *same* name as a base one would shadow it: that's
   how rebalance mods work.)
2. **Script**: `scripts/init.lua` spawns berserker reinforcements for the player every 10
   seconds (`fulcrum.on_tick` + `fulcrum.emit`), and listens for the game's `unit_died` event to
   fire an explosion effect when a berserker falls (`fulcrum.on_event`). Everything a script does
   runs inside the deterministic simulation tick — mods record and replay like any other input.

Delete this directory and the berserker (and its explosion) vanish cleanly; put it back and
they return. No game code changes in either direction.

## Making your own

```
mods/my_mod/
  mod.ron              <- Mod(id: "my_mod", name: "...", version: "0.1.0",
                              scripts: ["scripts/init.lua"])
  scripts/init.lua     <- fulcrum.on_init / on_tick / on_event hooks
  <any asset dirs>     <- mounted into the VFS above the base game
```

See `crates/fulcrum-mod/src/lua_api.md` for the full scripting API.
