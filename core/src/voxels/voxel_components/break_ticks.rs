use apostasy_macros::Component;

#[derive(Component, Default, Clone, Debug)]
pub struct BreakTicks(pub u32);

impl BreakTicks {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        self.0 = value
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("break_ticks expects a uint"))? as u32;
        Ok(())
    }
}
