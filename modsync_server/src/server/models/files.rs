use modsync_core::{
    api::{FileId, ModpackId},
    FileState, StrConversion,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

impl File {
    pub async fn insert<'a, E>(modpack_id: &ModpackId, path: &'a str, state: FileState, hash: Option<&String>, exec: E) -> Result<FileId, sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        let new_id = Uuid::new_v4().to_string();
        sqlx::query!(
            "INSERT INTO FILES (id, modpack, created_at, updated_at, path, state, sync_version, hash, uploaded)
            VALUES ($1, $2, now(), now(), $3, $4, 0, $5, false)",
            new_id, modpack_id.0, path, state.as_str(), hash
        )
        .execute(exec)
        .await?;
        Ok(FileId(new_id))
    }

    pub async fn get<'a, E>(id: &FileId, exec: E) -> Result<Self, sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        let x = sqlx::query!(
            "SELECT id, modpack, created_at, updated_at, path, state, sync_version, hash, uploaded
            FROM files WHERE modpack = $1 LIMIT 1",
            id.0
        )
        .fetch_one(exec)
        .await?;
        Ok(File {
            id: FileId(x.id),
            modpack: ModpackId(x.modpack),
            created_at: x.created_at,
            updated_at: x.updated_at,
            path: x.path,
            state: FileState::from_str(&x.state),
            sync_version: x.sync_version,
            hash: x.hash,
            uploaded: x.uploaded,
        })
    }

    pub async fn get_optional<'a, E>(id: &FileId, exec: E) -> Result<Option<Self>, sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        let file = sqlx::query!(
            "SELECT id, modpack, created_at, updated_at, path, state, sync_version, hash, uploaded
            FROM files WHERE modpack = $1 LIMIT 1",
            id.0
        )
        .fetch_optional(exec)
        .await?
        .map(|x| File {
            id: FileId(x.id),
            modpack: ModpackId(x.modpack),
            created_at: x.created_at,
            updated_at: x.updated_at,
            path: x.path,
            state: FileState::from_str(&x.state),
            sync_version: x.sync_version,
            hash: x.hash,
            uploaded: x.uploaded,
        });
        Ok(file)
    }

    pub async fn get_by_modpack<'a, E>(id: &ModpackId, exec: E) -> Result<Vec<Self>, sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        let files: Vec<Self> = sqlx::query!(
            "SELECT id, modpack, created_at, updated_at, path, state, sync_version, hash, uploaded
            FROM files WHERE modpack = $1",
            id.0
        )
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(|x| File {
            id: FileId(x.id),
            modpack: ModpackId(x.modpack),
            created_at: x.created_at,
            updated_at: x.updated_at,
            path: x.path,
            state: FileState::from_str(&x.state),
            sync_version: x.sync_version,
            hash: x.hash,
            uploaded: x.uploaded,
        })
        .collect();
        Ok(files)
    }

    pub async fn get_by_path<'a, E>(modpack_id: &ModpackId, path: &'a str, exec: E) -> Result<Option<Self>, sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        let file = sqlx::query!(
            "SELECT id, modpack, created_at, updated_at, path, state, sync_version, hash, uploaded
            FROM files WHERE modpack = $1 AND path = $2",
            modpack_id.0, path
        )
        .fetch_optional(exec)
        .await?
        .map(|x| File {
            id: FileId(x.id),
            modpack: ModpackId(x.modpack),
            created_at: x.created_at,
            updated_at: x.updated_at,
            path: x.path,
            state: FileState::from_str(&x.state),
            sync_version: x.sync_version,
            hash: x.hash,
            uploaded: x.uploaded,
        });
        Ok(file)
    }

    pub async fn get_by_hash<'a, E>(hash: &'a str, exec: E) -> Result<Option<Self>, sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        let file = sqlx::query!(
            "SELECT id, modpack, created_at, updated_at, path, state, sync_version, hash, uploaded
            FROM files WHERE hash = $1 AND uploaded = true",
            hash
        )
        .fetch_optional(exec)
        .await?
        .map(|x| File {
            id: FileId(x.id),
            modpack: ModpackId(x.modpack),
            created_at: x.created_at,
            updated_at: x.updated_at,
            path: x.path,
            state: FileState::from_str(&x.state),
            sync_version: x.sync_version,
            hash: x.hash,
            uploaded: x.uploaded,
        });
        Ok(file)
    }

    pub async fn delete<'a, E>(id: &FileId, exec: E) -> Result<(), sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        sqlx::query!(
            "DELETE FROM files WHERE id = $1",
            id.0
        )
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn set_uploaded<'a, E>(id: &FileId, uploaded: bool, hash: Option<&String>, exec: E) -> Result<(), sqlx::Error>
    where
        E: sqlx::PgExecutor<'a>,
    {
        sqlx::query!(
            "UPDATE files SET updated_at = now(), uploaded = $1, hash = $2, sync_version = sync_version + 1 WHERE id = $3",
            uploaded, hash, id.0
        )
        .execute(exec)
        .await?;
        Ok(())
    }
}

impl From<File> for modsync_core::models::files::File {
    fn from(x: File) -> Self {
        Self {
            id: x.id,
            modpack: x.modpack,
            created_at: x.created_at,
            updated_at: x.updated_at,
            path: x.path,
            state: x.state,
            sync_version: x.sync_version,
            hash: x.hash,
            uploaded: x.uploaded
        }
    }
}

