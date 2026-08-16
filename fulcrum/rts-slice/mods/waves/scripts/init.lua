-- The base game's wave director, in Lua on purpose: game logic scripting is a first-class
-- Fulcrum tool, not just a modding afterthought. Emits spawn_wave events the Rust sim consumes.

local WAVE_EVERY = 420 -- ticks between attacks (7 s at 60 Hz)
local wave_number = 0

fulcrum.on_tick(function(tick)
    if tick == 0 or tick % WAVE_EVERY ~= 0 then
        return
    end
    wave_number = wave_number + 1
    local count = 4 + math.min(wave_number * 2, 12)
    -- Attackers muster on the right edge, at a deterministically random height.
    local y = math.random(-500, 500)
    fulcrum.emit("spawn_wave", {
        kind = "soldier",
        team = 2,
        count = count,
        x = 900,
        y = y,
        -- March on the player's staging ground.
        target_x = -700,
        target_y = 0,
    })
    fulcrum.log("wave " .. wave_number .. ": " .. count .. " attackers")
end)
