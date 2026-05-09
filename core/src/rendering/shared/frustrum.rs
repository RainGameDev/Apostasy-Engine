use apostasy_macros::Resource;
use cgmath::{Matrix4, Vector3, Vector4};

pub struct Frustum {
    planes: [Vector4<f32>; 6],
}

impl Frustum {
    pub fn from_view_proj(vp: &Matrix4<f32>) -> Self {
        // Rows of the matrix
        let r0 = Vector4::new(vp.x.x, vp.y.x, vp.z.x, vp.w.x);
        let r1 = Vector4::new(vp.x.y, vp.y.y, vp.z.y, vp.w.y);
        let r2 = Vector4::new(vp.x.z, vp.y.z, vp.z.z, vp.w.z);
        let r3 = Vector4::new(vp.x.w, vp.y.w, vp.z.w, vp.w.w);

        let mut planes = [
            r3 + r0, // left
            r3 - r0, // right
            r3 + r1, // bottom
            r3 - r1, // top
            r3 + r2, // near
            r3 - r2, // far
        ];

        // Normalize each plane by its xyz length
        for plane in &mut planes {
            let len = (plane.x * plane.x + plane.y * plane.y + plane.z * plane.z).sqrt();
            plane.x /= len;
            plane.y /= len;
            plane.z /= len;
            plane.w /= len;
        }

        Self { planes }
    }

    pub fn contains_aabb(&self, min: Vector3<f32>, max: Vector3<f32>) -> bool {
        for plane in &self.planes {
            let px = if plane.x >= 0.0 { max.x } else { min.x };
            let py = if plane.y >= 0.0 { max.y } else { min.y };
            let pz = if plane.z >= 0.0 { max.z } else { min.z };

            if plane.x * px + plane.y * py + plane.z * pz + plane.w < 0.0 {
                return false;
            }
        }
        true
    }
}

#[derive(Resource, Clone)]
pub struct ObjectsDrawing(pub u32);
