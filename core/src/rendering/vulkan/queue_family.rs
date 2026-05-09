use anyhow::Result;
use ash::vk::QueueFamilyProperties;

use crate::rendering::vulkan::device::PhysicalDevice;

#[derive(Debug, Clone)]
pub struct QueueFamily {
    pub index: u32,
    pub properties: QueueFamilyProperties,
}

#[derive(Debug, Clone)]
pub struct QueueFamilies {
    pub graphics: u32,
    pub present: u32,
    pub transfer: u32,
    pub compute: u32,
}

pub type QueueFamilyPicker = fn(Vec<PhysicalDevice>) -> Result<(PhysicalDevice, QueueFamilies)>;

pub mod queue_family_picker {
    use anyhow::{Context, Result};
    use ash::vk::QueueFlags;

    use crate::rendering::vulkan::{device::PhysicalDevice, queue_family::QueueFamilies};

    pub fn single_queue_family(
        physical_devices: Vec<PhysicalDevice>,
    ) -> Result<(PhysicalDevice, QueueFamilies)> {
        let physical_device = physical_devices.into_iter().next().unwrap();
        let queue_family = physical_device
            .queue_families
            .iter()
            .find(|qf| {
                qf.properties.queue_flags.contains(QueueFlags::GRAPHICS)
                    && qf.properties.queue_flags.contains(QueueFlags::COMPUTE)
            })
            .context("Failed to find a queue family that matches conditions: QueueFlag::Graphics and QueueFlags::Compute")?
            .clone();

        Ok((
            physical_device,
            QueueFamilies {
                graphics: queue_family.index,
                present: queue_family.index,
                transfer: queue_family.index,
                compute: queue_family.index,
            },
        ))
    }
}
