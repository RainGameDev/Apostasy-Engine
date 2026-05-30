use apostasy_macros::Resource;
use ash::vk;
use egui::{Context, TextureId};

#[derive(Resource, Clone)]
pub struct EguiContext(pub Context);

#[derive(Resource, Clone)]
pub struct ViewportTexture(pub TextureId);

#[derive(Resource, Clone, Copy)]
pub struct ViewportSize {
    pub logical_width: f32, // egui logical points
    pub logical_height: f32,
    pub pixel_width: f32, // physical pixels (pixels_per_point * logical * supersample)
    pub pixel_height: f32,
    pub supersample: f32, // 1.0 = native, 2.0 = 2x supersample
}

impl ViewportSize {
    pub fn new(logical_w: f32, logical_h: f32) -> Self {
        Self {
            logical_width: logical_w,
            logical_height: logical_h,
            pixel_width: logical_w,
            pixel_height: logical_h,
            supersample: 1.0,
        }
    }

    pub fn to_extent(&self) -> vk::Extent2D {
        vk::Extent2D {
            width: self.pixel_width as u32,
            height: self.pixel_height as u32,
        }
    }

    pub fn aspect_logical(&self) -> f32 {
        if self.logical_height == 0.0 {
            1.0
        } else {
            self.logical_width / self.logical_height
        }
    }
}

impl Default for ViewportSize {
    fn default() -> Self {
        Self {
            logical_width: 960.0,
            logical_height: 540.0,
            pixel_width: 960.0,
            pixel_height: 540.0,
            supersample: 1.0,
        }
    }
}
