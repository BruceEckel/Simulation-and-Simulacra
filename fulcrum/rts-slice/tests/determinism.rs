//! Milestone acceptance: a 2,000-tick scripted battle — move commands, Lua-directed waves,
//! mod-spawned berserkers, combat — runs bit-identically for the same seed, holds the tick
//! budget at 200+ units, and loses the third unit type cleanly when sample_mod is deleted.

use fulcrum::prelude::*;
use rts_slice::game::{self, GamePlugin, Health, Mobility, Team, UnitDefs, UnitKind};

fn mods_dir() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/mods").to_string()
}

fn build_app(seed: u64, record: bool, mods: &str) -> Fulcrum {
    let app = Fulcrum::with_config(FulcrumConfig {
        title: "rts-slice".into(),
        seed,
        record_replays: record,
        ..Default::default()
    })
    .insert_resource(AssetServer::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets"
    )))
    .with_plugin(ScenePlugin)
    .with_plugin(SpatialPlugin { cell_size: 64.0 })
    .with_plugin(ModPlugin::from_dir(mods));
    game::register_components(app).with_plugin(GamePlugin)
}

/// Order every mobile player unit to `(x, y)` — the same command the right-click UI sends.
pub fn order_army(world: &mut World, x: f32, y: f32) {
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

/// The scripted battle: push the army through the wall gap into the attacker waves.
pub fn script(world: &mut World, tick: u32) {
    match tick {
        120 => order_army(world, 150.0, -240.0), // through the lower gap
        900 => order_army(world, 450.0, 0.0),    // press toward the enemy muster
        1500 => order_army(world, -400.0, 100.0), // fall back
        _ => {}
    }
}

type UnitPrint = (u32, u32, i32, String, u8);

fn fingerprint(world: &mut World) -> Vec<UnitPrint> {
    let mut query = world.query::<(&Transform2D, &Health, &UnitKind, &Team)>();
    let mut units: Vec<_> = query
        .iter(world)
        .map(|(transform, health, kind, team)| {
            (
                transform.translation.x.to_bits(),
                transform.translation.y.to_bits(),
                health.current,
                kind.0.clone(),
                team.0,
            )
        })
        .collect();
    units.sort_unstable();
    units
}

fn run_battle(seed: u64, ticks: u32) -> (Vec<UnitPrint>, f64, usize) {
    let mut app = build_app(seed, false, &mods_dir());
    app.run_startup();
    let mut peak_units = 0;
    let started = std::time::Instant::now();
    for tick in 0..ticks {
        script(app.world_mut(), tick);
        app.tick();
        if tick % 120 == 0 {
            let world = app.world_mut();
            let count = world
                .query_filtered::<(), With<Health>>()
                .iter(world)
                .count();
            peak_units = peak_units.max(count);
        }
    }
    let avg_tick_ms = started.elapsed().as_secs_f64() * 1000.0 / ticks as f64;
    (fingerprint(app.world_mut()), avg_tick_ms, peak_units)
}

#[test]
fn determinism_scripted_battle_2000_ticks() {
    let (a, tick_ms, peak) = run_battle(DEFAULT_SEED, 2000);
    let (b, _, _) = run_battle(DEFAULT_SEED, 2000);
    assert_eq!(a, b, "same seed + same commands must reproduce exactly");
    assert!(!a.is_empty(), "somebody survived the battle");
    assert!(
        a.iter().any(|(.., team)| *team == 2),
        "waves spawned attackers"
    );
    println!("battle: avg tick {tick_ms:.3} ms, peak {peak} units");
}

/// Scale target: ~200 units pathing and fighting inside the 8 ms tick budget (asserted with
/// headroom in release; the printed number is the real baseline).
#[test]
fn two_hundred_units_hold_the_tick_budget() {
    let mut app = build_app(DEFAULT_SEED, false, &mods_dir());
    app.run_startup();
    // Muster ~200 units via the same spawn_wave events the Lua director emits.
    for i in 0..5u32 {
        for (team, x, tx) in [(1u8, -800.0f32, 400.0f32), (2, 800.0, -400.0)] {
            let payload: ron::Value = ron::from_str(&format!(
                r#"{{ "kind": "soldier", "team": {team}, "count": 20, "x": {x}, "y": {}, "target_x": {tx}, "target_y": 0 }}"#,
                -450.0 + i as f32 * 220.0
            ))
            .unwrap();
            app.world_mut()
                .resource_mut::<Events<ModEvent>>()
                .write(ModEvent {
                    name: "spawn_wave".into(),
                    payload,
                });
        }
    }
    app.tick();
    let world = app.world_mut();
    let alive = world
        .query_filtered::<(), With<Health>>()
        .iter(world)
        .count();
    assert!(alive >= 200, "expected 200+ units, got {alive}");

    let started = std::time::Instant::now();
    const TICKS: u32 = 600;
    for _ in 0..TICKS {
        app.tick();
    }
    let avg_ms = started.elapsed().as_secs_f64() * 1000.0 / TICKS as f64;
    println!("{alive} units: avg tick {avg_ms:.3} ms over {TICKS} ticks");
    if !cfg!(debug_assertions) {
        assert!(avg_ms < 8.0, "tick budget blown: {avg_ms:.3} ms >= 8 ms");
    }
}

#[test]
fn deleting_sample_mod_removes_the_third_unit_type() {
    // A mods dir with only the wave director.
    let trimmed = std::env::temp_dir().join("rts-slice-mods-trimmed");
    let _ = std::fs::remove_dir_all(&trimmed);
    std::fs::create_dir_all(trimmed.join("waves")).unwrap();
    for entry in ["mod.ron", "scripts/init.lua"] {
        let from =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/mods/waves")).join(entry);
        let to = trimmed.join("waves").join(entry);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(from, to).unwrap();
    }

    let mut without = build_app(1, false, trimmed.to_str().unwrap());
    without.run_startup();
    let kinds = |app: &mut Fulcrum| {
        let mut kinds: Vec<String> = app
            .world()
            .resource::<UnitDefs>()
            .0
            .keys()
            .cloned()
            .collect();
        kinds.sort();
        kinds
    };
    assert_eq!(kinds(&mut without), ["soldier", "worker"]);

    // Full mods dir: the berserker is back, no code change.
    let mut with = build_app(1, false, &mods_dir());
    with.run_startup();
    assert_eq!(kinds(&mut with), ["berserker", "soldier", "worker"]);
}
