// email.rs

use log::{error, trace};
use num_format::{Locale, ToFormattedString};
use serde::Serialize;
use std::path::Path;
use tera::Result as TeraResult;
use tera::{from_value, to_value, Context, Tera, Value};

use crate::config::{disk_archive_for_uuid, ContactRole};
use crate::sps::jade_db::service::disk::JadeDisk;
use crate::sps::jade_db::service::disk::{get_num_file_pairs, get_size_file_pairs};
use crate::sps::process::disk_archiver::DiskArchiver;
use crate::sps::utils::{get_free_space, get_total_space};

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

pub const CLOSE_DISK_TEMPLATE: &str = "closeArchiveDisk.tera";
pub const TERA_RENDER_ERROR: &str = "Something went wrong with Tera.";

#[derive(Serialize)]
pub struct EmailDisk {
    pub id: i64,
    pub label: String,
    pub copy_id: i32,
    pub uuid: String,
    pub date_created: String,
    pub date_updated: String,
    pub path: String,
}

impl From<&JadeDisk> for EmailDisk {
    fn from(value: &JadeDisk) -> Self {
        let id = value.jade_disk_id;
        let label = value.label.clone();
        let copy_id = value.copy_id;
        let uuid = value.uuid.clone();
        let dc_ndt = value.date_created; //.clone(); // TODO: remove?
        let du_ndt = value.date_updated; //.clone(); // TODO: remove?
        let date_created = dc_ndt.format("%b %d, %Y %-I:%M:%S %p").to_string();
        let date_updated = du_ndt.format("%b %d, %Y %-I:%M:%S %p").to_string();
        let path = value.device_path.clone();

        EmailDisk {
            id,
            label,
            copy_id,
            uuid,
            date_created,
            date_updated,
            path,
        }
    }
}

async fn build_disk_closed_body(
    disk_archiver: &DiskArchiver,
    _label_path: &Path,
    disk: &JadeDisk,
) -> String {
    let tera = &disk_archiver.tera;
    let mut context: Context = Context::default();

    let hostname = &disk_archiver.host.host_name;
    context.insert("hostname", hostname);

    let disk_archive_uuid = &disk.disk_archive_uuid;
    if let Some(disk_archive) =
        disk_archive_for_uuid(&disk_archiver.disk_archives, disk_archive_uuid)
    {
        context.insert("disk_archive", disk_archive);
    } else {
        return TERA_RENDER_ERROR.to_string();
    }

    let email_disk: EmailDisk = disk.into();
    context.insert("disk", &email_disk);

    let num_file_pairs = match get_num_file_pairs_on_disk(disk_archiver, disk).await {
        Ok(count) => count,
        Err(_) => {
            return TERA_RENDER_ERROR.to_string();
        }
    };
    let size_file_pairs = match get_size_file_pairs_on_disk(disk_archiver, disk).await {
        Ok(size) => size,
        Err(_) => {
            return TERA_RENDER_ERROR.to_string();
        }
    };
    context.insert("num_file_pairs", &num_file_pairs);
    context.insert("size_file_pairs", &size_file_pairs);

    let rate_bytes_sec = match calculate_rate_bytes_sec(disk, size_file_pairs) {
        Ok(rate_bytes_sec) => rate_bytes_sec,
        Err(_) => {
            return TERA_RENDER_ERROR.to_string();
        }
    };
    context.insert("rate_bytes_sec", &rate_bytes_sec);

    let device_path = &disk.device_path;
    let free_bytes = match get_free_space(device_path) {
        Ok(free_bytes) => free_bytes,
        Err(_) => {
            return TERA_RENDER_ERROR.to_string();
        }
    };
    let total_bytes = match get_total_space(device_path) {
        Ok(total_bytes) => total_bytes,
        Err(_) => {
            return TERA_RENDER_ERROR.to_string();
        }
    };
    context.insert("free_bytes", &free_bytes);
    context.insert("total_bytes", &total_bytes);

    // (0) Not Mounted:
    // (0) Not Usable:
    // (7) Available:
    //     /mnt/slot7
    //     /mnt/slot12
    //     /mnt/slot8
    //     /mnt/slot10
    //     /mnt/slot6
    //     /mnt/slot11
    //     /mnt/slot9
    // (1) In-Use:
    //     /mnt/slot5 ID:1885 [IceCube Disk Archive]
    // (4) Finished:
    //     /mnt/slot3 ID:1883 [IceCube Disk Archive]
    //     /mnt/slot4 ID:1884 [IceCube Disk Archive]
    //     /mnt/slot1 ID:1881 [IceCube Disk Archive]
    //     /mnt/slot2 ID:1882 [IceCube Disk Archive]

    context.insert("not_mounted_paths", &Vec::<String>::new());

    context.insert("not_usable_paths", &Vec::<String>::new());

    context.insert(
        "available_paths",
        &vec![
            "/mnt/slot7",
            "/mnt/slot12",
            "/mnt/slot8",
            "/mnt/slot10",
            "/mnt/slot6",
            "/mnt/slot11",
            "/mnt/slot9",
        ],
    );

    context.insert(
        "in_use_paths",
        &vec!["/mnt/slot5 ID:1885 [IceCube Disk Archive]"],
    );

    context.insert(
        "finished_paths",
        &vec![
            "/mnt/slot3 ID:1883 [IceCube Disk Archive]",
            "/mnt/slot4 ID:1884 [IceCube Disk Archive]",
            "/mnt/slot1 ID:1881 [IceCube Disk Archive]",
            "/mnt/slot2 ID:1882 [IceCube Disk Archive]",
        ],
    );

    render_email_body(tera, CLOSE_DISK_TEMPLATE, &context)
}

fn calculate_rate_bytes_sec(disk: &JadeDisk, size: i64) -> Result<i64> {
    // determine the duration over which the files were written
    let start = disk.date_created;
    let stop = disk.date_updated;
    let duration = stop.signed_duration_since(start);
    let seconds = duration.num_seconds();

    // if we had a negative duration
    if seconds < 1 {
        // no time travel allowed! https://www.youtube.com/watch?v=qAMoWEwK-4A
        let disk_id = disk.jade_disk_id;
        error!(
            "Unable to determine duration of Disk ID:{disk_id}; was {seconds} seconds (must be >0)"
        );
        return Err(format!(
            "Unable to determine duration of Disk ID:{disk_id}; was {seconds} seconds (must be >0)"
        )
        .into());
    }

    // return to caller: (how much we wrote / how long it took us to write it)
    Ok(size / seconds)
}

pub fn comma_separated_filter(
    value: &Value,
    _: &std::collections::HashMap<String, Value>,
) -> TeraResult<Value> {
    // let number = from_value::<i64>(value.clone()).unwrap_or(0);
    // let formatted = format!("{:?}", number);
    // Ok(to_value(formatted).unwrap())
    let number = from_value::<i64>(value.clone()).unwrap_or(0);
    let formatted = number.to_formatted_string(&Locale::en);
    Ok(to_value(formatted).unwrap())
}

pub fn compile_templates(tera_template_glob: &str) -> Result<Tera> {
    let mut tera = Tera::new(tera_template_glob)?;
    tera.register_filter("comma", comma_separated_filter);
    Ok(tera)
}

async fn get_num_file_pairs_on_disk(disk_archiver: &DiskArchiver, disk: &JadeDisk) -> Result<i64> {
    let num_file_pairs = get_num_file_pairs(&disk_archiver.context.db_pool, disk).await?;
    Ok(num_file_pairs)
}

async fn get_size_file_pairs_on_disk(disk_archiver: &DiskArchiver, disk: &JadeDisk) -> Result<i64> {
    let size_file_pairs = get_size_file_pairs(&disk_archiver.context.db_pool, disk).await?;
    Ok(size_file_pairs)
}

fn render_email_body(tera: &Tera, template_name: &str, context: &Context) -> String {
    match tera.render(template_name, context) {
        Ok(body) => body,
        Err(e) => {
            error!("Unable to render Tera template '{template_name}' due to: {e}");
            TERA_RENDER_ERROR.to_string()
        }
    }
}

pub fn send_email_streaming_disk_ended(
    disk_archiver: &DiskArchiver,
    label_path: &Path,
    disk: &JadeDisk,
) -> Result<()> {
    // first, let's build the email
    let _body = build_disk_closed_body(disk_archiver, label_path, disk);

    // obtain the list of human contacts to be informed
    let contacts = &disk_archiver.contacts.contacts;

    // for each contact on the list
    let email_to = contacts
        .iter()
        // keep the ones with the JADE_ADMIN and WINTER_OVER role
        .filter(|x| (x.role == ContactRole::JadeAdmin) || (x.role == ContactRole::WinterOver));
    // for each contact on the list
    for email_contact in email_to {
        let name = &email_contact.name;
        let address = &email_contact.email;
        trace!("Will send a Streaming Disk Full Notification e-mail to: {name} <{address}>");
    }

    // indicate to the caller that we succeeded
    Ok(())
}

//---------------------------------------------------------------------------
//---------------------------------------------------------------------------
//---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DiskArchive;
    use std::collections::HashMap;
    use tera::{Context, Number};

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }

    #[test]
    fn test_comma_separated_filter() -> Result<()> {
        let value = &Value::Number(Number::from_i128(12345).unwrap());
        let result = comma_separated_filter(value, &HashMap::new())?;
        assert_eq!("12,345", result);

        let value = &Value::Number(Number::from_i128(123456789).unwrap());
        let result = comma_separated_filter(value, &HashMap::new())?;
        assert_eq!("123,456,789", result);

        let value = &Value::Number(Number::from_i128(5952694763520).unwrap());
        let result = comma_separated_filter(value, &HashMap::new())?;
        assert_eq!("5,952,694,763,520", result);

        Ok(())
    }

    #[test]
    fn test_tera_template_close_archive() -> Result<()> {
        let mut tera = Tera::new("tests/data/**/*.tera")?;
        tera.register_filter("comma", comma_separated_filter);

        let mut context: Context = Context::default();

        // jade has filled an archival disk.

        // Host:         jade01
        // Archive:      IceCube Disk Archive

        context.insert("hostname", "jade01");

        context.insert(
            "disk_archive",
            &DiskArchive {
                id: 1,
                uuid: "e09e65f7-37d1-45a7-9553-723a582504ef".to_string(),
                description: "IceCube Disk Archive".to_string(),
                name: "Disk-IceCube".to_string(),
                num_copies: 2,
                paths: vec![
                    "/mnt/slot1".to_string(),
                    "/mnt/slot2".to_string(),
                    "/mnt/slot3".to_string(),
                    "/mnt/slot4".to_string(),
                    "/mnt/slot5".to_string(),
                    "/mnt/slot6".to_string(),
                    "/mnt/slot7".to_string(),
                    "/mnt/slot8".to_string(),
                    "/mnt/slot9".to_string(),
                    "/mnt/slot10".to_string(),
                    "/mnt/slot11".to_string(),
                    "/mnt/slot12".to_string(),
                ],
                short_name: "IceCube".to_string(),
            },
        );

        // The filled disk has the following information.

        // ID:           1884
        // Label:        IceCube_2_2024_0062
        // Copy:         2
        // UUID:         4a976221-f39b-4e5e-a0c6-e4fa7e3e88d5

        context.insert(
            "disk",
            &EmailDisk {
                id: 1884,
                label: "IceCube_2_2024_0062".to_string(),
                copy_id: 2,
                uuid: "4a976221-f39b-4e5e-a0c6-e4fa7e3e88d5".to_string(),
                date_created: "Dec 11, 2024 7:10:25 PM".to_string(),
                date_updated: "Dec 16, 2024 4:54:59 PM".to_string(),
                path: "/mnt/slot4".to_string(),
            },
        );

        // Started:      Dec 11, 2024 7:10:25 PM
        // Finished:     Dec 16, 2024 4:54:59 PM
        // Rate:         14,025,813 bytes/sec

        context.insert("rate_bytes_sec", &14025813i64);

        // Host:         jade01
        // Device:       /mnt/slot4
        // Free:         7,245,053,952 bytes
        // Total:        5,952,694,763,520 bytes

        context.insert("free_bytes", &7245053952u64);
        context.insert("total_bytes", &5952694763520u64);

        // File Count:   32,351
        // Data Size:    5,945,177,808,502 bytes

        context.insert("num_file_pairs", &32351i64);
        context.insert("size_file_pairs", &5945177808502i64);

        // Please unmount, remove, and safely store this disk.

        // Capacity Update:

        // (0) Not Mounted:
        // (0) Not Usable:
        // (7) Available:
        //     /mnt/slot7
        //     /mnt/slot12
        //     /mnt/slot8
        //     /mnt/slot10
        //     /mnt/slot6
        //     /mnt/slot11
        //     /mnt/slot9
        // (1) In-Use:
        //     /mnt/slot5 ID:1885 [IceCube Disk Archive]
        // (4) Finished:
        //     /mnt/slot3 ID:1883 [IceCube Disk Archive]
        //     /mnt/slot4 ID:1884 [IceCube Disk Archive]
        //     /mnt/slot1 ID:1881 [IceCube Disk Archive]
        //     /mnt/slot2 ID:1882 [IceCube Disk Archive]

        context.insert("not_mounted_paths", &Vec::<String>::new());

        context.insert("not_usable_paths", &Vec::<String>::new());

        context.insert(
            "available_paths",
            &vec![
                "/mnt/slot7",
                "/mnt/slot12",
                "/mnt/slot8",
                "/mnt/slot10",
                "/mnt/slot6",
                "/mnt/slot11",
                "/mnt/slot9",
            ],
        );

        context.insert(
            "in_use_paths",
            &vec!["/mnt/slot5 ID:1885 [IceCube Disk Archive]"],
        );

        context.insert(
            "finished_paths",
            &vec![
                "/mnt/slot3 ID:1883 [IceCube Disk Archive]",
                "/mnt/slot4 ID:1884 [IceCube Disk Archive]",
                "/mnt/slot1 ID:1881 [IceCube Disk Archive]",
                "/mnt/slot2 ID:1882 [IceCube Disk Archive]",
            ],
        );

        let result = tera.render("closeArchiveDisk.tera", &context)?;

        let expected = include_str!("../../tests/data/test_tera_template_close_archive.txt");

        assert_eq!(expected, result);
        Ok(())
    }
}
