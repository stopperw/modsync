use serde::{Deserialize, Serialize};

use crate::{api::{FileId, ModpackId}, FileState};

#[derive(Serialize, Deserialize)]
pub struct File {
    pub id: FileId,
    pub modpack: ModpackId,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub path: String,
    pub state: FileState,
    pub sync_version: i32,
    pub hash: Option<String>,
    pub uploaded: bool,
}

