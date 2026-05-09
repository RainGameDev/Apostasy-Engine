use apostasy_core::{
    Component,
    anyhow::Result,
    cgmath::{Vector3, Zero},
    egui, fixed_update,
    items::container::Container,
    log,
    objects::{
        Object,
        components::transform::Transform,
        resources::{
            cursor_manager::CursorManager, input_manager::InputManager,
            window_manager::WindowManager,
        },
        tags::Player,
        world::World,
    },
    physics::{Gravity, collider::Collider, velocity::Velocity},
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
            build_delay: 3,
            current_build_ticks: 0,
        }
    }
}

#[start]
pub fn player_init(world: &mut World) -> Result<()> {
    let transform = Transform {
        local_position: Vector3::new(0.0, 50.0, 0.0),
        ..Default::default()
    };

    let camera = Object::new()
        .add_component(Transform {
            local_position: Vector3 {
                x: 0.0,
                y: 0.8,
                z: 0.0,
            },
            ..Default::default()
        })
        .add_component(Camera::default())
        .add_tag(ActiveCamera)
        .add_tag(GameCamera);
    let player = Object::new()
        .add_component(transform)
        .add_component(Velocity::default())
        .add_component(Container::default())
        .add_component(Gravity::default())
        .add_component(Collider::player())
        .add_component(PlayerData::default())
        .add_tag(Player)
        .add_tag(LoadingGate)
        .add_tag(NeedsSpawnPoint);

    let mut model_renderer = ModelRenderer::from_path("model.glb");
    model_renderer.is_wireframe = true;

    let voxel_outline = Object::new()
        .add_component(Transform::default())
        .add_component(model_renderer)
        .add_tag(VoxelOutline);

    let player_id = world.add_object(player.clone());
    let cam_id = world.add_object(camera.clone());
    world.add_object(voxel_outline);
    world.set_parent(cam_id, Some(player_id))?;
    Ok(())
}

#[update]
pub fn update(world: &mut World) -> Result<()> {
    if world.get_resource::<IsPaused>().is_ok()
        && !world.get_resource::<HasInitGeneration>().is_ok()
    {
        return Ok(());
    }

    let player = world.get_object_with_tag::<Player>()?;

    // Block movement if player is still loading
    let has_loading_gate = player.has_tag::<LoadingGate>();
    if has_loading_gate {
        let player = world.get_object_with_tag_mut::<Player>()?;
        let velocity = player.get_component_mut::<Velocity>()?;
        velocity.linear_velocity.x = 0.0;
        velocity.linear_velocity.y = 0.0;
        velocity.linear_velocity.z = 0.0;

        let player = world.get_object_with_tag_mut::<Player>()?;
        let transform = player.get_component_mut::<Transform>()?;
        transform.local_position.y = 400.0;

        transform.local_euler_angles = Vector3::zero();
        let camera = world.get_object_with_tag_mut::<GameCamera>()?;
        let transform = camera.get_component_mut::<Transform>()?;
        transform.local_euler_angles = Vector3::new(-90.0, 0.0, 0.0);

        return Ok(());
    }

    let inputs = world.get_resource::<InputManager>()?;

    let mouse_delta = inputs.mouse_delta;
    let direction = inputs.input_vector_2d("Right", "Left", "Backwards", "Forwards");
    let should_jump = inputs.is_keybind_active("Jump");

    let player = world.get_object_with_tag_mut::<Player>()?;
    let player_transform = player.get_component_mut::<Transform>()?;
    player_transform.local_euler_angles.y -= mouse_delta.0 as f32 * 0.5;

    let camera = world.get_object_with_tag_mut::<GameCamera>()?;
    let cam_transform = camera.get_component_mut::<Transform>()?;
    cam_transform.local_euler_angles.x -= mouse_delta.1 as f32 * 0.5;
    cam_transform.local_euler_angles.x = cam_transform.local_euler_angles.x.clamp(-89.0, 89.0);

    let player = world.get_object_with_tag::<Player>()?;
    let rotation = player.get_component::<Transform>()?.global_rotation;

    let player = world.get_object_with_tag_mut::<Player>()?;
    let velocity = player.get_component_mut::<Velocity>()?;

    let wish_dir = rotation * Vector3::new(direction.x, 0.0, direction.y);
    velocity.linear_velocity.x = wish_dir.x * 3.0;
    velocity.linear_velocity.z = wish_dir.z * 3.0;

    if should_jump && velocity.is_grounded {
        velocity.linear_velocity.y = 4.0;
    }

    Ok(())
}
#[fixed_update]
pub fn block_updates(world: &mut World, _delta: f32) -> Result<()> {
    let inputs = world.get_resource::<InputManager>()?;
    let voxel_registry = world.get_resource::<VoxelRegistry>()?.clone();
    let to_break = inputs.is_mousebind_active("Break");
    let to_place = inputs.is_mousebind_active("Place");
    let can_build;

    let outline = world
        .get_objects_with_tag_with_ids::<VoxelOutline>()
        .first()
        .unwrap()
        .0
        .clone();
    let mut new_pos = Vector3::zero();

    if let Ok(hit) = voxel_raycast_camera(world, 4.0) {
        new_pos = Vector3::new(
            hit.voxel_pos.x as f32 + 0.5,
            hit.voxel_pos.y as f32 + 0.5,
            hit.voxel_pos.z as f32 + 0.5,
        );
    } else {
        new_pos = Vector3::new(0.0 + 0.5, -6000.0 + 0.5, 0.0 + 0.5);
    }

    let outline_transform = world
        .get_object_mut(outline)
        .unwrap()
        .get_component_mut::<Transform>()?;
    outline_transform.local_position = new_pos;
    outline_transform.global_position = new_pos;

    {
        let player_id = world
            .get_objects_with_tag_with_ids::<Player>()
            .first()
            .unwrap()
            .0
            .clone();

        let player = world.get_object_mut(player_id).unwrap();
        let player_data = player.get_component_mut::<PlayerData>()?;
        player_data.current_build_ticks += 1;

        can_build = player_data.current_build_ticks >= player_data.build_delay;

        if player_data.current_build_ticks >= player_data.build_delay {
            player_data.current_build_ticks = 0;
        }
    }

    if to_break {
        voxel_raycast_system(world, Some(0), 4.0)?;
    }

    let player_id = world
        .get_objects_with_tag_with_ids::<Player>()
        .first()
        .unwrap()
        .0
        .clone();

    let place_action: Option<(u64, u64)> = if to_place && can_build {
        let player = world.get_object_mut(player_id).unwrap();
        let inventory = player.get_component_mut::<Container>()?;

        if let Some(item) = inventory.items.get(inventory.selected_item as usize) {
            if item.amount > 0 {
                let voxel = item.item.split(":").collect::<Vec<_>>();
                let voxel_key = format!("{}:Voxel:{}", voxel[0], voxel[2]);

                if let Some(voxel_id) = voxel_registry.name_to_id.get(&voxel_key) {
                    Some((inventory.selected_item as u64, voxel_id.clone() as u64))
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

    if let Some((selected_index, voxel_id)) = place_action {
        voxel_raycast_system(world, Some(voxel_id as u16), 4.0)?;

        let player_id = world
            .get_objects_with_tag_with_ids::<Player>()
            .first()
            .unwrap()
            .0
            .clone();

        let player = world.get_object_mut(player_id).unwrap();
        let inventory = player.get_component_mut::<Container>()?;
        inventory.remove_item_index(selected_index as usize);
    }

    Ok(())
}

#[update]
pub fn hud(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if !world.get_resource::<HasInitGeneration>().is_ok() {
        return Ok(());
    }

    if !world.get_resource::<LoadingState>()?.is_complete {
        return Ok(());
    }

    if let Some(player) = world.get_objects_with_tag_with_ids::<Player>().first() {
        let player = player.1;
        let inventory = player.get_component::<Container>()?.clone();

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

    Ok(())
}
