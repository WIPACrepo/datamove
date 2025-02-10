// sps.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::sps::jade_db::service::disk::JadeDisk;

pub const NO_ID: i64 = 0;

// --------------------------------------------------------------------------
// -- disk_archiver ---------------------------------------------------------
// --------------------------------------------------------------------------

/// provide this struct to jade for its `jade status` command
#[derive(Deserialize, Serialize)]
pub struct DiskArchiverStatus {
    pub workers: Vec<DiskArchiverWorkerStatus>,

    #[serde(rename = "cacheAge")]
    pub cache_age: u64,

    #[serde(rename = "inboxAge")]
    pub inbox_age: u64,

    #[serde(rename = "problemFileCount")]
    pub problem_file_count: u64,

    #[serde(rename = "message", skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct DiskArchiverWorkerStatus {
    #[serde(rename = "archivalDisks")]
    pub archival_disks: HashMap<String, Disk>,

    #[serde(rename = "inboxCount")]
    pub inbox_count: u64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Disk {
    pub status: DiskStatus,

    #[serde(rename = "id")]
    pub id: i64,

    #[serde(rename = "closed", skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,

    #[serde(rename = "copyId", skip_serializing_if = "Option::is_none")]
    pub copy_id: Option<i32>,

    #[serde(rename = "onHold", skip_serializing_if = "Option::is_none")]
    pub on_hold: Option<bool>,

    #[serde(rename = "uuid", skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    #[serde(rename = "archive", skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,

    #[serde(rename = "available", skip_serializing_if = "Option::is_none")]
    pub available: Option<bool>,

    #[serde(rename = "label", skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    #[serde(rename = "serial", skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
}

impl Disk {
    pub fn for_status(status: DiskStatus) -> Self {
        // if this is an Available disk, set the available flag
        let available = if status == DiskStatus::Available {
            Some(true)
        } else {
            None
        };
        // return the disk for the requested status
        Disk {
            status,
            id: NO_ID,
            closed: None,
            copy_id: None,
            on_hold: None,
            uuid: None,
            archive: None,
            available,
            label: None,
            serial: None,
        }
    }
}

impl TryFrom<JadeDisk> for Disk {
    type Error = Box<dyn core::error::Error>;

    fn try_from(value: JadeDisk) -> Result<Self, Self::Error> {
        let closed: bool = value.closed;
        let on_hold: bool = value.on_hold;
        let status = if closed {
            DiskStatus::Finished
        } else {
            DiskStatus::InUse
        };
        // return the Disk
        Ok(Disk {
            status,
            id: value.jade_disk_id,
            closed: Some(closed),
            copy_id: Some(value.copy_id),
            on_hold: Some(on_hold),
            uuid: Some(value.uuid),
            archive: Some(value.disk_archive_uuid),
            available: None, // a disk from the database is already claimed (never available)
            label: Some(value.label),
            serial: None, // TODO: need to add this!!
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DiskStatus {
    #[serde(rename = "Not Mounted")]
    NotMounted,
    #[serde(rename = "Not Usable")]
    NotUsable,
    #[serde(rename = "Finished")]
    Finished,
    #[serde(rename = "In-Use")]
    InUse,
    #[serde(rename = "Available")]
    Available,
}

impl std::fmt::Display for DiskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status_str = match self {
            DiskStatus::NotMounted => "Not Mounted",
            DiskStatus::NotUsable => "Not Usable",
            DiskStatus::Finished => "Finished",
            DiskStatus::InUse => "In-Use",
            DiskStatus::Available => "Available",
        };
        write!(f, "{}", status_str)
    }
}

/// provide this struct to IceCube Live for detector status
#[derive(Deserialize, Serialize)]
pub struct LiveDiskArchiverStatus {
    #[serde(rename = "cacheAge")]
    pub cache_age: u64,

    #[serde(rename = "inboxAge")]
    pub inbox_age: u64,

    #[serde(rename = "message", skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(rename = "problemFileCount")]
    pub problem_file_count: u64,

    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    #[serde(rename = "inboxCount")]
    pub inbox_count: u64,

    #[serde(rename = "archivalDisks")]
    pub archival_disks: HashMap<String, Disk>,
}

impl From<DiskArchiverStatus> for LiveDiskArchiverStatus {
    fn from(value: DiskArchiverStatus) -> Self {
        // Extract values from the first worker or provide defaults
        let (inbox_count, archival_disks) = value
            .workers
            .first()
            .map(|worker| (worker.inbox_count, worker.archival_disks.clone()))
            .unwrap_or((0, HashMap::new()));

        Self {
            cache_age: value.cache_age,
            inbox_age: value.inbox_age,
            message: value.message,
            problem_file_count: value.problem_file_count,
            status: value.status,
            inbox_count,
            archival_disks,
        }
    }
}

//---------------------------------------------------------------------------
//---------------------------------------------------------------------------
//---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }

    #[test]
    fn test_disk_archiver_status() {
        assert!(true);
    }

    #[test]
    fn test_disk_archiver_status_live_serialization() {
        let mut archival_disks = HashMap::new();

        archival_disks.insert(
            "/mnt/slot12".to_string(),
            Disk {
                status: DiskStatus::NotMounted,
                id: 0,
                closed: None,
                copy_id: None,
                on_hold: None,
                uuid: None,
                archive: None,
                available: None,
                label: None,
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot7".to_string(),
            Disk {
                status: DiskStatus::Available,
                id: 0,
                closed: None,
                copy_id: None,
                on_hold: None,
                uuid: None,
                archive: None,
                available: Some(true),
                label: None,
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot8".to_string(),
            Disk {
                status: DiskStatus::NotMounted,
                id: 0,
                closed: None,
                copy_id: None,
                on_hold: None,
                uuid: None,
                archive: None,
                available: None,
                label: None,
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot10".to_string(),
            Disk {
                status: DiskStatus::NotMounted,
                id: 0,
                closed: None,
                copy_id: None,
                on_hold: None,
                uuid: None,
                archive: None,
                available: None,
                label: None,
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot5".to_string(),
            Disk {
                status: DiskStatus::Available,
                id: 0,
                closed: None,
                copy_id: None,
                on_hold: None,
                uuid: None,
                archive: None,
                available: Some(true),
                label: None,
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot11".to_string(),
            Disk {
                status: DiskStatus::NotMounted,
                id: 0,
                closed: None,
                copy_id: None,
                on_hold: None,
                uuid: None,
                archive: None,
                available: None,
                label: None,
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot6".to_string(),
            Disk {
                status: DiskStatus::Available,
                id: 0,
                closed: None,
                copy_id: None,
                on_hold: None,
                uuid: None,
                archive: None,
                available: Some(true),
                label: None,
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot3".to_string(),
            Disk {
                status: DiskStatus::InUse,
                id: 1685,
                closed: Some(false),
                copy_id: Some(2),
                on_hold: Some(false),
                uuid: Some("8e49c095-7702-4f22-92c5-4b4d5d2bb76f".to_string()),
                archive: Some("IceCube Disk Archive".to_string()),
                available: None,
                label: Some("IceCube_2_2024_0108".to_string()),
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot4".to_string(),
            Disk {
                status: DiskStatus::InUse,
                id: 1683,
                closed: Some(false),
                copy_id: Some(1),
                on_hold: Some(false),
                uuid: Some("29affab2-2469-4d70-a1c8-4b2e67294437".to_string()),
                archive: Some("IceCube Disk Archive".to_string()),
                available: None,
                label: Some("IceCube_1_2024_0102".to_string()),
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot1".to_string(),
            Disk {
                status: DiskStatus::Finished,
                id: 1659,
                closed: Some(true),
                copy_id: Some(1),
                on_hold: Some(false),
                uuid: Some("8464d018-60d5-4fbb-bd00-30a15f0c32ed".to_string()),
                archive: Some("IceCube Disk Archive".to_string()),
                available: None,
                label: Some("IceCube_1_2024_0091".to_string()),
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot2".to_string(),
            Disk {
                status: DiskStatus::Available,
                id: 0,
                closed: None,
                copy_id: None,
                on_hold: None,
                uuid: None,
                archive: None,
                available: Some(true),
                label: None,
                serial: None,
            },
        );

        archival_disks.insert(
            "/mnt/slot9".to_string(),
            Disk {
                status: DiskStatus::NotMounted,
                id: 0,
                closed: None,
                copy_id: None,
                on_hold: None,
                uuid: None,
                archive: None,
                available: None,
                label: None,
                serial: None,
            },
        );

        let status = LiveDiskArchiverStatus {
            cache_age: 800035,
            inbox_age: 38,
            message: None,
            problem_file_count: 0,
            status: Some("OK".to_string()),
            inbox_count: 0,
            archival_disks,
        };

        let serialized = serde_json::to_string_pretty(&status).unwrap();

        let expected_json = r#"
        {
            "cacheAge": 800035,
            "inboxAge": 38,
            "problemFileCount": 0,
            "status": "OK",
            "inboxCount": 0,
            "archivalDisks": {
                "/mnt/slot12": {
                    "status": "Not Mounted",
                    "id": 0
                },
                "/mnt/slot7": {
                    "status": "Available",
                    "id": 0,
                    "available": true
                },
                "/mnt/slot8": {
                    "status": "Not Mounted",
                    "id": 0
                },
                "/mnt/slot10": {
                    "status": "Not Mounted",
                    "id": 0
                },
                "/mnt/slot5": {
                    "status": "Available",
                    "id": 0,
                    "available": true
                },
                "/mnt/slot11": {
                    "status": "Not Mounted",
                    "id": 0
                },
                "/mnt/slot6": {
                    "status": "Available",
                    "id": 0,
                    "available": true
                },
                "/mnt/slot3": {
                    "status": "In-Use",
                    "id": 1685,
                    "closed": false,
                    "copyId": 2,
                    "onHold": false,
                    "uuid": "8e49c095-7702-4f22-92c5-4b4d5d2bb76f",
                    "archive": "IceCube Disk Archive",
                    "label": "IceCube_2_2024_0108"
                },
                "/mnt/slot4": {
                    "status": "In-Use",
                    "id": 1683,
                    "closed": false,
                    "copyId": 1,
                    "onHold": false,
                    "uuid": "29affab2-2469-4d70-a1c8-4b2e67294437",
                    "archive": "IceCube Disk Archive",
                    "label": "IceCube_1_2024_0102"
                },
                "/mnt/slot1": {
                    "status": "Finished",
                    "id": 1659,
                    "closed": true,
                    "copyId": 1,
                    "onHold": false,
                    "uuid": "8464d018-60d5-4fbb-bd00-30a15f0c32ed",
                    "archive": "IceCube Disk Archive",
                    "label": "IceCube_1_2024_0091"
                },
                "/mnt/slot2": {
                    "status": "Available",
                    "id": 0,
                    "available": true
                },
                "/mnt/slot9": {
                    "status": "Not Mounted",
                    "id": 0
                }
            }
        }"#;
        let actual_value: Value = serde_json::from_str(&serialized).unwrap();
        let expected_value: Value = serde_json::from_str(expected_json).unwrap();
        assert_eq!(
            actual_value, expected_value,
            "Serialized JSON did not match expected value."
        );
    }
}
