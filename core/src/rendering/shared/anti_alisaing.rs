use apostasy_macros::Resource;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AntiAliasingAmount {
    #[default]
    X0,
    X2,
    X4,
    X8,
}

#[derive(Resource, Clone, Default)]
pub struct AntiAliasing {
    pub amount: AntiAliasingAmount,
    pub available_options: Vec<AntiAliasingAmount>,
}
