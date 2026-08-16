//! Milestone acceptance: record the 2,000-tick scripted battle, then play the `.freplay` back
//! in a fresh app — every embedded state hash (33 checkpoints + final) must reproduce, mods,
//! Lua waves, flow fields, combat and all.

use fulcrum::prelude::*;
use rts_slice::game::{self, GamePlugin, Health, Mobility, Team};

fn build_app(record: bool) -> Fulcrum {
    let app = Fulcrum::with_config(FulcrumConfig {
        title: "rts-slice".into(),
        record_replays: record,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(ScenePlugin)
    .with_plugin(SpatialPlugin { cell_size: 64.0 })
    .with_plugin(ModPlugin::from_dir(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/mods"
    )));
    game::register_components(app).with_plugin(GamePlugin)
}

fn order_army(world: &mut World, x: f32, y: f32) {
    let units: Vec<u64> = {
        let mut query = world.query_filtered::<(Entity, &Team), (With<Mobility>, With<Health>)>();
        query
            .iter(world)
            .filter(|(_, team)| team.0 == 1)
            .map(|(entity, _)| entity.to_bits())
            .collect()
    };
    let payload = ron::to_string(&game::MoveCommand { units, x, y }).unwrap();
    world.resource_mut::<CommandOutbox>().send("move", payload);
}

#[test]
fn replay_recorded_battle_plays_back_hash_clean() {
    let dir = std::env::temp_dir().join("rts-slice-replay");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("battle.freplay");

    let mut app = build_app(true);
    app.run_startup();
    for tick in 0..2000 {
        match tick {
            120 => order_army(app.world_mut(), 150.0, -240.0),
            900 => order_army(app.world_mut(), 450.0, 0.0),
            1500 => order_army(app.world_mut(), -400.0, 100.0),
            _ => {}
        }
        app.tick();
    }
    app.save_replay(&path).unwrap();

    let replay = Replay::load(&path).unwrap();
    assert_eq!(replay.ticks.len(), 2000);
    assert!(
        replay.state_hashes.len() >= 34,
        "expected 34 checkpoints, got {}",
        replay.state_hashes.len()
    );
    assert!(
        replay
            .ticks
            .iter()
            .any(|t| t.commands.iter().any(|c| c.name == "move")),
        "move commands were recorded"
    );
    assert_eq!(
        replay.header.mods,
        vec![
            ("waves".to_string(), "1.0.0".to_string()),
            ("sample_mod".to_string(), "1.0.0".to_string()),
        ],
        "the loaded mod set travels in the header"
    );

    build_app(false)
        .run_replay(&path)
        .expect("battle playback must reproduce every embedded state hash");
}
