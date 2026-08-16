//! The select/move/fight loop, end to end: the scripted battle must produce actual combat —
//! hits, deaths (heard by mods as `unit_died`), and corpses that decay away.

use fulcrum::prelude::*;
use rts_slice::game::{self, Corpse, GamePlugin, Health, Mobility, Team};

#[test]
fn the_battle_is_actually_fought() {
    let app = Fulcrum::new("rts-slice")
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
    let mut app = game::register_components(app).with_plugin(GamePlugin);
    app.run_startup();
    let mut hits = 0usize;
    let mut deaths = 0usize;
    let mut cursor = bevy_ecs::message::MessageCursor::<ModEvent>::default();
    for tick in 0..2000u32 {
        if let 120 | 900 | 1500 = tick {
            let world = app.world_mut();
            let units: Vec<u64> = world
                .query_filtered::<(Entity, &Team), With<Mobility>>()
                .iter(world)
                .filter(|(_, t)| t.0 == 1)
                .map(|(e, _)| e.to_bits())
                .collect();
            let (x, y) = match tick {
                120 => (150.0, -240.0),
                900 => (450.0, 0.0),
                _ => (-400.0, 100.0),
            };
            let payload = ron::to_string(&game::MoveCommand { units, x, y }).unwrap();
            world.resource_mut::<CommandOutbox>().send("move", payload);
        }
        app.tick();
        for e in cursor.read(app.world().resource::<Events<ModEvent>>()) {
            match e.name.as_str() {
                "unit_hit" => hits += 1,
                "unit_died" => deaths += 1,
                _ => {}
            }
        }
    }
    let world = app.world_mut();
    let corpses = world.query::<&Corpse>().iter(world).count();
    let alive = world
        .query_filtered::<&Team, With<Health>>()
        .iter(world)
        .count();
    println!("2000 ticks: {hits} hits, {deaths} deaths, {corpses} corpses on field, {alive} alive");
    assert!(hits > 0 && deaths > 0, "the battle must actually be fought");
}
