use apostasy_macros::{Component, Inspect};

#[derive(Component, Inspect, Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[component(serde)]
pub struct BreakTicks(pub u32);
