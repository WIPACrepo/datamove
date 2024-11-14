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
    pub archive_path: String,
    pub max_threads: u8,
    pub retry_attempts: u32,
    pub work_cycle_sleep_seconds: u64,
}
