use serde::{Deserialize, Serialize};

pub mod api;
pub mod models;

pub trait StrConversion {
    fn from_str(value: &str) -> Self;
    fn as_str(&self) -> &'static str;
}

// pub trait TryFromStr {
//     pub fn try_from_str(value: &str) -> Result<Self>;
// }

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum DownloadSource {
    ModsyncDl,
    Modrinth,
}

impl std::fmt::Display for DownloadSource {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.as_str())
    }
}
impl StrConversion for DownloadSource {
    fn from_str(value: &str) -> Self {
        match value {
            "Modrinth" => Self::Modrinth,
            _ => Self::ModsyncDl,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::ModsyncDl => "ModsyncDl",
            Self::Modrinth => "Modrinth",
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ModState {
    Created,
    Updated,
    Deleted,
    Ignored,
}

impl std::fmt::Display for ModState {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.as_str())
    }
}
impl StrConversion for ModState {
    fn from_str(value: &str) -> Self {
        match value {
            "Created" => Self::Created,
            "Updated" => Self::Updated,
            "Deleted" => Self::Deleted,
            _ => Self::Ignored,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Updated => "Updated",
            Self::Deleted => "Deleted",
            Self::Ignored => "Ignored",
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum FileState {
    Exists,
    Deleted,
    Ignored,
}

impl std::fmt::Display for FileState {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.as_str())
    }
}
impl StrConversion for FileState {
    fn from_str(value: &str) -> Self {
        match value {
            "Exists" => Self::Exists,
            "Deleted" => Self::Deleted,
            _ => Self::Ignored,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Exists => "Exists",
            Self::Deleted => "Deleted",
            Self::Ignored => "Ignored",
        }
    }
}

