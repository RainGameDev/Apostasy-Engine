use std::{collections::HashMap, fs, path::Path};

use apostasy_core::{
    anyhow::Result,
    cgmath::{Vector3, Zero},
    log, log_warn,
    objects::{
        Object, components::transform::Transform, resources::input_manager::InputManager,
        tags::Player, world::World,
    },
    rendering::components::{camera::GameCamera, model_renderer::ModelRenderer},
    serde_yaml, update,
    voxels::{
        structure::{StructureAsset, StructureBlock},
        voxel::VoxelRegistry,
        voxel_raycast::{Direction, voxel_raycast},
    },
};
use apostasy_macros::{Resource, Tag};

use crate::states::HasInitGeneration;
#[derive(Resource, Debug, Clone)]
pub struct StructureSelection {
    pub start: Vector3<i32>,
    pub end: Vector3<i32>,
}
#[derive(Resource, Debug, Clone)]
pub struct StructureSelectionMode;

impl Default for StructureSelection {
    fn default() -> Self {
        Self {
            start: Vector3::zero(),
            end: Vector3::zero(),
        }
    }
}

#[derive(Tag, Clone)]
pub struct StructureSelectionArea;

#[update]
pub fn structure_selection(world: &mut World) -> Result<()> {
    if !world.get_resource::<HasInitGeneration>().is_ok() {
        return Ok(());
    }

    let inputs = world.get_resource::<InputManager>()?;
    let registry = world.get_resource::<VoxelRegistry>()?.clone();
    let camera = world
        .get_object_with_tag::<GameCamera>()?
        .get_component::<Transform>()
        .unwrap()
        .clone();

    let save = inputs.is_keybind_active("SaveStructure");
    let toggle_selection = inputs.is_keybind_active("ToggleStructureSelection");
    let left_mouse = inputs.is_mousebind_active("Break");
    let right_mouse = inputs.is_mousebind_active("Place");
    let set_start = inputs.is_keybind_active("SetStructureStart");
    let set_end = inputs.is_keybind_active("SetStructureEnd");

    let player = world
        .get_object_with_tag::<Player>()?
        .get_component::<Transform>()
        .unwrap()
        .global_position;

    if toggle_selection {
        if world.get_resource::<StructureSelectionMode>().is_ok() {
            world.remove_resource::<StructureSelectionMode>();
        } else {
            world.insert_resource(StructureSelectionMode);
        }
    }

    if world.get_resource::<StructureSelectionMode>().is_ok() {
        if left_mouse {
            let raycast = voxel_raycast(world, &camera, 32.0, Direction::Forward);

            if let Ok(structure_selection) = world.get_resource_mut::<StructureSelection>() {
                if let Some(raycast) = raycast {
                    structure_selection.start = raycast.voxel_pos;
                }
            }
        }
        if right_mouse {
            let raycast = voxel_raycast(world, &camera, 32.0, Direction::Forward);

            if let Ok(structure_selection) = world.get_resource_mut::<StructureSelection>() {
                if let Some(raycast) = raycast {
                    structure_selection.end = raycast.voxel_pos;
                }
            }
        }
    }

    if let Ok(structure_selection) = world.get_resource_mut::<StructureSelection>() {
        if set_start {
            structure_selection.start =
                Vector3::new(player.x as i32, player.y as i32, player.z as i32);

            log!("Setting structure start: {:?}", structure_selection.start);
        }
        if set_end {
            structure_selection.end =
                Vector3::new(player.x as i32, player.y as i32, player.z as i32);
            log!("Setting structure end: {:?}", structure_selection.end);
        }

        let min = structure_selection.start;
        let max = structure_selection.end;
        let size = Vector3::new(
            (max.x - min.x).abs(),
            (max.y - min.y).abs(),
            (max.z - min.z).abs(),
        );
        if save {
            let padding = Vector3::new(2, 2, 2);
            let padded_size = (max + padding) - (min - padding);
            let mut blocks: Vec<StructureBlock> = Vec::new();

            // Voxels
            for x in min.x..max.x {
                for y in min.y..max.y {
                    for z in min.z..max.z {
                        log!("{:?}", [x, y, z]);
                        if let Some(voxel) = world.get_voxel(x, y, z) {
                            let voxel = registry.id_to_name.get(&voxel).unwrap();
                            blocks.push(StructureBlock {
                                position: [x, y, z],
                                voxel: voxel.clone(),
                            });
                        } else {
                            log_warn!(
                                "Voxel: {} {} {} is out of range, aborting the structure creation",
                                x,
                                y,
                                z
                            );
                            return Ok(());
                        }
                    }
                }
            }

            log!("{:?}", blocks);

            let structure = StructureAsset {
                name: "Structure".to_string(),
                namespace: "Apostasy".to_string(),
                class: "Structure".to_string(),
                origin: [0, 0, 0],
                size: Some([padded_size.x, padded_size.y, padded_size.z]),
                blocks,
                metadata: HashMap::new(),
            };

            let dir = Path::new("res/structures");

            // Create the directory if it doesn't exist
            fs::create_dir_all(dir).expect("Failed to create res/structures directory");

            // Count existing new_structure_* files
            let count = fs::read_dir(dir)
                .expect("Failed to read res/structures directory")
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("new_structure_")
                })
                .count();

            let filename = format!("new_structure_{}.yaml", count);
            let filepath = dir.join(&filename);

            let contents = serde_yaml::to_string(&structure)?;
            fs::write(&filepath, contents)?;

            println!("Created: {}", filepath.display());
        }

        if let Ok(selection_area) = world.get_object_with_tag_mut::<StructureSelectionArea>() {
            let transform = selection_area.get_component_mut::<Transform>().unwrap();
            transform.local_scale = Vector3::new(size.x as f32, size.y as f32, size.z as f32);
            transform.local_position = Vector3::new(
                (min.x + max.x) as f32 / 2.0,
                (min.y + max.y) as f32 / 2.0,
                (min.z + max.z) as f32 / 2.0,
            );
        } else {
            let mut model_renderer = ModelRenderer::from_path("centered_cube.glb");
            model_renderer.is_wireframe = true;
            let object = Object::new()
                .add_component(Transform::default())
                .add_component(model_renderer)
                .add_tag(StructureSelectionArea);
            world.add_object(object);
        }
    } else {
        world.insert_resource(StructureSelection::default());
    }

    Ok(())
}
