use std::{collections::HashMap, fs, path::Path};

use apostasy_core::{
    anyhow::Result,
    cgmath::{Vector3, Zero},
    log, log_warn,
    objects::{
        components::transform::Transform, resources::input_manager::InputManager, tags::Player,
        world::World,
    },
    serde_yaml, update,
    voxels::{
        structure::{StructureAsset, StructureBlock},
        voxel::VoxelRegistry,
    },
};
use apostasy_macros::Resource;

use crate::states::HasInitGeneration;
#[derive(Resource, Debug, Clone)]
pub struct StructureSelection {
    pub start: Vector3<i32>,
    pub end: Vector3<i32>,
}

impl Default for StructureSelection {
    fn default() -> Self {
        Self {
            start: Vector3::zero(),
            end: Vector3::zero(),
        }
    }
}

#[update]
pub fn structure_selection(world: &mut World) -> Result<()> {
    if !world.get_resource::<HasInitGeneration>().is_ok() {
        return Ok(());
    }

    let inputs = world.get_resource::<InputManager>()?;
    let registry = world.get_resource::<VoxelRegistry>()?.clone();

    let save = inputs.is_keybind_active("SaveStructure");
    let set_start = inputs.is_keybind_active("SetStructureStart");
    let set_end = inputs.is_keybind_active("SetStructureEnd");

    let player = world
        .get_object_with_tag::<Player>()?
        .get_component::<Transform>()
        .unwrap()
        .global_position;

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

        if save {
            let min = structure_selection.start;
            let max = structure_selection.end;
            let padding = Vector3::new(2, 2, 2);
            let size = (max + padding) - (min - padding);
            let mut blocks: Vec<StructureBlock> = Vec::new();

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
                size: Some([size.x, size.y, size.z]),
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
    } else {
        world.insert_resource(StructureSelection::default());
    }

    Ok(())
}
