use apostasy_macros::Resource;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AntiAliasingAmount {
    X0,
    X2,
    #[default]
    X4,
    X8,
}

#[derive(Resource, Clone, Default)]
pub struct AntiAliasing {
    pub amount: AntiAliasingAmount,
}
