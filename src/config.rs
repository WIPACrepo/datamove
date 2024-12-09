// config.rs

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct DatamoveConfiguration {
    pub jade_database: JadeDatabaseConfig,
    pub sps_disk_archiver: SpsDiskArchiverConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JadeDatabaseConfig {
    pub username: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub database_name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpsDiskArchiverConfig {
    pub archive_headroom: u64,
    pub cache_dir: String,
    pub disk_archives_json_path: String,
    pub inbox_dir: String,
    pub problem_files_dir: String,
    pub status_port: u16,
    pub work_cycle_sleep_seconds: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiskArchives {
    #[serde(rename = "diskArchives")]
    pub disk_archives: Vec<DiskArchive>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiskArchive {
    #[serde(rename = "id")]
    pub id: u64,

    #[serde(rename = "uuid", skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(rename = "numCopies")]
    pub num_copies: u64,

    #[serde(rename = "paths")]
    pub paths: Vec<String>,

    #[serde(rename = "shortName", skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
}
