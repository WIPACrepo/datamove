// config.rs

use chrono::{Datelike, NaiveDateTime};
use lettre::{message::Mailbox, Address};
use serde::{Deserialize, Serialize};
use std::{fs::File, str::FromStr};

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

//
// datamove.toml
//

#[derive(Clone, Debug, Deserialize)]
pub struct DatamoveConfiguration {
    pub email_configuration: EmailConfig,
    pub jade_database: JadeDatabaseConfig,
    pub sps_disk_archiver: SpsDiskArchiverConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EmailConfig {
    pub enabled: bool,
    pub from: String,
    pub host: String,
    pub password: String,
    pub port: u16,
    pub reply_to: String,
    pub username: String,
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
    pub data_streams_json_path: String,
    pub disk_archives_json_path: String,
    pub inbox_dir: String,
    pub minimum_disk_age_seconds: u32,
    pub outbox_dir: String,
    pub problem_files_dir: String,
    pub status_port: u16,
    pub tera_template_glob: String,
    pub work_cycle_sleep_seconds: u64,
    pub work_dir: String,
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

impl From<&Contact> for Mailbox {
    fn from(value: &Contact) -> Self {
        let name = Some(value.name.clone());
        let email = Address::from_str(&value.email)
            // .expect(&format!("Invalid e-mail address: {}", value.email));
            .unwrap_or_else(|_| panic!("Invalid e-mail address: {}", value.email));
        Mailbox::new(name, email)
    }
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
// dataStreams.json
//

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompressionType {
    /// .tar.bz2 Compression
    Bzip2,
    /// .tar.gz Compression
    Gzip,
    /// .flat.tar Compression
    None,
    /// .tar.xz Compression
    Xz,
    /// .tar.zst Compression
    Zstd,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetroDiskPolicy {
    Archive,
    Ignore,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Credentials {
    pub username: String,
    #[serde(rename = "sshKeyPath")]
    pub ssh_key_path: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StreamMetadata {
    pub category: String,
    #[serde(rename = "dataCenterEmail")]
    pub data_center_email: String,
    #[serde(rename = "dataCenterName")]
    pub data_center_name: String,
    #[serde(rename = "entryTitle")]
    pub entry_title: String,
    pub parameters: String,
    #[serde(rename = "difSensorName")]
    pub dif_sensor_name: String,
    #[serde(rename = "sensorName")]
    pub sensor_name: String,
    pub subcategory: String,
    #[serde(rename = "technicalContactEmail")]
    pub technical_contact_email: String,
    #[serde(rename = "technicalContactName")]
    pub technical_contact_name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DataStream {
    pub id: u64,
    pub uuid: String,
    pub active: bool,
    #[serde(rename = "xferLimitKbitsSec")]
    pub xfer_limit_kbits_sec: Option<u64>,
    pub compression: CompressionType,
    #[serde(rename = "fileHost")]
    pub file_host: String,
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "filePrefix")]
    pub file_prefix: String,
    #[serde(rename = "binarySuffix")]
    pub binary_suffix: String,
    #[serde(rename = "semaphoreSuffix")]
    pub semaphore_suffix: String,
    pub credentials: Credentials,
    #[serde(rename = "streamMetadata")]
    pub stream_metadata: StreamMetadata,
    pub archives: Vec<String>,
    #[serde(rename = "retroDiskPolicy")]
    pub retro_disk_policy: RetroDiskPolicy,
}

impl DataStream {
    pub fn compute_data_warehouse_path(&self, datetime: &NaiveDateTime) -> String {
        let stream_metadata = &self.stream_metadata;
        let date = datetime.date();

        format!(
            "{}/{}/{}/{}/{:02}{:02}",
            stream_metadata.sensor_name,
            date.year(),
            stream_metadata.category,
            stream_metadata.subcategory,
            date.month(),
            date.day()
        )
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DataStreamsConfig {
    #[serde(rename = "dataStreams")]
    pub data_streams: Vec<DataStream>,
}

#[derive(Clone, Debug)]
pub struct DataStreams(pub Vec<DataStream>);

impl DataStreams {
    pub fn for_uuid(&self, uuid: &str) -> Option<DataStream> {
        self.0
            .iter()
            .find(|&data_stream| data_stream.uuid == uuid)
            .cloned()
    }
}

impl<'a> IntoIterator for &'a DataStreams {
    type Item = &'a DataStream;
    type IntoIter = std::slice::Iter<'a, DataStream>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

pub fn load_data_streams(path: &str) -> Result<DataStreamsConfig> {
    // open the disk archives JSON configuration file
    let file = File::open(path).map_err(|e| format!("Failed to open file {}: {}", path, e))?;
    // deserialize the JSON into the DataStreams structure
    let data_streams: DataStreamsConfig =
        serde_json::from_reader(&file).map_err(|e| format!("Failed to deserialize JSON: {}", e))?;
    // return the DataStreams structure to the caller
    Ok(data_streams)
}

//
// diskArchives.json
//

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

#[derive(Clone, Debug, Deserialize)]
pub struct DiskArchivesConfig {
    #[serde(rename = "diskArchives")]
    pub disk_archives: Vec<DiskArchive>,
}

#[derive(Clone, Debug)]
pub struct DiskArchives(pub Vec<DiskArchive>);

impl DiskArchives {
    pub fn for_uuid(&self, uuid: &str) -> Option<DiskArchive> {
        self.0
            .iter()
            .find(|&disk_archive| disk_archive.uuid == uuid)
            .cloned()
    }
}

impl<'a> IntoIterator for &'a DiskArchives {
    type Item = &'a DiskArchive;
    type IntoIter = std::slice::Iter<'a, DiskArchive>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

pub fn load_disk_archives(path: &str) -> Result<DiskArchivesConfig> {
    // open the disk archives JSON configuration file
    let file = File::open(path).map_err(|e| format!("Failed to open file {}: {}", path, e))?;
    // deserialize the JSON into the DiskArchives structure
    let disk_archives: DiskArchivesConfig =
        serde_json::from_reader(&file).map_err(|e| format!("Failed to deserialize JSON: {}", e))?;
    // return the DiskArchives structure to the caller
    Ok(disk_archives)
}

//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }

    #[test]
    fn test_deserialize_datastreams_json() -> Result<()> {
        let data_streams_json_text = include_str!("../tests/data/test_dataStreams.json");
        let _data_streams: DataStreamsConfig = serde_json::from_str(&data_streams_json_text)?;
        Ok(())
    }
}
