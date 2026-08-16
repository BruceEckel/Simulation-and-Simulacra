-- Sample mod: a third unit type (units/berserker.unit.ron, discovered through the VFS — the
-- game never heard of it) plus a scripted on-death explosion. Delete this directory and both
-- vanish; no game code changes either way.

-- Reinforce the player with berserkers periodically.
fulcrum.on_tick(function(tick)
    if tick > 0 and tick % 600 == 0 then
        fulcrum.emit("spawn_wave", {
            kind = "berserker",
            team = 1,
            count = 3,
            x = -850,
            y = -200,
            target_x = -600,
            target_y = 0,
        })
        fulcrum.log("berserker reinforcements!")
    end
end)

-- Berserkers go out with a bang: the game turns spawn_effect events into particles + audio.
fulcrum.on_event("unit_died", function(event)
    if event.kind == "berserker" then
        fulcrum.emit("spawn_effect", { effect = "effects/explosion.fx.ron", x = event.x, y = event.y })
    end
end)
