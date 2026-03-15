use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Region {
    #[default]
    Americas,
    Europe,
    Asia,
}

impl Region {
    pub const ALL: [Self; 3] = [Self::Americas, Self::Europe, Self::Asia];

    pub fn flag(self) -> &'static str {
        match self {
            Self::Americas => "🇺🇸",
            Self::Europe => "🇪🇺",
            Self::Asia => "🇯🇵",
        }
    }

    pub fn ping_host(self) -> &'static str {
        match self {
            Self::Americas => "us.actual.battle.net",
            Self::Europe => "eu.actual.battle.net",
            Self::Asia => "kr.actual.battle.net",
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Americas => write!(f, "Americas"),
            Self::Europe => write!(f, "Europe"),
            Self::Asia => write!(f, "Asia"),
        }
    }
}
