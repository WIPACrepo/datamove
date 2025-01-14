// config.rs

use serde::{Deserialize, Serialize};
use std::fs::File;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

//
// datamove.toml
//

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
    pub contacts_json_path: String,
    pub disk_archives_json_path: String,
    pub inbox_dir: String,
    pub problem_files_dir: String,
    pub status_port: u16,
    pub tera_template_glob: String,
    pub work_cycle_sleep_seconds: u64,
}

//
// contacts.json
//

#[derive(Clone, Debug, Deserialize)]
pub struct Contacts {
    #[serde(rename = "contacts")]
    pub contacts: Vec<Contact>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Contact {
    pub name: String,
    pub email: String,
    pub role: ContactRole,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub enum ContactRole {
    /// Disabled account. Use this role to indicate that the account is
    /// disabled and should not be authenticated or contacted.
    #[serde(rename = "DISABLED")]
    Disabled,

    /// Administrator. Interested in jade from an administrative point
    /// of view; service levels, queue sizes, etc.
    #[serde(rename = "JADE_ADMIN")]
    JadeAdmin,

    /// Operator. Interested in jade from an operation point of view;
    /// are there any tapes to change?
    #[serde(rename = "WINTER_OVER")]
    WinterOver,

    /// Coordinator. Interested in jade from a detector operations point
    /// of view; who is using our data transfer capabilites and for what?
    #[serde(rename = "RUN_COORDINATION")]
    RunCoordination,
}

pub fn load_contacts(path: &str) -> Result<Contacts> {
    // open the contacts JSON configuration file
    let file = File::open(path).map_err(|e| format!("Failed to open file {}: {}", path, e))?;
    // deserialize the JSON into the Contacts structure
    let contacts: Contacts =
        serde_json::from_reader(&file).map_err(|e| format!("Failed to deserialize JSON: {}", e))?;
    // return the Contacts structure to the caller
    Ok(contacts)
}

//
// diskArchives.json
//

#[derive(Clone, Debug, Deserialize)]
pub struct DiskArchives {
    #[serde(rename = "diskArchives")]
    pub disk_archives: Vec<DiskArchive>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DiskArchive {
    #[serde(rename = "id")]
    pub id: u64,

    #[serde(rename = "uuid")]
    pub uuid: String,

    #[serde(rename = "description")]
    pub description: String,

    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "numCopies")]
    pub num_copies: u64,

    #[serde(rename = "paths")]
    pub paths: Vec<String>,

    #[serde(rename = "shortName")]
    pub short_name: String,
}

pub fn disk_archive_for_uuid<'config>(
    disk_archives: &'config DiskArchives,
    uuid: &str,
) -> Option<&'config DiskArchive> {
    // for disk_archive in &disk_archives.disk_archives {
    //     if disk_archive.uuid == uuid {
    //         return Some(disk_archive);
    //     }
    // }
    // None
    disk_archives
        .disk_archives
        .iter()
        .find(|&disk_archive| disk_archive.uuid == uuid)
}

pub fn load_disk_archives(path: &str) -> Result<DiskArchives> {
    // open the disk archives JSON configuration file
    let file = File::open(path).map_err(|e| format!("Failed to open file {}: {}", path, e))?;
    // deserialize the JSON into the DiskArchives structure
    let disk_archives: DiskArchives =
        serde_json::from_reader(&file).map_err(|e| format!("Failed to deserialize JSON: {}", e))?;
    // return the DiskArchives structure to the caller
    Ok(disk_archives)
}
