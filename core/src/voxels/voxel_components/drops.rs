use apostasy_macros::Component;

#[derive(Component, Default, Clone, Debug)]
pub struct Drops(pub String);

impl Drops {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        self.0 = value
            .as_str()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "drops expects a string formatted as [namespace]:[Items]:[item name]"
                )
            })?
            .to_string();
        Ok(())
    }
}
