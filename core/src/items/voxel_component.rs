use apostasy_macros::Component;

#[derive(Component, Clone, Debug)]
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

impl Voxel {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        self.name = value
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("name expects a string"))?
            .to_string();
        Ok(())
    }
}
