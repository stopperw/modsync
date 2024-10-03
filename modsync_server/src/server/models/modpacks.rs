use modsync_core::api::ModpackId;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Modpack {
    pub id: ModpackId,
    pub name: String,
    pub modloader: Option<String>,
    pub modloader_version: Option<String>,
    pub game_version: Option<String>,
    pub sync_version: i32,
}

impl Modpack {
    pub async fn get_optional<'a, E>(id: &ModpackId, exec: E) -> Result<Option<Self>, sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        let file = sqlx::query!(
            "SELECT id, name, modloader, modloader_version, game_version, sync_version
            FROM modpacks WHERE id = $1 LIMIT 1",
            id.0
        )
        .fetch_optional(exec)
        .await?
        .map(|x| Modpack {
            id: ModpackId(x.id),
            name: x.name,
            modloader: x.modloader,
            modloader_version: x.modloader_version,
            game_version: x.game_version,
            sync_version: x.sync_version,
        });
        Ok(file)
    }

    pub async fn delete<'a, E>(id: &ModpackId, exec: E) -> Result<(), sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        sqlx::query!(
            "DELETE FROM modpacks WHERE id = $1",
            id.0
        )
        .execute(exec)
        .await?;
        Ok(())
    }
}

impl From<Modpack> for modsync_core::models::modpacks::Modpack {
    fn from(x: Modpack) -> Self {
        Self {
            id: x.id,
            name: x.name,
            modloader: x.modloader,
            modloader_version: x.modloader_version,
            game_version: x.game_version,
            sync_version: x.sync_version,
        }
    }
}

