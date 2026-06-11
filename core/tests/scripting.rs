use apostasy_core::objects::Object;
use apostasy_core::objects::components::transform::Transform;
use apostasy_core::objects::systems::{DeltaTime, EngineTimer};
use apostasy_core::objects::world::World;
use apostasy_core::scripting::{Script, ScriptEngine, script_update};

/// Writes a script to a unique temp file and returns its path
fn temp_script(name: &str, src: &str) -> String {
    let path = std::env::temp_dir().join(format!("apostasy_test_{}.rhai", name));
    std::fs::write(&path, src).unwrap();
    path.to_str().unwrap().to_string()
}

fn test_world() -> World {
    let mut world = World::default();
    world.insert_resource(DeltaTime(0.016));
    world.insert_resource(EngineTimer(0.0));
    world.insert_resource(ScriptEngine::new());
    world
}

#[test]
fn update_runs_every_frame_and_can_move_a_transform() {
    let path = temp_script(
        "move",
        r#"
        fn update() {
            translate(me(), vec3(1.0, 0.0, 0.0));
        }
    "#,
    );

    let mut world = test_world();
    let id = world.add_object(
        Object::new()
            .add_component(Transform::default())
            .add_component(Script::from_path(path)),
    );

    // five frames, the script nudges x by one each time
    for _ in 0..5 {
        script_update(&mut world).unwrap();
    }

    let t = world
        .get_object(id)
        .unwrap()
        .get_component::<Transform>()
        .unwrap();
    assert!(
        (t.local_position.x - 5.0).abs() < 1e-4,
        "x was {}",
        t.local_position.x
    );
}

#[test]
fn start_runs_once_and_can_spawn() {
    let path = temp_script(
        "spawn",
        r#"
        fn start() {
            let child = create("spawned");
            set_position(child, 1.0, 2.0, 3.0);
            add_component(child, "Transform");
        }
    "#,
    );

    let mut world = test_world();
    world.add_object(Object::new().add_component(Script::from_path(path)));

    // run several frames, start should only fire once so we end with exactly one spawned child
    for _ in 0..3 {
        script_update(&mut world).unwrap();
    }

    let spawned = world
        .get_all_objects()
        .iter()
        .filter(|(_, o)| o.name == "spawned")
        .count();
    assert_eq!(spawned, 1, "start ran the wrong number of times");
}

#[test]
fn get_and_set_component_round_trips_through_yaml() {
    let path = temp_script(
        "reflect",
        r#"
        fn update() {
            let t = get_component(me(), "Transform");
            t.local_position = [9.0, 0.0, 0.0];
            set_component(me(), "Transform", t);
        }
    "#,
    );

    let mut world = test_world();
    let id = world.add_object(
        Object::new()
            .add_component(Transform::default())
            .add_component(Script::from_path(path)),
    );

    script_update(&mut world).unwrap();

    let t = world
        .get_object(id)
        .unwrap()
        .get_component::<Transform>()
        .unwrap();
    assert!(
        (t.local_position.x - 9.0).abs() < 1e-4,
        "x was {}",
        t.local_position.x
    );
}
