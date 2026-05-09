use apostasy_macros::Component;

#[derive(Component, Default, Clone, Debug)]
pub struct IsTransparent();

impl IsTransparent {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
}
