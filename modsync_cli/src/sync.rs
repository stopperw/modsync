use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    str::FromStr,
    time::Instant,
};

use anyhow::anyhow;
use clap::Args;
use colored::Colorize;
use globset::{Glob, GlobSetBuilder};
use ignore::gitignore::GitignoreBuilder;
use log::{error, info};
use modsync_core::{
    api::{FileSyncBody, FileSyncResponse, ModpackResponse},
    FileState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

/// Command to sync local mods to the server
#[derive(Args, Debug)]
pub struct SyncCommand {
    /// Game directory to sync
    target_directory: Option<String>,

    /// Force sync all mods, instead of only changes
    #[arg(short = 'f', long)]
    force_sync: bool,

    /// Force upload all mod files
    #[arg(short = 'u', long)]
    force_upload: bool,

    /// Download server's state view into target directory
    #[arg(short = 'd', long)]
    download_state: bool,
}

#[derive(Serialize, Deserialize)]
pub struct UploadConfig {
    pub modpack_id: String,
    pub server_url: String,
    pub api_key: String,
    pub include_globs: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub enum FileDirtyness {
    Clean,
    Created,
    Updated,
    Deleted,
}

#[derive(Serialize, Deserialize)]
pub struct SyncFile {
    pub hash: Option<String>,
    pub state: FileState,
    pub dirty: FileDirtyness,
}

impl SyncFile {
    pub fn created(hash: Option<String>) -> Self {
        SyncFile {
            hash,
            state: FileState::Exists,
            dirty: FileDirtyness::Created,
        }
    }

    pub fn make_deleted(&mut self) {
        self.state = FileState::Deleted;
        self.dirty = FileDirtyness::Deleted;
    }

    pub fn make_updated(&mut self, hash: String) {
        self.hash = Some(hash);
        self.dirty = FileDirtyness::Updated;
    }

    pub fn mark_synced(&mut self) {
        self.dirty = FileDirtyness::Clean;
    }
}

#[derive(Serialize, Deserialize)]
pub struct SyncState {
    pub state_version: u32,
    pub upload_version: u32,
    pub files: HashMap<String, SyncFile>,
}

impl SyncState {
    pub fn new() -> Self {
        SyncState {
            state_version: 0,
            upload_version: 0,
            files: HashMap::new(),
        }
    }
}

impl SyncCommand {
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let target = self.target_directory.clone().unwrap_or(".".to_string());
        let target_path = Path::new(&target);

        let config_string = std::fs::read_to_string(target_path.join("modsync.sync.toml"))
            .map_err(|_| {
                anyhow::anyhow!(
                    "No sync config found at {}",
                    target_path.join("modsync.sync.toml").to_string_lossy()
                )
            })?;
        let config: UploadConfig = toml::from_str(&config_string)?;

        let mut auth_value =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", config.api_key))?;
        auth_value.set_sensitive(true);
        let mut default_headers = reqwest::header::HeaderMap::new();
        default_headers.append(reqwest::header::AUTHORIZATION, auth_value);
        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .build()?;

        client
            .post(format!("{}/hello", config.server_url))
            .send()
            .await?
            .error_for_status()
            .map_err(|x| match x.status() {
                Some(reqwest::StatusCode::UNAUTHORIZED) => anyhow!("Invalid API key"),
                _ => x.into(),
            })?;
        info!(
            "Server ({}) authentication successful! Starting synchronization...",
            config.server_url
        );

        let instant = Instant::now();

        // Includes globset
        let mut builder = GlobSetBuilder::new();
        for i in config.include_globs.iter() {
            builder.add(Glob::new(i)?);
        }
        let includes = builder.build()?;

        // Excludes
        let mut builder = GitignoreBuilder::new(".");
        for i in config.excludes.iter() {
            builder.add_line(None, i)?;
        }
        let excludes = builder.build()?;

        let mut state = if self.download_state {
            let modpack: ModpackResponse = client
                .get(format!(
                    "{}/modpack/{}",
                    config.server_url, config.modpack_id
                ))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let mut files: HashMap<String, SyncFile> = HashMap::new();
            for (path, sync_file) in modpack.files.into_iter().map(|x| (x.path.clone(), x)) {
                files.insert(
                    path,
                    SyncFile {
                        hash: sync_file.hash,
                        state: sync_file.state,
                        dirty: FileDirtyness::Updated,
                    },
                );
            }
            SyncState {
                upload_version: 0,
                state_version: 0,
                files,
            }
        } else {
            let state_file = File::open(target_path.join("modsync.state.toml"));
            if let Ok(mut state_file) = state_file {
                let mut state_string = String::new();
                state_file.read_to_string(&mut state_string)?;
                let upload_state: SyncState = toml::from_str(&state_string)?;
                upload_state
            } else {
                SyncState::new()
            }
        };

        let mut checked_files: Vec<PathBuf> = Vec::new();
        for (entry, path) in WalkDir::new(target_path)
            .into_iter()
            .filter_map(|x| x.ok())
            .filter(|x| relativize_path(target_path, x.path()).is_some())
            .filter_map(|x| relativize_path(target_path, x.path()).map(|path| (x, path)))
            .filter(|(_, path)| includes.is_match(path))
            .filter(|(_, path)| !excludes.matched(path, false).is_ignore())
        {
            let path_str = match path.to_str() {
                Some(s) => s,
                None => {
                    error!("Invalid filename: {}", path.to_string_lossy().red());
                    continue;
                }
            };
            let mut file = File::open(entry.path())?;
            checked_files.push(path.clone());
            let sync_file = state.files.get_mut(path_str);
            match sync_file {
                Some(sync_file) => {
                    info!(
                        "[{}] Checking file {} for changes...",
                        "/".cyan(),
                        path_str.cyan()
                    );

                    // Hashing
                    let mut hasher = Sha256::new();
                    std::io::copy(&mut file, &mut hasher)?;
                    let hash_bytes = hasher.finalize();
                    let hash = hash_bytes
                        .iter()
                        .map(|x| format!("{:02x}", x))
                        .collect::<Vec<String>>()
                        .join("");
                    let hash_mismatch = match &sync_file.hash {
                        Some(sync_hash) => hash != *sync_hash,
                        None => false,
                    };

                    if hash_mismatch {
                        info!("[{}] File changed: {}", "*".yellow(), path_str.yellow());
                        sync_file.make_updated(hash);
                    }
                }
                None => {
                    info!("[{}] New file: {}", "+".green(), path_str.green());

                    // Hashing
                    let mut hasher = Sha256::new();
                    std::io::copy(&mut file, &mut hasher)?;
                    let hash_bytes = hasher.finalize();
                    let hash = hash_bytes
                        .iter()
                        .map(|x| format!("{:02x}", x))
                        .collect::<Vec<String>>()
                        .join("");

                    state
                        .files
                        .insert(path_str.to_string(), SyncFile::created(Some(hash)));
                }
            }
        }

        // Checking removed files
        for (path, sync_file) in state
            .files
            .iter_mut()
            .filter(|(_, x)| x.state == FileState::Exists)
            .filter(|(x, _)| !checked_files.contains(&PathBuf::from_str(x).unwrap()))
        {
            info!("[{}] File removed: {}", "x".red(), path.red());
            sync_file.make_deleted();
        }

        info!("Starting server synchronization...");

        // Synchronize to server
        let force_sync = self.force_sync;
        let force_upload = self.force_upload;
        for (path, sync_file) in state
            .files
            .iter_mut()
            .filter(|(_, x)| x.dirty != FileDirtyness::Clean || force_sync)
        {
            info!("[{}] Synchronizing {}...", "%".blue(), path.blue());
            let _sync_result = client
                .post(format!(
                    "{}/modpack/{}/filesync",
                    config.server_url, config.modpack_id
                ))
                .json(&FileSyncBody {
                    path: path.clone(),
                    state: sync_file.state,
                    hash: sync_file.hash.clone(),
                })
                .send()
                .await?
                .error_for_status()
                .map_err(|x| match x.status() {
                    Some(reqwest::StatusCode::UNAUTHORIZED) => anyhow!("Invalid API key"),
                    _ => x.into(),
                })?
                .json::<FileSyncResponse>()
                .await?;

            if sync_file.state == FileState::Exists
                && (force_upload
                    || sync_file.dirty == FileDirtyness::Created
                    || sync_file.dirty == FileDirtyness::Updated)
            {
                info!("[{}] Uploading {}...", "@".purple(), path.purple());
                let mut file = File::open(target_path.join(path))?;
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                drop(file);
                let part = reqwest::multipart::Part::bytes(data).file_name("upload");
                let multipart = reqwest::multipart::Form::new().part("upload", part);
                let _upload_result = client
                    .post(format!(
                        "{}/modpack/{}/upload",
                        config.server_url, config.modpack_id,
                    ))
                    .query(&[("file_path", path)])
                    .multipart(multipart)
                    .send()
                    .await?
                    .error_for_status()
                    .map_err(|x| match x.status() {
                        Some(reqwest::StatusCode::UNAUTHORIZED) => anyhow!("Invalid API key"),
                        _ => x.into(),
                    })?;
                // .json::<FileUploadResponse>()
                // .await?;
            }

            sync_file.mark_synced();
        }

        state.upload_version += 1;

        info!("Saving local state...");
        let state_toml = toml::to_string(&state)?;
        let mut state_file = File::create(target_path.join("modsync.state.toml"))?;
        state_file.write_all(state_toml.as_bytes())?;

        info!(
            "{} Sync completed in {:.2}s",
            "SUCCESS!".green(),
            instant.elapsed().as_secs_f32()
        );

        Ok(())
    }
}

pub fn relativize_path<T, P>(target: T, path: P) -> Option<PathBuf>
where
    T: AsRef<Path>,
    P: AsRef<Path>,
{
    let target = target.as_ref();
    let path = path.as_ref();
    let mut target_components = target.components();
    let mut path_components = path.components();

    let mut components: Vec<Component> = Vec::new();

    loop {
        match (target_components.next(), path_components.next()) {
            (Some(t), Some(p)) if components.is_empty() && t == p => {}
            (None, Some(p)) => components.push(p),
            (None, None) => {
                return Some(components.into_iter().collect());
            }
            _ => {}
        }
    }
}
