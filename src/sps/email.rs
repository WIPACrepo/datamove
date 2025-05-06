// email.rs

use std::collections::HashMap;
use std::path::Path;

use lettre::{transport::smtp::client::Tls, Message, SmtpTransport, Transport};
use num_format::{Locale, ToFormattedString};
use serde::Serialize;
use tera::Result as TeraResult;
use tera::{from_value, to_value, Context, Tera, Value};
use tracing::{error, info, warn};

use crate::config::{Contact, ContactRole, EmailConfig};
use crate::sps::jade_db::service::disk::JadeDisk;
use crate::sps::jade_db::service::disk::{get_num_file_pairs, get_size_file_pairs};
use crate::sps::process::disk_archiver::build_archival_disks_status;
use crate::sps::process::disk_archiver::DiskArchiver;
use crate::sps::utils::{get_free_space, get_total_space};
use crate::status::sps::{
    Disk,
    DiskStatus::{Available, Finished, InUse, NotMounted, NotUsable},
};

use crate::error::{DatamoveError, Result};

pub const CLOSE_DISK_TEMPLATE: &str = "closeArchiveDisk.tera";
pub const CREATE_DISK_TEMPLATE: &str = "createArchiveDisk.tera";
pub const DISK_FULL_SUBJECT: &str = "Archive Disk Full Notification";
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

pub struct CapacityUpdate {
    pub not_mounted_paths: Vec<String>,
    pub not_usable_paths: Vec<String>,
    pub available_paths: Vec<String>,
    pub in_use_paths: Vec<String>,
    pub finished_paths: Vec<String>,
}

fn build_capacity_update(
    _disk_archiver: &DiskArchiver,
    archival_disk_map: &HashMap<String, Disk>,
) -> CapacityUpdate {
    // create some containers to hold the status strings
    let mut not_mounted_paths = Vec::<String>::new();
    let mut not_usable_paths = Vec::<String>::new();
    let mut available_paths = Vec::<String>::new();
    let mut in_use_paths = Vec::<String>::new();
    let mut finished_paths = Vec::<String>::new();

    // sort the disks into their respective bins by status
    for (path, disk) in archival_disk_map {
        match disk.status {
            NotMounted => {
                not_mounted_paths.push(path.to_string());
            }
            NotUsable => {
                not_usable_paths.push(path.to_string());
            }
            Available => {
                available_paths.push(path.to_string());
            }
            InUse => {
                // determine the disk archive the disk belongs to
                let archive = disk
                    .archive
                    .clone()
                    .unwrap_or("Unknown Archive".to_string());
                // add the information to the In-Use paths
                let info = format!("{} ID:{} [{}]", path, disk.id, archive);
                in_use_paths.push(info);
            }
            Finished => {
                // determine the disk archive the disk belongs to
                let archive = disk
                    .archive
                    .clone()
                    .unwrap_or("Unknown Archive".to_string());
                // add the information to the Finished paths
                let info = format!("{} ID:{} [{}]", path, disk.id, archive);
                finished_paths.push(info);
            }
        }
    }

    // return the capacity update information to the caller
    CapacityUpdate {
        not_mounted_paths,
        not_usable_paths,
        available_paths,
        in_use_paths,
        finished_paths,
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
    if let Some(disk_archive) = disk_archiver.disk_archives.for_uuid(disk_archive_uuid) {
        context.insert("disk_archive", &disk_archive);
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

    let archival_disk_map = build_archival_disks_status(disk_archiver).await;
    let capacity_update = build_capacity_update(disk_archiver, &archival_disk_map);
    let CapacityUpdate {
        not_mounted_paths,
        not_usable_paths,
        available_paths,
        in_use_paths,
        finished_paths,
    } = capacity_update;
    context.insert("not_mounted_paths", &not_mounted_paths);
    context.insert("not_usable_paths", &not_usable_paths);
    context.insert("available_paths", &available_paths);
    context.insert("in_use_paths", &in_use_paths);
    context.insert("finished_paths", &finished_paths);

    render_email_body(tera, CLOSE_DISK_TEMPLATE, &context)
}

async fn build_disk_started_body(
    disk_archiver: &DiskArchiver,
    _label_path: &Path,
    disk: &JadeDisk,
) -> String {
    let tera = &disk_archiver.tera;
    let mut context: Context = Context::default();

    let hostname = &disk_archiver.host.host_name;
    context.insert("hostname", hostname);

    let disk_archive_uuid = &disk.disk_archive_uuid;
    if let Some(disk_archive) = disk_archiver.disk_archives.for_uuid(disk_archive_uuid) {
        context.insert("disk_archive", &disk_archive);
    } else {
        return TERA_RENDER_ERROR.to_string();
    }

    let email_disk: EmailDisk = disk.into();
    context.insert("disk", &email_disk);

    let device_path = &disk.device_path;
    let free_bytes = match get_free_space(device_path) {
        Ok(free_bytes) => free_bytes,
        Err(_) => {
            return TERA_RENDER_ERROR.to_string();
        }
    };
    context.insert("free_bytes", &free_bytes);

    render_email_body(tera, CREATE_DISK_TEMPLATE, &context)
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
        let msg = format!(
            "Unable to determine duration of Disk ID:{disk_id}; was {seconds} seconds (must be >0)"
        );
        error!(msg);
        return Err(DatamoveError::Other(msg.into()));
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

pub async fn send_email_disk_full(
    disk_archiver: &DiskArchiver,
    label_path: &Path,
    disk: &JadeDisk,
) -> Result<()> {
    // destructure our EmailConfig into useful variables
    let EmailConfig {
        enabled,
        from,
        host,
        password: _,
        port,
        reply_to,
        username: _,
    } = &disk_archiver.context.config.email_configuration;
    // if email is disabled, don't send any email
    if !enabled {
        warn!("In our current configuration, e-mail is disabled. No {DISK_FULL_SUBJECT} e-mail will be sent.");
        warn!("send_email_disk_full(): email_config.enabled = {enabled}");
        return Ok(());
    }
    // engage our Auto Memories Doll to write a letter for us
    let mut email = Message::builder()
        .from(from.parse()?)
        .reply_to(reply_to.parse()?);
    // obtain the list of human contacts to be informed
    let contacts = &disk_archiver.contacts.contacts;
    let contacts = contacts
        .iter()
        // keep the ones with the JADE_ADMIN and WINTER_OVER role
        .filter(|x| (x.role == ContactRole::JadeAdmin) || (x.role == ContactRole::WinterOver));
    // for each contact on the list
    for email_to in contacts {
        // log about the recipient
        let Contact {
            name,
            email: address,
            role: _role,
        } = email_to;
        info!("Sending a {DISK_FULL_SUBJECT} e-mail to: {name} <{address}>");
        // add the recipient to the e-mail
        email = email.to(email_to.into());
    }
    // set the subject of the e-mail
    email = email.subject(DISK_FULL_SUBJECT);
    // build and set the body to create the e-mail message
    let email_body = build_disk_closed_body(disk_archiver, label_path, disk).await;
    let message = email.body(email_body)?;
    // create an SmtpTransport to use for sending the message
    // let creds = Credentials::new(username.to_owned(), password.to_owned());
    // let mailer = SmtpTransport::relay(host)
    //     .unwrap()
    //     .credentials(creds)
    //     .port(*port)
    //     .build();
    let mailer = SmtpTransport::builder_dangerous(host)
        .port(*port)
        .tls(Tls::None)
        .build();
    // You've Got Mail!
    info!("Sending {DISK_FULL_SUBJECT} message.");
    mailer.send(&message)?;

    // indicate to the caller that we succeeded
    Ok(())
}

pub async fn send_email_disk_started(
    disk_archiver: &DiskArchiver,
    label_path: &Path,
    disk: &JadeDisk,
) -> Result<()> {
    // destructure our EmailConfig into useful variables
    let EmailConfig {
        enabled,
        from,
        host,
        password: _,
        port,
        reply_to,
        username: _,
    } = &disk_archiver.context.config.email_configuration;
    // if email is disabled, don't send any email
    if !enabled {
        warn!("In our current configuration, e-mail is disabled. No Streaming Archive Started e-mail will be sent.");
        warn!("send_email_disk_started(): email_config.enabled = {enabled}");
        return Ok(());
    }
    // engage our Auto Memories Doll to write a letter for us
    let mut email = Message::builder()
        .from(from.parse()?)
        .reply_to(reply_to.parse()?);
    // obtain the list of human contacts to be informed
    let contacts = &disk_archiver.contacts.contacts;
    let contacts = contacts
        .iter()
        // keep the ones with the JADE_ADMIN and WINTER_OVER role
        .filter(|x| (x.role == ContactRole::JadeAdmin) || (x.role == ContactRole::WinterOver));
    // for each contact on the list
    for email_to in contacts {
        // log about the recipient
        let Contact {
            name,
            email: address,
            role: _role,
        } = email_to;
        info!("Sending a Streaming Archive Started e-mail to: {name} <{address}>");
        // add the recipient to the e-mail
        email = email.to(email_to.into());
    }
    // set the subject of the e-mail
    let host_name = &disk_archiver.host.host_name;
    let device_path = &disk.device_path;
    let subject = format!("Streaming Archive Started on {}:{}", host_name, device_path);
    email = email.subject(subject);
    // build and set the body to create the e-mail message
    let email_body = build_disk_started_body(disk_archiver, label_path, disk).await;
    let message = email.body(email_body)?;
    // create an SmtpTransport to use for sending the message
    // let creds = Credentials::new(username.to_owned(), password.to_owned());
    // let mailer = SmtpTransport::relay(host)
    //     .unwrap()
    //     .credentials(creds)
    //     .port(*port)
    //     .build();
    let mailer = SmtpTransport::builder_dangerous(host)
        .port(*port)
        .tls(Tls::None)
        .build();
    // You've Got Mail!
    info!("Sending Streaming Archive Started message.");
    mailer.send(&message)?;

    // indicate to the caller that we succeeded
    Ok(())
}

//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------
//---------------------------------------------------------------------------------------------------------------------

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
        let mut tera = Tera::new("etc/**/*.tera")?;
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

        context.insert("rate_bytes_sec", &14_025_813i64);

        // Host:         jade01
        // Device:       /mnt/slot4
        // Free:         7,245,053,952 bytes
        // Total:        5,952,694,763,520 bytes

        context.insert("free_bytes", &7_245_053_952u64);
        context.insert("total_bytes", &5_952_694_763_520u64);

        // File Count:   32,351
        // Data Size:    5,945,177,808,502 bytes

        context.insert("num_file_pairs", &32_351i64);
        context.insert("size_file_pairs", &5_945_177_808_502i64);

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

        let result = tera.render(CLOSE_DISK_TEMPLATE, &context)?;

        let expected = include_str!("../../tests/data/test_tera_template_close_archive.txt");

        assert_eq!(expected, result);
        Ok(())
    }

    #[test]
    fn test_tera_template_create_archive() -> Result<()> {
        let mut tera = Tera::new("etc/**/*.tera")?;
        tera.register_filter("comma", comma_separated_filter);

        let mut context: Context = Context::default();

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

        context.insert(
            "disk",
            &EmailDisk {
                id: 1912,
                label: "IceCube_2_2025_0009".to_string(),
                copy_id: 2,
                uuid: "f69db55c-365c-40f9-9695-c13fa37343cf".to_string(),
                date_created: "Feb 16, 2025 7:11:17 AM".to_string(),
                date_updated: "Feb 16, 2025 7:11:17 AM".to_string(),
                path: "/mnt/slot10".to_string(),
            },
        );

        context.insert("free_bytes", &5_952_677_957_632u64);

        let result = tera.render(CREATE_DISK_TEMPLATE, &context)?;

        let expected = include_str!("../../tests/data/test_tera_template_create_archive.txt");

        assert_eq!(expected, result);
        Ok(())
    }
}
