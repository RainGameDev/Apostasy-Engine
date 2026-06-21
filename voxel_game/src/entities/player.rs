use apostasy_core::{
    Component,
    anyhow::Result,
    cgmath::{Vector3, Zero},
    egui, fixed_update,
    items::{ItemRegistry, container::Container, voxel_component::Voxel},
    log_warn,
    ecs::{
        components::transform::Transform, resources::input_manager::InputManager,
        tags::Player, world::World,
    },
    physics::{
        Gravity,
        collider::{Collider, ColliderShape},
        velocity::Velocity,
    },
    rendering::components::{
        camera::{ActiveCamera, Camera, GameCamera},
        model_renderer::ModelRenderer,
    },
    serde_yaml, start,
    ui::ui_context::EguiContext,
    update,
    voxels::{
        voxel::VoxelRegistry,
        voxel_raycast::{voxel_raycast_camera, voxel_raycast_system},
    },
};

use crate::{
    entities::{loading_gate::LoadingGate, spawn_point::NeedsSpawnPoint},
    states::{HasInitGeneration, IsPaused},
    world::{VoxelOutline, loading_state::LoadingState},
};

#[derive(Component, Clone, Debug)]
pub struct PlayerData {
    pub build_delay: u32,
    pub current_build_ticks: u32,
}

impl PlayerData {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> Result<()> {
        Ok(())
    }
}

impl Default for PlayerData {
    fn default() -> Self {
        Self {
            build_delay: 4,
            current_build_ticks: 0,
        }
    }
}

impl apostasy_core::ecs::component::Inspect for PlayerData {}

#[start]
pub fn player_init(world: &mut World) -> Result<()> {
    let player_id = world.spawn();
    world.add_component(player_id, Transform {
        local_position: Vector3::new(0.0, 50.0, 0.0),
        ..Default::default()
    });
    world.add_component(player_id, Velocity::default());
    world.add_component(player_id, Container::default());
    world.add_component(player_id, Gravity::default());
    world.add_component(player_id, Collider {
        shape: ColliderShape::Capsule { radius: 0.4, height: 1.8 },
        is_static: false,
        ..Default::default()
    });
    world.add_component(player_id, PlayerData::default());
    world.add_tag::<Player>(player_id);
    world.add_tag::<LoadingGate>(player_id);
    world.add_tag::<NeedsSpawnPoint>(player_id);

    let cam_id = world.spawn();
    world.add_component(cam_id, Transform {
        local_position: Vector3::new(0.0, 0.8, 0.0),
        ..Default::default()
    });
    world.add_component(cam_id, Camera::default());
    world.add_tag::<ActiveCamera>(cam_id);
    world.add_tag::<GameCamera>(cam_id);

    let mut model_renderer = ModelRenderer::from_path("model.glb");
    model_renderer.is_wireframe = true;

    let outline_id = world.spawn();
    world.add_component(outline_id, Transform {
        local_scale: Vector3::new(0.85, 0.85, 0.85),
        ..Default::default()
    });
    world.add_component(outline_id, model_renderer);
    world.add_tag::<VoxelOutline>(outline_id);

    world.set_parent(cam_id, Some(player_id))?;
    Ok(())
}

#[update(priority = 1)]
pub fn update(world: &mut World) -> Result<()> {
    if world.has_resource::<IsPaused>() && !world.has_resource::<HasInitGeneration>() {
        return Ok(());
    }

    let player_id = world.get_entity_with_tag::<Player>()?;

    if world.get_resource::<IsPaused>().is_ok() {
        if let Some(velocity) = world.get_component_mut::<Velocity>(player_id) {
            velocity.process = false;
        }
        return Ok(());
    } else if let Some(velocity) = world.get_component_mut::<Velocity>(player_id) {
        velocity.process = true;
    }

    // Block movement if player is still loading
    if world.has_tag::<LoadingGate>(player_id) {
        if let Some(velocity) = world.get_component_mut::<Velocity>(player_id) {
            velocity.linear_velocity = Vector3::zero();
        }
        if let Some(transform) = world.get_component_mut::<Transform>(player_id) {
            transform.local_position.y = 400.0;
            transform.local_euler_angles = Vector3::zero();
        }

        let cam_id = world.get_entity_with_tag::<GameCamera>()?;
        if let Some(cam_transform) = world.get_component_mut::<Transform>(cam_id) {
            cam_transform.local_euler_angles = Vector3::new(-90.0, 0.0, 0.0);
        }
        return Ok(());
    }

    let inputs = world.get_resource::<InputManager>()?.clone();
    let mouse_delta = inputs.mouse_delta;
    let direction = inputs.input_vector_2d("Left", "Right", "Backwards", "Forwards");
    let should_jump = inputs.keybind_active("Jump")?;

    if let Some(player_transform) = world.get_component_mut::<Transform>(player_id) {
        player_transform.local_euler_angles.y -= mouse_delta.0 as f32;
    }

    let cam_id = world.get_entity_with_tag::<GameCamera>()?;
    if let Some(cam_transform) = world.get_component_mut::<Transform>(cam_id) {
        cam_transform.local_euler_angles.x -= mouse_delta.1 as f32;
        cam_transform.local_euler_angles.x = cam_transform.local_euler_angles.x.clamp(-89.0, 89.0);
    }

    let rotation = world.get_component::<Transform>(player_id)
        .map(|t| t.global_rotation)
        .unwrap_or_default();

    if let Some(velocity) = world.get_component_mut::<Velocity>(player_id) {
        let wish_dir = rotation * Vector3::new(direction.x, 0.0, direction.y);
        velocity.linear_velocity.x = wish_dir.x * 3.0;
        velocity.linear_velocity.z = wish_dir.z * 3.0;

        if should_jump && velocity.is_grounded {
            velocity.linear_velocity.y = 5.0;
        }
    }

    Ok(())
}

#[fixed_update]
pub fn block_updates(world: &mut World, _delta: f32) -> Result<()> {
    if world.get_resource::<IsPaused>().is_ok() {
        return Ok(());
    }

    let inputs = world.get_resource::<InputManager>()?.clone();
    let voxel_registry = world.get_resource::<VoxelRegistry>()?.clone();
    let item_registry = world.get_resource::<ItemRegistry>()?.clone();
    let to_break = inputs.mousebind_active("Break")?;
    let to_place = inputs.mousebind_active("Place")?;
    let can_build;

    let outline_id = world.get_entity_with_tag::<VoxelOutline>()?;

    let new_pos = if let Some(hit) = voxel_raycast_camera(world, 4.0) {
        Vector3::new(
            hit.voxel_pos.x as f32 + 0.5,
            hit.voxel_pos.y as f32 + 0.5,
            hit.voxel_pos.z as f32 + 0.5,
        )
    } else {
        Vector3::new(0.5, -5999.5, 0.5)
    };

    if let Some(t) = world.get_component_mut::<Transform>(outline_id) {
        t.local_position = new_pos;
    }

    let player_id = world.get_entity_with_tag::<Player>()?;

    {
        let player_data = world.get_component_mut::<PlayerData>(player_id)
            .ok_or_else(|| apostasy_core::anyhow::anyhow!("Player has no PlayerData"))?;
        player_data.current_build_ticks += 1;
        can_build = player_data.current_build_ticks >= player_data.build_delay;
    }

    if to_break {
        voxel_raycast_system(world, Some(0), 4.0)?;
    }

    let place_action: Option<(u64, u64)> = if to_place && can_build {
        if let Some(inventory) = world.get_component_mut::<Container>(player_id) {
            if let Some(item) = inventory.items.get(inventory.selected_item as usize) {
                if item.amount > 0 {
                    let item_def = item_registry.get_def(&item.item);
                    if let Some(item_def) = item_def {
                        if item_def.has_component::<Voxel>() {
                            let voxel = item_def.get_component::<Voxel>().unwrap();
                            let voxel_key = voxel.name.clone();
                            let selected = inventory.selected_item;
                            if let Some(voxel_id) = voxel_registry.name_to_id.get(&voxel_key) {
                                Some((selected as u64, *voxel_id as u64))
                            } else {
                                log_warn!(
                                    "Voxel with the name: {} does not exist in the registry",
                                    voxel_key
                                );
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some((selected_index, voxel_id)) = place_action
        && let Some(raycast) = voxel_raycast_camera(world, 4.0)
    {
        let (pos, half_extents) = {
            let transform = world.get_component::<Transform>(player_id)
                .ok_or_else(|| apostasy_core::anyhow::anyhow!("Player has no Transform"))?;
            let collider = world.get_component::<Collider>(player_id)
                .ok_or_else(|| apostasy_core::anyhow::anyhow!("Player has no Collider"))?;
            (transform.global_position, collider.half_extents)
        };

        let min = Vector3::new(
            (pos.x - half_extents.x).floor() as i32,
            (pos.y - half_extents.y).floor() as i32,
            (pos.z - half_extents.z).floor() as i32,
        );
        let max = Vector3::new(
            (pos.x + half_extents.x).floor() as i32,
            (pos.y + half_extents.y).floor() as i32,
            (pos.z + half_extents.z).floor() as i32,
        );

        let face_offset = match raycast.face {
            0 => (1, 0, 0),
            1 => (-1, 0, 0),
            2 => (0, 1, 0),
            3 => (0, -1, 0),
            4 => (0, 0, 1),
            5 => (0, 0, -1),
            _ => (0, 0, 0),
        };

        let place_pos = Vector3::new(
            raycast.voxel_pos.x + face_offset.0,
            raycast.voxel_pos.y + face_offset.1,
            raycast.voxel_pos.z + face_offset.2,
        );

        let inside_player = place_pos.x >= min.x
            && place_pos.x <= max.x
            && place_pos.y >= min.y
            && place_pos.y <= max.y
            && place_pos.z >= min.z
            && place_pos.z <= max.z;

        if inside_player {
            return Ok(());
        }

        voxel_raycast_system(world, Some(voxel_id as u16), 4.0)?;

        if let Some(inventory) = world.get_component_mut::<Container>(player_id) {
            inventory.remove_item_index(selected_index as usize);
        }
        if let Some(player_data) = world.get_component_mut::<PlayerData>(player_id) {
            player_data.current_build_ticks = 0;
        }
    }

    Ok(())
}

#[update]
pub fn hud(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if !world.has_resource::<HasInitGeneration>() {
        return Ok(());
    }

    if !world.get_resource::<LoadingState>()?.is_complete {
        return Ok(());
    }

    if let Ok(player_id) = world.get_entity_with_tag::<Player>() {
        let inventory = world.get_component::<Container>(player_id).cloned();
        if let Some(inventory) = inventory {
            egui::Window::new("Inventory")
                .anchor(egui::Align2::CENTER_TOP, [10.0, 10.0])
                .show(&ctx, |ui| {
                    for index in 0..inventory.items.len() {
                        if let Some(item) = inventory.items.get(index) {
                            if inventory.selected_item == index as u32 {
                                ui.label(format!("*{} ({})", item.item, item.amount));
                            } else {
                                ui.label(format!("{} ({})", item.item, item.amount));
                            }
                        }
                    }
                });
        }
    }

    Ok(())
}

#[update]
pub fn hotbar_update(world: &mut World) -> Result<()> {
    let inputs = world.get_resource::<InputManager>()?.clone();

    let pressed_slot = (1..=9)
        .find(|&i| inputs.is_keybind_active(&format!("Hotbar{}", i)))
        .map(|i| i - 1);

    let player_id = world.get_entity_with_tag::<Player>()?;

    if let Some(inventory) = world.get_component_mut::<Container>(player_id) {
        if let Some(slot) = pressed_slot
            && inventory.items.len() > slot
        {
            inventory.selected_item = slot as u32;
        }
    }

    Ok(())
}
