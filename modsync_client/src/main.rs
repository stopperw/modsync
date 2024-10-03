use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use clap::Parser;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info};
use modsync_core::{api::ModpackResponse, FileState};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use futures_util::StreamExt;

/// Synchronize your client's mods with the server!
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Force check all mods for mismatches
    #[arg(short = 'f', long)]
    force_check: bool,
}

#[derive(Serialize, Deserialize)]
pub struct FileInfo {
    pub sync_version: i32,
    pub hash: Option<String>,
    pub dirty: bool,
    pub disable_sync: Option<bool>,
}

impl FileInfo {
    pub fn new(sync_version: i32, hash: Option<String>) -> Self {
        FileInfo {
            sync_version,
            hash,
            dirty: true,
            disable_sync: None
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub modpack_id: String,
    pub server_url: String,
    #[serde(default)]
    pub files: HashMap<String, FileInfo>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(err) = run().await {
        error!("{} {}", "Error:".bright_red(), err);
    }

    info!("Modsync will exit in 10 seconds...");
    tokio::time::sleep(Duration::from_secs(10)).await;

    Ok(())
}

async fn run() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    pretty_env_logger::init();
    let args = Args::parse();

    info!(
        "{}",
        format!(
            "Modsync Client v{} / i swear, if you ask me why i made another \"git but worse\"",
            env!("CARGO_PKG_VERSION")
        )
        .red()
    );

    let config_string = tokio::fs::read_to_string("modsync.toml")
        .await
        .map_err(|_| anyhow::anyhow!("No modsync.toml found!"))?;
    let mut config: Config = toml::from_str(&config_string)?;

    let client = Client::new();

    let modpack: ModpackResponse = client
        .get(format!(
            "{}/modpack/{}",
            config.server_url, config.modpack_id
        ))
        .send()
        .await?
        .json()
        .await?;
    info!(
        "{}",
        format!(
            "Modpack {} from {}",
            modpack.modpack.name, config.server_url
        )
        .italic()
    );

    for (path, sync_file) in modpack.files.iter().map(|x| (x.path.clone(), x)) {
        if sync_file.state == FileState::Ignored {
            continue;
        }
        info!("Synchronizing {}...", path.blue());
        if !config.files.contains_key(&path) {
            config
                .files
                .insert(path.clone(), FileInfo::new(sync_file.sync_version, None));
        }
        let file_info = config.files.get_mut(&path).unwrap();
        if file_info.disable_sync.unwrap_or(false) {
            continue;
        }
        file_info.hash = sync_file.hash.clone();
        let file_hash = sync_file.hash.clone().unwrap_or("".to_string());
        let file = File::open(&path);
        if let Ok(mut file) = file {
            if sync_file.state == FileState::Exists
                && (file_info.dirty
                    || sync_file.sync_version > file_info.sync_version
                    || args.force_check)
            {
                // Verify file's hash and redownload if needed
                info!("[{}] Checking file {}...", "*".yellow(), path.yellow());

                // Hashing
                let mut hasher = Sha256::new();
                std::io::copy(&mut file, &mut hasher)?;
                let hash = hasher.finalize();
                // FIXME: doesn't sound efficient tbf
                let hash_str = hash
                    .into_iter()
                    .map(|x| format!("{:02x}", x))
                    .collect::<Vec<String>>()
                    .join("");

                if file_hash != hash_str {
                    info!(
                        "[{}] {} was updated, redownloading...",
                        "#".yellow(),
                        path.yellow()
                    );
                    download_file(&client, &config.server_url, &file_hash, &path).await?;
                    info!("[{}] {} redownloaded!", "#".green(), path.green());
                }
            } else if sync_file.state == FileState::Deleted {
                // Remove the file
                std::fs::remove_file(&path)?;
                info!("[{}] {} is removed.", "-".red(), path.red());
            }
        } else if sync_file.state == FileState::Exists {
            // Download the file
            info!(
                "[{}] File {} added, downloading...",
                "+".green(),
                path.green()
            );
            download_file(&client, &config.server_url, &file_hash, &path).await?;
            info!("[{}] {} downloaded!", "+".green(), path.green());
        }
        file_info.sync_version = sync_file.sync_version;
        file_info.dirty = false;
    }

    let config_string = toml::to_string(&config)?;
    tokio::fs::write("modsync.toml", config_string.as_bytes()).await?;

    info!("{}", "Sync complete! Have fun.".green());

    Ok(())
}

pub async fn download_file<'a, P>(
    client: &Client,
    url: &'a str,
    hash: &'a str,
    path: P,
) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    make_parent_directories(path.as_ref())?;
    let mut file = File::create(path.as_ref())?;

    let response = client
        .get(format!("{}/dl/hash/{}", url, hash))
        .send()
        .await?
        .error_for_status()?;
    let total_size = response.content_length();

    let bar = if let Some(size) = total_size {
        let bar = ProgressBar::new(size);
        bar.set_style(
            ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{bar:.cyan/blue}] {bytes}/{total_bytes}")?
                .progress_chars("#>-")
        );
        bar
    } else {
        ProgressBar::new_spinner()
    };

    let mut bar_progress: u64 = 0;
    bar.set_position(bar_progress);
    bar.tick();

    let mut file_stream = response
        .bytes_stream();

    while let Some(chunk) = file_stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        bar_progress += chunk.len() as u64;
        bar.set_position(bar_progress);
        bar.tick();
    }

    bar.finish();

    Ok(())
}

pub fn make_parent_directories<P>(path: P) -> Result<(), std::io::Error>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let mut components = path.components();
    let mut new_path_components: Vec<std::path::Component> = Vec::new();
    loop {
        match (components.next(), components.next()) {
            (Some(a), Some(_)) => {
                new_path_components.push(a);
                let new_path = new_path_components.iter().collect::<PathBuf>();
                if !std::fs::exists(&new_path)? {
                    std::fs::create_dir(new_path)?;
                }
            }
            _ => return Ok(()),
        }
    }
}
