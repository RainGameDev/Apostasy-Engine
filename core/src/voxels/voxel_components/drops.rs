use apostasy_macros::{Component, Inspect};

/// Item drop reference formatted as `[namespace]:[Items]:[item name]`.
#[derive(Component, Inspect, Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[component(serde)]
pub struct Drops(pub String);
