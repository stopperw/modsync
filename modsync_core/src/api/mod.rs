use serde::{Deserialize, Serialize};
use sqlx::sqlx_macros::Type;

use crate::{models::{self, modpacks::Modpack}, FileState};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, Eq, PartialOrd, Ord, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct ModpackId(pub String);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, Eq, PartialOrd, Ord, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct FileId(pub String);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, Eq, PartialOrd, Ord, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct UploadId(pub String);

#[derive(Serialize, Deserialize, Default)]
pub struct HelloResponse {
    pub version: String,
    pub version_number: u32,
}

#[derive(Serialize, Deserialize)]
pub struct ModpackResponse {
    pub modpack: Modpack,
    pub files: Vec<models::files::File>,
}

// File sync
#[derive(Serialize, Deserialize)]
pub struct FileSyncBody {
    pub path: String,
    pub state: FileState,
    pub hash: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct FileSyncResponse {}

// Modpack Create
#[derive(Serialize, Deserialize)]
pub struct ModpackCreateBody {
    pub name: String,
    pub game: String,
    pub game_version: String,
    pub modloader: String,
    pub modloader_version: String,
}

#[derive(Serialize, Deserialize)]
pub struct ModpackCreateResponse {
    pub modpack_id: ModpackId,
}

