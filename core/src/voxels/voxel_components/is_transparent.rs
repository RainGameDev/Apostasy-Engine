use apostasy_macros::{Component, Inspect};

#[derive(Component, Inspect, Default, Clone, Debug)]
pub struct IsTransparent();

impl IsTransparent {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
}
