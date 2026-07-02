//! Round-trips entities through the worldspace serializer and checks that
//! legacy YAML formats (flattened enums, scalar voxel components, empty-string
//! options) still load, so the serde-derived component persistence can't
//! silently corrupt saved scenes.

use apostasy_core::cgmath::Vector3;
use apostasy_core::ecs::components::get_component_registration;
use apostasy_core::ecs::components::transform::Transform;
use apostasy_core::ecs::world::World;
use apostasy_core::physics::Gravity;
use apostasy_core::physics::collider::{Collider, ColliderShape};
use apostasy_core::physics::velocity::Velocity;
use apostasy_core::rendering::components::camera::Camera;
use apostasy_core::rendering::components::lighting::{Light, LightType};
use apostasy_core::rendering::components::model_renderer::ModelRenderer;
use apostasy_core::worldspaces::worldspace_serializer::{load_cell, serialize_cell};

fn origin() -> Vector3<i32> {
    Vector3::new(0, 0, 0)
}

/// Spawns one entity with every scene-persisted component, saves the cell,
/// loads it into a fresh world, and asserts the re-serialized cell is identical.
#[test]
fn cell_roundtrip_is_lossless() {
    let mut world = World::default();
    let id = world.spawn().id();
    world.set_name(id, "roundtrip");

    let mut transform = Transform::default();
    transform.local_position = Vector3::new(1.5, -2.0, 3.25);
    transform.local_euler_angles = Vector3::new(0.0, 90.0, 45.0);
    transform.local_scale = Vector3::new(2.0, 2.0, 2.0);
    world.add_component(id, transform);

    let mut camera = Camera::default();
    camera.fov_y = 75.0;
    camera.is_main = true;
    world.add_component(id, camera);

    let mut renderer = ModelRenderer::default();
    renderer.model_path = "m_test_model".to_string();
    renderer.material_override = Some("mat_test".to_string());
    world.add_component(id, renderer);

    let mut light = Light::default();
    light.light_type = LightType::Spot {
        length: 12.0,
        angle: 30.0,
    };
    light.intensity = 3.5;
    world.add_component(id, light);

    let mut collider = Collider::default();
    collider.shape = ColliderShape::Capsule {
        radius: 0.4,
        height: 1.8,
    };
    collider.offset = Vector3::new(0.0, 0.9, 0.0);
    collider.is_static = true;
    world.add_component(id, collider);

    let mut velocity = Velocity::default();
    velocity.linear_velocity = Vector3::new(1.0, 0.0, -1.0);
    velocity.mass = 4.0;
    world.add_component(id, velocity);

    world.add_component(id, Gravity { strength: 3.7 });

    let saved = serialize_cell(&world, origin()).expect("cell should serialize");

    let mut reloaded = World::default();
    // Ensure the target cell exists in the fresh world.
    let seed = reloaded.spawn().id();
    reloaded.despawn(seed);
    load_cell(&mut reloaded, origin(), &saved).expect("cell should load");

    let resaved = serialize_cell(&reloaded, origin()).expect("cell should re-serialize");
    assert_eq!(
        saved, resaved,
        "cell changed across a save/load/save roundtrip"
    );

    // Spot-check values survived, not just self-consistency.
    let id2 = reloaded.get_entities_with_component::<Camera>()[0];
    assert_eq!(reloaded.get_name(id2), Some("roundtrip"));
    let cam = reloaded.get_component::<Camera>(id2).unwrap();
    assert_eq!(cam.fov_y, 75.0);
    assert!(cam.is_main);
    let t = reloaded.get_component::<Transform>(id2).unwrap();
    assert_eq!(t.local_position, Vector3::new(1.5, -2.0, 3.25));
    let c = reloaded.get_component::<Collider>(id2).unwrap();
    assert!(c.is_static);
    assert!(matches!(
        c.shape,
        ColliderShape::Capsule { radius, height } if radius == 0.4 && height == 1.8
    ));
    let l = reloaded.get_component::<Light>(id2).unwrap();
    assert_eq!(l.intensity, 3.5);
    assert!(matches!(
        l.light_type,
        LightType::Spot { length, angle } if length == 12.0 && angle == 30.0
    ));
    let v = reloaded.get_component::<Velocity>(id2).unwrap();
    assert_eq!(v.mass, 4.0);
    assert_eq!(reloaded.get_component::<Gravity>(id2).unwrap().strength, 3.7);
    let m = reloaded.get_component::<ModelRenderer>(id2).unwrap();
    assert_eq!(m.model_path, "m_test_model");
    assert_eq!(m.material_override.as_deref(), Some("mat_test"));
}

/// Loads a cell written in the legacy hand-rolled YAML format (flattened
/// `light_type`/`shape` keys, `[x, y, z]` vectors, `""` for a missing
/// material override) and checks every field lands correctly.
#[test]
fn legacy_scene_format_still_loads() {
    let yaml = r#"
name: legacy
entities:
  - name: OldEntity
    components:
      - type: Transform
        local_position: [1.0, 2.0, 3.0]
        local_euler_angles: [0.0, 180.0, 0.0]
        local_scale: [1.0, 1.0, 1.0]
        global_position: [1.0, 2.0, 3.0]
        global_euler_angles: [0.0, 180.0, 0.0]
        global_scale: [1.0, 1.0, 1.0]
        global_rotation: [0.0, 1.0, 0.0, 0.0]
      - type: ModelRenderer
        model_path: m_default_cube
        material_override: ""
        is_wireframe: false
      - type: Light
        light_type: Point
        radius: 6.0
        color:
          r: 0.5
          g: 0.25
          b: 1.0
        intensity: 2.0
        is_emitting: true
        is_flickering: false
        intensity_min: 1.0
        intensity_max: 2.0
        radius_min: 1.0
        radius_max: 21.0
      - type: Collider
        shape: Cuboid
        size: [2.0, 1.0, 2.0]
        offset: [0.0, 0.5, 0.0]
        is_static: true
        is_area: false
    tags: []
    children: []
"#;
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
    let mut world = World::default();
    let seed = world.spawn().id();
    world.despawn(seed);
    load_cell(&mut world, origin(), &value).expect("legacy cell should load");

    let id = world.get_entities_with_component::<Transform>()[0];
    let t = world.get_component::<Transform>(id).unwrap();
    assert_eq!(t.local_position, Vector3::new(1.0, 2.0, 3.0));
    assert_eq!(t.local_euler_angles, Vector3::new(0.0, 180.0, 0.0));

    let m = world.get_component::<ModelRenderer>(id).unwrap();
    assert_eq!(m.model_path, "m_default_cube");
    assert_eq!(m.material_override, None, "\"\" must load as None");

    let l = world.get_component::<Light>(id).unwrap();
    assert!(matches!(l.light_type, LightType::Point { radius } if radius == 6.0));
    assert_eq!(l.color, Vector3::new(0.5, 0.25, 1.0));
    assert_eq!(l.intensity, 2.0);

    let c = world.get_component::<Collider>(id).unwrap();
    assert!(matches!(
        c.shape,
        ColliderShape::Cuboid { size } if size == Vector3::new(2.0, 1.0, 2.0)
    ));
    assert_eq!(c.offset, Vector3::new(0.0, 0.5, 0.0));
    assert!(c.is_static);
}

/// Voxel/item asset definitions feed scalar YAML values to component
/// registrations (`BreakTicks: 10`, `HasTint: 0`, ...). Those must keep
/// deserializing, and must stay out of scene serialization (reads return None).
#[test]
fn scalar_asset_components_deserialize() {
    let mut world = World::default();
    let id = world.spawn().id();

    for (type_name, yaml) in [
        ("BreakTicks", "10"),
        ("Drops", "Apostasy:Item:Dirt"),
        ("HasTint", "1"),
        ("Voxel", "Apostasy:Voxel:Grass"),
    ] {
        let reg = get_component_registration(type_name)
            .unwrap_or_else(|| panic!("{type_name} not registered"));
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let mut component = (reg.create)();
        (reg.deserialize)(&mut component, &value)
            .unwrap_or_else(|e| panic!("{type_name} failed to deserialize: {e}"));
        (reg.add_to_world)(&mut world, id, component);
        assert!(
            (reg.read)(&world, id).is_none(),
            "{type_name} is asset data and must not serialize into scenes"
        );
    }

    use apostasy_core::items::voxel_component::Voxel;
    use apostasy_core::voxels::voxel_components::break_ticks::BreakTicks;
    use apostasy_core::voxels::voxel_components::drops::Drops;
    use apostasy_core::voxels::voxel_components::tints::{HasTint, TintType};
    assert_eq!(world.get_component::<BreakTicks>(id).unwrap().0, 10);
    assert_eq!(
        world.get_component::<Drops>(id).unwrap().0,
        "Apostasy:Item:Dirt"
    );
    assert_eq!(world.get_component::<HasTint>(id).unwrap().0, TintType::Water);
    assert_eq!(
        world.get_component::<Voxel>(id).unwrap().name,
        "Apostasy:Voxel:Grass"
    );
}

/// Registry `apply` performs a partial update: fields missing from the patch
/// keep their current values.
#[test]
fn apply_is_a_partial_update() {
    let mut world = World::default();
    let id = world.spawn().id();

    let mut camera = Camera::default();
    camera.fov_y = 60.0;
    camera.near = 0.5;
    camera.is_main = true;
    world.add_component(id, camera);

    let reg = get_component_registration("Camera").unwrap();
    let patch: serde_yaml::Value = serde_yaml::from_str("fov_y: 100.0").unwrap();
    (reg.apply)(&mut world, id, &patch);

    let cam = world.get_component::<Camera>(id).unwrap();
    assert_eq!(cam.fov_y, 100.0, "patched field must update");
    assert_eq!(cam.near, 0.5, "unpatched field must keep its value");
    assert!(cam.is_main, "unpatched field must keep its value");
}
