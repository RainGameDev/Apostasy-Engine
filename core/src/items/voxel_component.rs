use apostasy_macros::{Component, Inspect};

#[derive(Component, Inspect, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[component(serde)]
#[serde(transparent)]
pub struct Voxel {
    pub name: String,
}

impl Default for Voxel {
    fn default() -> Self {
        Self {
            name: "Apostasy:Voxel:Air".to_string(),
        }
    }
}
