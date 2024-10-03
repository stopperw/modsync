use serde::{Deserialize, Serialize};

use crate::api::ModpackId;

#[derive(Serialize, Deserialize)]
pub struct Modpack {
    pub id: ModpackId,
    pub name: String,
    pub modloader: Option<String>,
    pub modloader_version: Option<String>,
    pub game_version: Option<String>,
    pub sync_version: i32,
}

