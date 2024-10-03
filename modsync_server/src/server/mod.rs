use std::{env::var, sync::Arc, time::Duration};

use axum::{
    async_trait,
    extract::{
        DefaultBodyLimit, FromRef, FromRequestParts, Multipart, Path, Query, Request, State,
    },
    http::request::Parts,
    response::IntoResponse,
    routing::{get, post},
    Json, RequestPartsExt, Router,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use clap::Parser;
use error::ApiError;
use models::modpacks::Modpack;
use modsync_core::{
    api::{
        FileId, FileSyncBody, FileSyncResponse, HelloResponse, ModpackCreateBody,
        ModpackCreateResponse, ModpackId, ModpackResponse,
    },
    StrConversion,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::{fs::File, io::AsyncWriteExt};
use tower::ServiceExt;
use tower_http::{
    compression::CompressionLayer, limit::RequestBodyLimitLayer, services::ServeFile,
    timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::info;
use uuid::Uuid;

mod error;
mod models;

/// Modsync server
#[derive(Parser, Debug)]
pub struct ServeCommand {}

#[derive(Serialize, Deserialize)]
pub struct ServerConfigFile {
    pub database_url: Option<String>,
    pub master_key: Option<String>,
    pub port: Option<String>,
    pub uploads_directory: Option<String>,
}

#[derive(Clone)]
pub struct ServerConfig {
    pub database_url: String,
    pub master_key: String,
    pub port: u16,
    pub uploads_directory: String,
}

pub struct AppState {
    pub pool: PgPool,
    pub master_key: String,
    pub config: ServerConfig,
}

impl ServeCommand {
    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!("Modsync Server v{}", env!("CARGO_PKG_VERSION"));

        let server_config_text = std::fs::read_to_string(
            var("MODSYNC_CONFIG_PATH").unwrap_or("modsync.server.toml".to_string()),
        );
        let server_config_file = if let Ok(text) = server_config_text {
            Some(toml::from_str::<ServerConfigFile>(&text)?)
        } else {
            None
        };

        let config = ServerConfig {
            database_url: var("DATABASE_URL")
                .ok()
                .or(server_config_file
                    .as_ref()
                    .and_then(|x| x.database_url.clone()))
                .expect("No database URL set!"),
            master_key: var("MODSYNC_MASTER_KEY")
                .ok()
                .or(server_config_file
                    .as_ref()
                    .and_then(|x| x.master_key.clone()))
                .expect("No master key set!"),
            port: var("MODSYNC_PORT")
                .ok()
                .or(server_config_file.as_ref().and_then(|x| x.port.clone()))
                .unwrap_or("7040".to_string())
                .parse()?,
            uploads_directory: var("MODSYNC_UPLOADS_DIRECTORY")
                .ok()
                .or(server_config_file
                    .as_ref()
                    .and_then(|x| x.uploads_directory.clone()))
                .unwrap_or("uploads".to_string()),
        };

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
            .await?;

        sqlx::migrate!().run(&pool).await?;

        let state = Arc::new(AppState {
            pool,
            master_key: config.master_key.clone(),
            config: config.clone(),
        });

        let app = Router::new()
            .route(
                "/",
                get(|| async { "Modsync server - https://github.com/stopperw/modsync" }),
            )
            .route("/hello", post(hello))
            .route("/modpack/create", post(modpack_create))
            .route("/modpack/:modpack_id", get(modpack_get))
            .route("/modpack/:modpack_id/update", post(hello))
            .route("/modpack/:modpack_id/filesync", post(modpack_file_sync))
            .route("/modpack/:modpack_id/delete", post(modpack_delete))
            .route("/modpack/:modpack_id/upload", post(dl_file_upload))
            .route(
                "/dl/hash/:file",
                get(dl_file_hash).layer(CompressionLayer::new()),
            )
            .layer(DefaultBodyLimit::disable())
            .layer(RequestBodyLimitLayer::new(
                250 * 1024 * 1024, /* 250mb */
            ))
            .layer(TimeoutLayer::new(Duration::from_secs(15)))
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port))
            .await
            .unwrap();
        info!("Serving on 0.0.0.0:{}", config.port);
        axum::serve(listener, app).await.unwrap();

        Ok(())
    }
}

async fn hello(_: AuthenticatedKey) -> Json<HelloResponse> {
    Json(HelloResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        version_number: 0,
    })
}

async fn modpack_get(
    State(state): State<Arc<AppState>>,
    Path(modpack_id): Path<ModpackId>,
) -> Result<Json<ModpackResponse>, ApiError> {
    let modpack = Modpack::get_optional(&modpack_id, &state.pool).await?;
    if let Some(modpack) = modpack {
        let files = models::files::File::get_by_modpack(&modpack.id, &state.pool).await?;
        return Ok(Json(ModpackResponse {
            modpack: modpack.into(),
            files: files.into_iter().map(|x| x.into()).collect(),
        }));
    }
    Err(ApiError::NotFound)
}

async fn modpack_delete(
    State(state): State<Arc<AppState>>,
    _: AuthenticatedKey,
    Path(modpack_id): Path<ModpackId>,
) -> Result<Json<GenericResponse>, ApiError> {
    let modpack = Modpack::get_optional(&modpack_id, &state.pool).await?;
    if let Some(modpack) = modpack {
        Modpack::delete(&modpack.id, &state.pool).await?;
        return Ok(Json(GenericResponse::new()));
    }
    Err(ApiError::NotFound)
}

async fn modpack_create(
    State(state): State<Arc<AppState>>,
    _: AuthenticatedKey,
    Json(data): Json<ModpackCreateBody>,
) -> Result<Json<ModpackCreateResponse>, ApiError> {
    let new_id = Uuid::new_v4().to_string();
    if sqlx::query!(
        "SELECT name FROM modpacks WHERE name = $1 LIMIT 1",
        data.name
    )
    .fetch_optional(&state.pool)
    .await?
    .is_some()
    {
        return Err(ApiError::AlreadyExists);
    }
    sqlx::query!(
        "
        INSERT INTO modpacks
        (id, name, game, game_version, modloader, modloader_version, sync_version) VALUES
        ($1, $2, $3, $4, $5, $6, 0)
    ",
        new_id,
        data.name,
        data.game,
        data.game_version,
        data.modloader,
        data.modloader_version
    )
    .execute(&state.pool)
    .await?;
    Ok(Json(ModpackCreateResponse {
        modpack_id: ModpackId(new_id),
    }))
}

#[derive(Serialize, Deserialize)]
pub struct FileUploadQuery {
    pub file_path: String,
}

#[derive(Serialize, Deserialize)]
pub enum FileUploadAction {
    Uploaded,
    Exists,
}

#[derive(Serialize, Deserialize)]
pub struct FileUploadResponse {
    pub action: FileUploadAction,
    pub file_id: Option<FileId>,
}

async fn dl_file_hash(
    State(state): State<Arc<AppState>>,
    Path(upload_hash): Path<String>,
    req: Request,
) -> Result<impl IntoResponse, ApiError> {
    sqlx::query!(
        "SELECT id FROM files WHERE hash = $1 AND uploaded = true",
        upload_hash
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound)?;
    Ok(
        ServeFile::new(std::path::Path::new(&state.config.uploads_directory).join(&upload_hash))
            .oneshot(req)
            .await,
    )
}

async fn dl_file_upload(
    State(state): State<Arc<AppState>>,
    _: AuthenticatedKey,
    Path(modpack_id): Path<ModpackId>,
    Query(query): Query<FileUploadQuery>,
    mut multipart: Multipart,
) -> Result<Json<FileUploadResponse>, ApiError> {
    let existing_file =
        models::files::File::get_by_path(&modpack_id, &query.file_path, &state.pool).await?;
    if existing_file.is_none() {
        return Err(ApiError::NotFound);
    }

    if let Some(field) = multipart.next_field().await? {
        let data = field.bytes().await?;

        // Hashing
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hasher.finalize();
        // FIXME: doesn't sound efficient tbf
        let hash_str = hash
            .into_iter()
            .map(|x| format!("{:02x}", x))
            .collect::<Vec<String>>()
            .join("");

        let uploaded_file = models::files::File::get_by_hash(&hash_str, &state.pool).await?;
        if let Some(file) = uploaded_file {
            if std::fs::exists(std::path::Path::new(&state.config.uploads_directory).join(&hash_str))? {
                return Ok(Json(FileUploadResponse {
                    action: FileUploadAction::Exists,
                    file_id: Some(file.id),
                }));
            }
        }

        if let Some(existing_file) = existing_file {
            let mut file =
                File::create(std::path::Path::new(&state.config.uploads_directory).join(&hash_str))
                    .await?;
            file.write_all(&data).await?;
            models::files::File::set_uploaded(
                &existing_file.id,
                true,
                Some(&hash_str),
                &state.pool,
            )
            .await?;
            return Ok(Json(FileUploadResponse {
                action: FileUploadAction::Uploaded,
                file_id: Some(existing_file.id),
            }));
        } else {
            return Err(ApiError::NotFound);
        }
    }
    Err(ApiError::BadRequest)
}

async fn modpack_file_sync(
    State(state): State<Arc<AppState>>,
    _: AuthenticatedKey,
    Path(modpack_id): Path<ModpackId>,
    Json(data): Json<FileSyncBody>,
) -> Result<Json<FileSyncResponse>, ApiError> {
    if sqlx::query!(
        "SELECT id FROM modpacks WHERE id = $1 LIMIT 1",
        &modpack_id.0
    )
    .fetch_optional(&state.pool)
    .await?
    .is_none()
    {
        return Err(ApiError::NotFound);
    }
    let file = models::files::File::get_by_path(&modpack_id, &data.path, &state.pool).await?;
    if let Some(file) = file {
        sqlx::query!(
            "UPDATE files SET path = $1, state = $2, hash = $3, updated_at = now() WHERE id = $4",
            data.path,
            data.state.as_str(),
            data.hash,
            file.id.0
        )
        .execute(&state.pool)
        .await?;
    } else {
        models::files::File::insert(
            &modpack_id,
            &data.path,
            data.state,
            data.hash.as_ref(),
            &state.pool,
        )
        .await?;
    }
    Ok(Json(FileSyncResponse {}))
}

#[derive(Serialize, Deserialize)]
pub struct GenericResponse {
    pub success: bool,
}

impl GenericResponse {
    pub fn new() -> Self {
        Self { success: true }
    }
}

#[allow(unused)]
pub struct AuthenticatedKey(pub String);

type AxumAppState = Arc<AppState>;
#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedKey
where
    AxumAppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AxumAppState::from_ref(state);
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| ApiError::Unauthorized)?;
        if state.master_key != *bearer.token() {
            return Err(ApiError::Unauthorized);
        }
        Ok(AuthenticatedKey(state.master_key.clone()))
    }
}
