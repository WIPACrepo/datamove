// disk_archiver.rs

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{NaiveDateTime, Utc};
use rand::seq::SliceRandom;
use regex::Regex;
use tera::Tera;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use crate::adhoc::utils::next_file;
use crate::config::{
    load_contacts, load_data_streams, load_disk_archives, Contacts, DataStream, DataStreams,
    DiskArchive, DiskArchives,
};
use crate::metadata::{ArchivalDiskFile, ArchivalDiskMetadata};
use crate::sps::email::{
    comma_separated_filter, compile_templates, send_email_disk_full, send_email_disk_started,
};
use crate::sps::jade_db::service::disk::add_file_pair as add_file_pair_to_disk;
use crate::sps::jade_db::service::disk::close as close_disk;
use crate::sps::jade_db::service::disk::create as create_disk;
use crate::sps::jade_db::service::disk::find_archived_file_pair_uuids;
use crate::sps::jade_db::service::disk::find_by_uuid as find_disk_by_uuid;
use crate::sps::jade_db::service::disk::find_file_pair as find_file_pair_on_disk;
use crate::sps::jade_db::service::disk::find_open as find_open_disk;
use crate::sps::jade_db::service::disk::get_removable_files;
use crate::sps::jade_db::service::disk::get_serial_number_age_in_secs;
use crate::sps::jade_db::service::disk::JadeDisk;
use crate::sps::jade_db::service::disk_label::get_next_label;
use crate::sps::jade_db::service::file_pair::find_by_uuid as find_file_pair_by_uuid;
use crate::sps::jade_db::service::file_pair::JadeFilePair;
use crate::sps::jade_db::service::host::ensure_host;
use crate::sps::jade_db::service::host::JadeHost;
use crate::sps::utils::crypto::compute_sha512;
use crate::sps::utils::lsblk::get_serial_for_mountpoint;
use crate::sps::utils::{
    count_uuid_labels, create_directory, flush_to_disk, get_file_count, get_free_space,
    get_oldest_file_age_in_secs, get_oldest_file_date, is_mount_point, is_writable_dir, move_file,
    touch_label,
};
use crate::status::sps::{Disk, DiskArchiverComponentStatus, DiskArchiverStatus, DiskStatus};
use crate::{sps::context::Context, status::sps::DiskArchiverWorkerStatus};

use crate::error::{DatamoveError, Result};

pub const CLOSE_SEMAPHORE_NAME: &str = "close.me";

/// if we try to find a disk more than ten times, something has gone terribly wrong
pub const MAX_TILT_COUNT: i32 = 10;

pub type SharedFlag = Arc<Mutex<bool>>;
pub type SharedStatus = Arc<Mutex<DiskArchiverComponentStatus>>;

#[derive(Clone)]
pub struct DiskArchiver {
    pub contacts: Contacts,
    pub context: Context,
    pub data_streams: DataStreams,
    pub disk_archives: DiskArchives,
    pub host: JadeHost,
    pub shutdown: SharedFlag,
    pub status: SharedStatus,
    pub tera: Tera,
}

impl DiskArchiver {
    pub async fn new(context: Context) -> Self {
        let host = ensure_host(&context.db_pool, &context.hostname)
            .await
            .expect("Unable to determine JadeHost running DiskArchiver");

        let contacts_json_path = &context.config.sps_disk_archiver.contacts_json_path;
        let contacts = load_contacts(contacts_json_path)
            .expect("Unable to load contacts from JSON configuration");

        let data_streams_json_path = &context.config.sps_disk_archiver.data_streams_json_path;
        let data_streams = load_data_streams(data_streams_json_path)
            .expect("Unable to load data_streams from JSON configuration")
            .data_streams;

        let disk_archives_json_path = &context.config.sps_disk_archiver.disk_archives_json_path;
        let disk_archives = load_disk_archives(disk_archives_json_path)
            .expect("Unable to load disk_archives from JSON configuration")
            .disk_archives;

        let tera_template_glob = &context.config.sps_disk_archiver.tera_template_glob;
        let mut tera =
            compile_templates(tera_template_glob).expect("Unable to compile Tera templates");
        tera.register_filter("comma", comma_separated_filter);

        Self {
            contacts,
            context,
            data_streams: DataStreams(data_streams),
            disk_archives: DiskArchives(disk_archives),
            host,
            shutdown: Arc::new(Mutex::new(false)),
            status: Arc::new(Mutex::new(DiskArchiverComponentStatus::Ok)),
            tera,
        }
    }

    pub async fn get_status(&self) -> DiskArchiverStatus {
        build_disk_archiver_status(self).await
    }

    pub async fn run(&self) {
        info!("DiskArchiver work loop begins.");
        // find out how long we need to sleep between work cycles
        let work_cycle_sleep_seconds = self
            .context
            .config
            .sps_disk_archiver
            .work_cycle_sleep_seconds;

        // flag: should we stop working and gracefully shut the program down?
        let mut graceful_shutdown = false;

        // until a shutdown is requested
        while !graceful_shutdown {
            // perform the work of the disk archiver
            if let Err(e) = do_work_cycle(self).await {
                error!("Error detected during do_work_cycle(): {e}");
                error!("Will shut down the DiskArchiver.");
                *self.shutdown.lock().unwrap() = true;
                *self.status.lock().unwrap() = DiskArchiverComponentStatus::FullStop;
                break;
            }
            // sleep until the next work cycle
            info!("Will sleep for {} seconds.", work_cycle_sleep_seconds);
            sleep(Duration::from_secs(work_cycle_sleep_seconds)).await;
            // check if we need to shut down before starting the next work cycle
            graceful_shutdown = match self.shutdown.lock() {
                Ok(flag) => *flag,
                Err(x) => {
                    error!("Unable to lock SharedFlag shutdown: {x}");
                    true
                }
            };
        }

        // log about the fact that we received a shutdown command
        info!("Initiating graceful shutdown of DiskArchiver.");
    }

    pub fn request_shutdown(&self) {
        info!("DiskArchiver shutdown requested.");
        // if we can get hold of the shutdown flag
        let mut flag = match self.shutdown.lock() {
            Ok(flag) => flag,
            Err(x) => {
                error!("Unable to lock SharedFlag shutdown: {x}");
                return;
            }
        };
        // raise the flag to indicate that we want to shut down
        *flag = true;
    }
}

/// archive a file pair to all the archives that need N copies of it
async fn archive_file_pair_to_archives(
    disk_archiver: &DiskArchiver,
    file_pair_path: &Path,
    jade_file_pair: &JadeFilePair,
    data_stream: &DataStream,
    archive_names: &[String],
) -> Result<()> {
    let archive_file = &jade_file_pair
        .archive_file
        .clone()
        .expect("jade_file_pair.archive_file IS null");
    info!(
        "Archiving '{}' to archives: {}",
        archive_file,
        archive_names.join(", ")
    );
    // for each disk archive
    for disk_archive in &disk_archiver.disk_archives {
        // if this archive appears in the list of destination archives
        if archive_names.contains(&disk_archive.name) {
            // for each copy we want to make
            for copy_id in 1..=disk_archive.num_copies {
                // send it to Copy:{copy_id} of Archive:{disk_archive}
                archive_file_pair_to_disk(
                    disk_archiver,
                    file_pair_path,
                    jade_file_pair,
                    data_stream,
                    disk_archive,
                    copy_id,
                )
                .await?;
            }
        }
    }
    // tell the caller that the file pair was successfully archived to all disks
    Ok(())
}

// async fn archive_file_pair_to_archives(
//     disk_archiver: &DiskArchiver,
//     file_pair_path: &Path,
//     jade_file_pair: &JadeFilePair,
//     data_stream: &DataStream,
//     archive_names: &[String],
// ) -> Result<()> {
//     let archive_file = &jade_file_pair
//         .archive_file
//         .clone()
//         .expect("jade_file_pair.archive_file IS null");
//     info!(
//         "Archiving '{}' to archives: {}",
//         archive_file,
//         archive_names.join(", ")
//     );

//     // Collect all archive tasks
//     let archive_futures: Vec<_> = disk_archiver
//         .disk_archives
//         .0
//         .iter()
//         .filter(|disk_archive| archive_names.contains(&disk_archive.name))
//         .flat_map(|disk_archive| {
//             (1..=disk_archive.num_copies).map(move |copy_id| {
//                 archive_file_pair_to_disk(
//                     disk_archiver,
//                     file_pair_path,
//                     jade_file_pair,
//                     data_stream,
//                     disk_archive,
//                     copy_id,
//                 )
//             })
//         })
//         .collect();

//     // Await all tasks concurrently
//     for result in join_all(archive_futures)
//         .await
//         .into_iter()
//         .collect::<Vec<_>>()
//     {
//         result?
//     }

//     Ok(())
// }

/// archive a file pair to a specific copy of a specific archive
/// (i.e.: IceCube Copy 1)
async fn archive_file_pair_to_disk(
    disk_archiver: &DiskArchiver,
    file_pair_path: &Path,
    jade_file_pair: &JadeFilePair,
    data_stream: &DataStream,
    disk_archive: &DiskArchive,
    copy_id: u64,
) -> Result<()> {
    let archive_file = &jade_file_pair
        .archive_file
        .clone()
        .expect("jade_file_pair.archive_file IS null");
    info!(
        "Archiving '{}' to '{}:Copy {}'",
        archive_file, disk_archive.name, copy_id
    );
    // if we've already archived this file, just return success
    trace!(
        "archive_file_pair_to_disk(Copy:{}) -- 1 -- checking DB",
        copy_id
    );
    if is_already_archived_to_copy(disk_archiver, jade_file_pair, disk_archive, copy_id).await? {
        info!(
            "'{}' already archived to '{}:Copy {}'",
            archive_file, disk_archive.name, copy_id
        );
        return Ok(());
    }
    // define a tilt count
    let mut tilt_count = 0;
    // until we find a disk to which we can write our file pair
    trace!(
        "archive_file_pair_to_disk(Copy:{}) -- 2 -- finding disk",
        copy_id
    );
    let mut dest_disk: Option<JadeDisk> = None;
    while dest_disk.is_none() {
        trace!(
            "archive_file_pair_to_disk(Copy:{}) -- 2.1 -- entering loop",
            copy_id
        );
        // increment the tilt counter
        tilt_count += 1;
        // if the tilt counter is too high
        if tilt_count >= MAX_TILT_COUNT {
            error!("tilt_count:{} >= MAX_TILT_COUNT:{}; been going around this disk loop too many times", tilt_count, MAX_TILT_COUNT);
            return Err(DatamoveError::Critical(
                "Failed to find a destination disk to archive a file pair.".into(),
            ));
        }
        // find or create an archival disk to write the file pair to
        {
            trace!(
                "archive_file_pair_to_disk(Copy:{}) -- 2.2 -- find or create copy",
                copy_id
            );
            let jade_disk =
                find_or_create_archive_copy(disk_archiver, disk_archive, copy_id).await?;
            dest_disk = Some(jade_disk.clone());
            trace!(
                "archive_file_pair_to_disk(Copy:{}) -- 2.2.1 -- cloned and close scope",
                copy_id
            );
        }
        trace!(
            "archive_file_pair_to_disk(Copy:{}) -- 2.2.2 -- 2nd clone and continue",
            copy_id
        );
        let jade_disk = dest_disk
            .clone()
            .expect("Somehow we found or created a None for a JadeDisk");
        // make sure the disk we found is physically present and usable
        trace!(
            "archive_file_pair_to_disk(Copy:{}) -- 2.3 -- is_okay_to_archive_to",
            copy_id
        );
        if !is_okay_to_archive_to(&jade_disk) {
            let msg = format!(
                "{} is NOT OK! The database told us Disk {}:{} (Copy {}) ({}) could be used, but it can't be used!",
                jade_disk.device_path,
                jade_disk.jade_disk_id,
                jade_disk.device_path,
                jade_disk.copy_id,
                jade_disk.uuid,
            );
            error!("{msg}");
            return Err(DatamoveError::Critical(msg));
        }
        // determine how much space is available and how much we need
        trace!(
            "archive_file_pair_to_disk(Copy:{}) -- 2.4 -- archive headroom",
            copy_id
        );
        let archive_headroom = disk_archiver
            .context
            .config
            .sps_disk_archiver
            .archive_headroom;
        let space_available = get_free_space(&jade_disk.device_path)? - archive_headroom;
        let space_needed = jade_file_pair
            .archive_size
            .expect("jade_file_pair.archive_size IS null") as u64;
        let archive_file = jade_file_pair
            .archive_file
            .clone()
            .expect("jade_file_pair.archive_file IS null");
        // if we don't have sufficient room on this disk
        if space_available < space_needed {
            // log about it
            info!(
                "{} ({} bytes free) does not have sufficient space for {} ({} bytes)",
                jade_disk.device_path, space_available, archive_file, space_needed
            );
            // close the disk
            // let pool = &disk_archiver.context.db_pool;
            // close_disk(pool, &jade_disk).await?;
            close_disk_by_path(disk_archiver, &jade_disk.device_path).await?;
            // try again
            continue;
        }
        // yay, we found a disk to which we can write our file pair
        trace!(
            "archive_file_pair_to_disk(Copy:{}) -- 2.5 -- returning candidate disk",
            copy_id
        );
        dest_disk = Some(jade_disk);
    }
    // if the disk is marked bad or on-hold, just bail out now
    trace!(
        "archive_file_pair_to_disk(Copy:{}) -- 3 -- checking disk",
        copy_id
    );
    let jade_disk = dest_disk.expect("How did None escape the dest_disk loop?");
    if jade_disk.bad {
        let msg = format!(
            "DiskService returned Disk {} ({}) which is marked BAD.",
            jade_disk.jade_disk_id, jade_disk.uuid,
        );
        error!("{msg}");
        return Err(DatamoveError::Critical(msg));
    }
    if jade_disk.on_hold {
        let msg = format!(
            "DiskService returned Disk {} ({}) which is marked ON-HOLD.",
            jade_disk.jade_disk_id, jade_disk.uuid,
        );
        error!("{msg}");
        return Err(DatamoveError::Critical(msg));
    }
    // determine the directory where we'll copy the file
    trace!(
        "archive_file_pair_to_disk(Copy:{}) -- 4 -- computing paths",
        copy_id
    );
    let disk_path = Path::new(&jade_disk.device_path);
    let date_modified_origin = jade_file_pair
        .date_modified_origin
        .expect("jade_file_pair.date_modified_origin IS null");
    let archival_path = data_stream.compute_data_warehouse_path(&date_modified_origin);
    let dest_dir_path = disk_path.join(archival_path);
    let file_pair_name = file_pair_path
        .file_name()
        .expect("File pair filename cannot be represented as UTF-8");
    let dest_path = dest_dir_path.join(file_pair_name);
    // copy the file pair to the destination archival disk (with チェックサム)
    trace!(
        "archive_file_pair_to_disk(Copy:{}) -- 5 -- copying file",
        copy_id
    );
    create_directory(&dest_dir_path)?;
    fs::copy(file_pair_path, &dest_path)?;
    flush_to_disk(&dest_path)?;
    trace!(
        "archive_file_pair_to_disk(Copy:{}) -- 6 -- computing checksum",
        copy_id
    );
    let dest_checksum = compute_sha512(&dest_path)?;
    let archive_checksum = jade_file_pair
        .archive_checksum
        .as_ref()
        .expect("jade_file_pair.archive_checksum IS null")
        .clone();
    if dest_checksum != archive_checksum {
        let msg = format!(
            "Checksum mismatch for file: {}\nDatabase checksum:  {}\nDisk checksum:      {}",
            dest_path.to_string_lossy(),
            archive_checksum,
            dest_checksum
        );
        error!("{msg}");
        return Err(DatamoveError::ChecksumError {
            expected: archive_checksum,
            actual: dest_checksum,
        });
    }
    // write the metadata for the file pair to the destination archival disk
    trace!(
        "archive_file_pair_to_disk(Copy:{}) -- 7 -- writing file_pair metadata",
        copy_id
    );
    let archival_disk_file = create_archival_disk_file(disk_archiver, jade_file_pair).await;
    let device_root = &jade_disk.device_path;
    let uuid = &jade_file_pair
        .jade_file_pair_uuid
        .clone()
        .expect("jade_file_pair.jade_file_pair_uuid IS null");
    save_archival_disk_file(device_root, uuid, &archival_disk_file)?;
    // update the database to indicate
    trace!(
        "archive_file_pair_to_disk(Copy:{}) -- 8 -- updating Disk <-> FilePair mapping in database",
        copy_id
    );
    let pool = &disk_archiver.context.db_pool;
    add_file_pair_to_disk(pool, &jade_disk, jade_file_pair).await?;
    // inform the caller that all went well
    trace!("archive_file_pair_to_disk(Copy:{}) -- 9 -- done", copy_id);
    trace!(
        "Done -- Archiving '{}' to '{}:Copy {}'",
        archive_file,
        disk_archive.name,
        copy_id
    );
    Ok(())
}

/// archive everything in the inbox to the disk archives they are destined for
async fn archive_file_pairs_to_archives(disk_archiver: &DiskArchiver) -> Result<()> {
    info!("DiskArchiver begins archiving inbox file pairs to archival disks.");
    let work_limit_break = disk_archiver
        .context
        .config
        .sps_disk_archiver
        .work_limit_break;
    // determine where we're going to be working with files
    let inbox_path = Path::new(&disk_archiver.context.config.sps_disk_archiver.inbox_dir);
    let outbox_path = Path::new(&disk_archiver.context.config.sps_disk_archiver.outbox_dir);
    let quarantine_path = Path::new(
        &disk_archiver
            .context
            .config
            .sps_disk_archiver
            .problem_files_dir,
    );
    let work_path = Path::new(&disk_archiver.context.config.sps_disk_archiver.work_dir);
    // while there are still files to work with
    let mut files_processed = 0;
    while let Some(file_pair_path) = next_file(inbox_path, work_path)? {
        // extract the uuid from the file name
        let Some(jade_file_pair_uuid) = parse_uuid_from_filename(&file_pair_path) else {
            // if there was no UUID, quarantine the file and move on to the next one
            error!("Unable to determine UUID for: {}", file_pair_path.display());
            move_file(&file_pair_path, quarantine_path);
            continue;
        };

        // load the data about the file from the database
        let pool = &disk_archiver.context.db_pool;
        let jade_file_pair = find_file_pair_by_uuid(pool, &jade_file_pair_uuid).await?;

        // extract the data stream uuid
        let Some(jade_data_stream_uuid) = &jade_file_pair.jade_data_stream_uuid else {
            // if there was no UUID, quarantine the file and move on to the next one
            error!(
                "Unable to determine Data Stream for: {}:{:#?}",
                jade_file_pair.jade_file_pair_id, file_pair_path,
            );
            move_file(&file_pair_path, quarantine_path);
            continue;
        };

        let Some(data_stream) = disk_archiver.data_streams.for_uuid(jade_data_stream_uuid) else {
            // if there is no DataStream that matches the UUID, quarantine the file and move on to the next one
            error!(
                "Attempted to find DataStream for FilePair: {}:{:#?}",
                jade_file_pair.jade_file_pair_id, file_pair_path,
            );
            error!(
                "No data stream exists for DataStream UUID: {}",
                jade_data_stream_uuid,
            );
            move_file(&file_pair_path, quarantine_path);
            continue;
        };

        // determine which archives will receieve a file from this data stream
        let archive_names = &data_stream.archives;
        match archive_file_pair_to_archives(
            disk_archiver,
            &file_pair_path,
            &jade_file_pair,
            &data_stream,
            archive_names,
        )
        .await
        {
            Ok(_) => {
                // success, continue normally
            }

            Err(e @ DatamoveError::Critical(_)) => {
                // handle critical error by returning immediately
                error!(
                    "Critical error encountered while archiving FilePair {}: {:#?}",
                    jade_file_pair.jade_file_pair_id, file_pair_path,
                );
                error!("Critical error was: {}", e);
                return Err(e);
            }

            Err(error) => {
                // if there was an error attemping to archive this file
                error!(
                    "Error while archiving FilePair: {}:{:#?}",
                    jade_file_pair.jade_file_pair_id, file_pair_path,
                );
                error!("Error was: {error}");
                move_file(&file_pair_path, quarantine_path);
                continue;
            }
        }

        // all finished with the file, so let's move it to the outbox
        info!("Archived: {:#?}", &file_pair_path);
        move_file(&file_pair_path, outbox_path);
        files_processed += 1;

        // check if we got shut down and need to exit early
        let graceful_shutdown = match disk_archiver.shutdown.lock() {
            Ok(flag) => *flag,
            Err(x) => {
                error!("Unable to lock SharedFlag shutdown: {x}");
                true
            }
        };
        if graceful_shutdown {
            info!("Graceful shutdown requested. Will stop processing inbox file pairs.");
            break;
        }

        // check if we need to take a break
        if files_processed >= work_limit_break {
            info!("Performed {work_limit_break} units of work. Will take a break.");
            break;
        }
    }

    // let the caller know we succeeded at archiving files to disk
    info!("DiskArchiver finished archiving inbox file pairs to archival disks.");
    Ok(())
}

async fn build_archival_disk_status(disk_archiver: &DiskArchiver, disk_path: &str) -> Disk {
    // ostensibly, this is a path to an archival disk. now let's put it
    // through the gauntlet and see what we've really got here...
    let path = Path::new(disk_path);
    // if the path doesn't exist at all, we can't use it
    if !path.exists() {
        debug!("Archival disk path '{}' does not exist.", path.display());
        return Disk::for_status(DiskStatus::NotUsable);
    }
    // if we can't write to the path, we can't use it
    let temp_path = path.join(".temp_check_writable");
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
    {
        Ok(_) => {
            let _ = fs::remove_file(&temp_path);
        }
        Err(_) => {
            debug!("Archival disk path '{}' is not writable.", path.display());
            return Disk::for_status(DiskStatus::NotUsable);
        }
    }
    // if the path isn't a mount point, it's not mounted
    if !is_mount_point(disk_path) {
        return Disk::for_status(DiskStatus::NotMounted);
    }
    // let's see if the mounted disk has any labels
    let disk_labels = match read_disk_labels(path) {
        Ok(labels) => labels,
        Err(_) => {
            debug!(
                "Unable to read disk labels from archival disk path '{}'",
                path.display()
            );
            return Disk::for_status(DiskStatus::NotUsable);
        }
    };
    // if there are too many labels, this disk is not usable
    if disk_labels.len() > 1 {
        debug!(
            "Archival disk path '{}' contains multiple UUID disk label files.",
            path.display()
        );
        return Disk::for_status(DiskStatus::NotUsable);
    }
    // if there are no labels, this disk is available
    if disk_labels.is_empty() {
        return Disk::for_status(DiskStatus::Available);
    }
    // there is exactly one label, so look up database information about that disk
    let pool = &disk_archiver.context.db_pool;
    let find_uuid = &disk_labels[0];
    let disk = match find_disk_by_uuid(pool, find_uuid).await {
        Ok(disk) => disk,
        Err(e) => {
            error!("Did not find JadeDisk for uuid '{find_uuid}' due to: {e}.");
            return Disk::for_status(DiskStatus::NotUsable);
        }
    };
    // return the fully realized disk status to the caller
    match Disk::try_from(disk) {
        Ok(mut disk_status) => {
            // map the `disk_archive_uuid` from the database to the description of the archive from `diskArchives.json`
            let archive_uuid = disk_status
                .archive
                .clone()
                .expect("jade_disk.disk_archive_uuid IS null");
            if let Some(disk_archive) = disk_archiver.disk_archives.for_uuid(&archive_uuid) {
                disk_status.archive = Some(disk_archive.description);
            }
            // return the status version of the disk
            disk_status
        }
        Err(e) => {
            error!(
                "Error when reading database table jade_disk for uuid '{}'.",
                find_uuid
            );
            error!("Error was: {}", e);
            Disk::for_status(DiskStatus::NotUsable)
        }
    }
}

pub async fn build_archival_disks_status(disk_archiver: &DiskArchiver) -> HashMap<String, Disk> {
    // create a hashmap to hold our archival disks
    let mut archival_disks = HashMap::new();

    // for each configured disk path
    let disk_paths = get_disk_paths(disk_archiver);
    for disk_path in disk_paths {
        // determine the status of the disk and put it in the map
        let disk = build_archival_disk_status(disk_archiver, &disk_path).await;
        archival_disks.insert(disk_path, disk);
    }

    // return the hashmap to the caller
    archival_disks
}

/// TODO: documentation comment
pub async fn build_disk_archiver_status(disk_archiver: &DiskArchiver) -> DiskArchiverStatus {
    let cache_dir = &disk_archiver.context.config.sps_disk_archiver.cache_dir;
    let inbox_dir = &disk_archiver.context.config.sps_disk_archiver.inbox_dir;
    let problem_files_dir = &disk_archiver
        .context
        .config
        .sps_disk_archiver
        .problem_files_dir;

    let cache_age = match get_oldest_file_age_in_secs(cache_dir) {
        Ok(age) => age,
        Err(e) => {
            error!(
                "Unable to determine age of oldest file in the cache directory: {}",
                e
            );
            0
        }
    };

    let inbox_count = match get_file_count(inbox_dir) {
        Ok(age) => age,
        Err(e) => {
            error!(
                "Unable to determine count of files in the inbox directory: {}",
                e
            );
            0
        }
    };

    let inbox_age = match get_oldest_file_age_in_secs(inbox_dir) {
        Ok(age) => age,
        Err(e) => {
            error!("Unable to determine age of oldest file in the inbox directory: {e}");
            0
        }
    };

    let problem_file_count = match get_file_count(problem_files_dir) {
        Ok(age) => age,
        Err(e) => {
            error!(
                "Unable to determine count of files in the problem_files directory: {}",
                e
            );
            0
        }
    };

    let archival_disks = build_archival_disks_status(disk_archiver).await;

    let status = match &*disk_archiver.status.lock().unwrap() {
        DiskArchiverComponentStatus::FullStop => Some("FULL_STOP".to_string()),
        DiskArchiverComponentStatus::Ok => Some("OK".to_string()),
    };

    DiskArchiverStatus {
        workers: vec![DiskArchiverWorkerStatus {
            archival_disks,
            inbox_count,
        }],
        cache_age,
        inbox_age,
        problem_file_count,
        message: None,
        status,
    }
}

pub async fn clean_disk_cache(disk_archiver: &DiskArchiver) -> Result<()> {
    // goal: clean the disk cache of files we no longer need to retain
    let cache_dir = &disk_archiver.context.config.sps_disk_archiver.cache_dir;
    info!("Cleaning disk cache: {}", cache_dir);

    // get all the UUIDs of the files currently on disk
    let cache_path = PathBuf::from(cache_dir);
    let disk_set = extract_uuids_from_cache(&cache_path)?;
    info!("Cache: Found {} files to check.", disk_set.len());

    // determine the date of the oldest file in the cache
    let oldest_file_date = get_oldest_file_date(cache_dir)?;
    if oldest_file_date.is_none() {
        info!("Disk cache contains no files.");
        return Ok(());
    }
    let cache_date = oldest_file_date.unwrap();

    // ask the database which files are copied to N archival disks, limited to files
    // contained on the finished disks that are currently loaded on the JADE machine
    let pool = &disk_archiver.context.db_pool;
    // TODO: remove next line
    // let disk_ids = get_loaded_disk_ids(disk_archiver).await;
    let required_copies = get_required_copies(disk_archiver)?;
    // TODO: remove next line
    // let database_set = get_removable_files(pool, &disk_ids, required_copies).await?;
    let database_set = get_removable_files(pool, cache_date, required_copies).await?;
    info!("DB: Found {} files ready for removal.", database_set.len());

    // take the intersection of the files we've got and the files we can delete
    let delete_set: HashSet<String> = disk_set.intersection(&database_set).cloned().collect();
    info!("Remove: Found {} files to be removed.", disk_set.len());
    remove_uuids_from_cache(&cache_path, &delete_set)?;

    // indicate to the caller that we successfully cleaned the disk cache
    info!("Disk cache cleaning complete.");
    Ok(())
}

pub async fn close_disk_by_path(disk_archiver: &DiskArchiver, disk_path: &str) -> Result<()> {
    // close the disk on the provided mount path
    info!("Closing disk: {}", disk_path);
    // determine the UUID label of the disk
    let path = Path::new(disk_path);
    let labels = read_disk_labels(path)?;
    if labels.is_empty() {
        let msg =
            format!("Attempted to read_disk_labels for {disk_path}, but no labels were found!");
        error!(msg);
        return Err(DatamoveError::Critical(msg));
    }
    let find_uuid = &labels[0];
    // look up the disk in the database
    let pool = &disk_archiver.context.db_pool;
    let disk = find_disk_by_uuid(pool, find_uuid).await?;
    // ensure each file pair on the disk has written metadata
    ensure_file_pair_metadata(disk_archiver, &disk).await?;
    // write disk metadata to the UUID label
    let label_path = path.join(find_uuid);
    if let Err(e) = write_archival_disk_metadata(&label_path, &disk) {
        let msg = format!(
            "Unable to write disk metadata to disk {disk_path} label file {find_uuid}: {e}."
        );
        error!(msg);
        return Err(DatamoveError::Critical(msg));
    }
    // close the disk
    close_disk(pool, &disk).await?;
    // reload the disk from the database
    let disk = find_disk_by_uuid(pool, find_uuid).await?;
    // send an email about the disk closure
    send_email_disk_full(disk_archiver, &label_path, &disk).await?;
    // indicate to the caller that we succeeded
    Ok(())
}

pub async fn close_on_semaphore(disk_archiver: &DiskArchiver) -> Result<()> {
    info!("Checking for close semaphores on all archival disks");
    // for each disk path
    let disk_paths = get_disk_paths(disk_archiver);
    for disk_path in disk_paths {
        // determine the path of the close semaphore for this disk
        let path = Path::new(&disk_path);
        let close_semaphore_path = path.join(CLOSE_SEMAPHORE_NAME);
        // determine if the close semaphore exists or not
        let exists = match std::fs::exists(&close_semaphore_path) {
            Ok(exists) => exists,
            Err(e) => {
                error!(
                    "Unable to determine if close semaphore '{}' exists: {e}",
                    close_semaphore_path.display()
                );
                continue;
            }
        };
        // if the close semaphore exists
        if exists {
            // close the disk
            info!("Found close semaphore: {}", close_semaphore_path.display());
            close_disk_by_path(disk_archiver, &disk_path).await?;
            // delete the semaphore from the disk
            info!(
                "Removing close semaphore: {}",
                close_semaphore_path.display()
            );
            fs::remove_file(close_semaphore_path)?;
        }
    }
    // tell the caller we succeeded
    Ok(())
}

async fn create_archive_copy(
    disk_archiver: &DiskArchiver,
    disk_archive: &DiskArchive,
    copy_id: u64,
) -> Result<()> {
    info!(
        "Creating a {}:Copy {} disk archive to use.",
        disk_archive.name, copy_id
    );
    // find available disk
    let Some(disk_path) = find_available_disk(disk_archive) else {
        let msg =
            "create_archive_copy(): Unable to find available disk to create archive.".to_string();
        error!("{msg}");
        return Err(DatamoveError::Critical(msg));
    };
    // get the serial number of the disk
    let Some(serial_number) = get_serial_for_mountpoint(&disk_path) else {
        let msg =
            format!("create_archive_copy(): Unable to obtain serial for mountpoint '{disk_path}'.");
        error!("{msg}");
        return Err(DatamoveError::Critical(msg));
    };
    // check the serial number for re-use
    let minimum_disk_age = disk_archiver
        .context
        .config
        .sps_disk_archiver
        .minimum_disk_age_seconds;
    let pool = &disk_archiver.context.db_pool;
    let age = get_serial_number_age_in_secs(pool, &serial_number).await?;
    if age < minimum_disk_age {
        let msg = format!(
            "Serial Number:'{}' re-used TOO SOON! Age:{}s (Required: >={}s)",
            serial_number, age, minimum_disk_age
        );
        return Err(DatamoveError::Critical(msg));
    }
    // generate a Label (i.e.: IceCube_2_2025_0008)
    let label = get_next_label(pool, disk_archive, copy_id).await?;
    // generate a UUID -> label the disk
    let uuid = Uuid::new_v4().to_string();
    let path = PathBuf::from(&disk_path);
    let label_path = path.join(&uuid);
    touch_label(&label_path)?;
    // create a database entry
    let capacity = get_free_space(&disk_path)?;
    let disk_archive_uuid = &disk_archive.uuid;
    let jade_host_id = disk_archiver.host.jade_host_id;
    let now: NaiveDateTime = Utc::now().naive_utc();
    let jade_disk_id = create_disk(
        pool,
        &JadeDisk {
            jade_disk_id: -1, // doesn't matter
            bad: false,
            capacity: capacity as i64,
            closed: false,
            copy_id: copy_id as i32,
            date_created: now,
            date_updated: now,
            device_path: disk_path.clone(),
            label,
            on_hold: false,
            uuid: uuid.clone(),
            version: 1,
            jade_host_id,
            disk_archive_uuid: disk_archive_uuid.clone(),
            serial_number,
            hardware_metadata: "".to_string(),
        },
    )
    .await?;
    // reload the created disk from the database
    let disk = find_disk_by_uuid(pool, &uuid).await?;
    // log about creating the disk
    info!(
        "Disk {} ({}) created as Host {} DiskArchive {}:{} Copy {} at {}",
        jade_disk_id,
        uuid,
        disk_archiver.host.host_name,
        disk_archive.id,
        disk_archive.description,
        copy_id,
        &disk_path
    );
    // email a 'Streaming Archive Started on' email about it
    send_email_disk_started(disk_archiver, &label_path, &disk).await?;
    // indicate to the caller that we succeeded
    Ok(())
}

pub async fn create_archival_disk_file(
    disk_archiver: &DiskArchiver,
    jade_file_pair: &JadeFilePair,
) -> ArchivalDiskFile {
    // compute the things we need to compute with the metadata
    let fetched_by_host = &disk_archiver.host.host_name;
    let date_verified = match jade_file_pair.date_verified {
        Some(date_verified) => date_verified.and_utc().timestamp_millis(),
        None => 0,
    };
    // capture the file metadata to be written to the disk
    ArchivalDiskFile {
        archive_checksum: jade_file_pair
            .archive_checksum
            .clone()
            .expect("jade_file_pair.archive_checksum IS null"),
        archive_file: jade_file_pair
            .archive_file
            .clone()
            .expect("jade_file_pair.archive_file IS null"),
        archive_size: jade_file_pair
            .archive_size
            .expect("jade_file_pair.archive_size IS null"),
        binary_file: jade_file_pair
            .binary_file
            .clone()
            .expect("jade_file_pair.binary_file IS null"),
        binary_size: jade_file_pair
            .binary_size
            .expect("jade_file_pair.binary_size IS null"),
        data_stream_uuid: jade_file_pair
            .jade_file_pair_uuid
            .clone()
            .expect("jade_file_pair.jade_file_pair_uuid IS null"),
        data_warehouse_path: jade_file_pair
            .data_warehouse_path
            .clone()
            .expect("jade_file_pair.data_warehouse_path IS null"),
        date_created: jade_file_pair
            .date_created
            .expect("jade_file_pair.date_created IS null")
            .and_utc()
            .timestamp_millis(),
        date_fetched: jade_file_pair
            .date_fetched
            .expect("jade_file_pair.date_fetched IS null")
            .and_utc()
            .timestamp_millis(),
        date_processed: jade_file_pair
            .date_processed
            .expect("jade_file_pair.date_processed IS null")
            .and_utc()
            .timestamp_millis(),
        date_updated: jade_file_pair
            .date_updated
            .expect("jade_file_pair.date_updated IS null")
            .and_utc()
            .timestamp_millis(),
        date_verified,
        disk_count: 0,
        fetch_checksum: jade_file_pair
            .fetch_checksum
            .clone()
            .expect("jade_file_pair.fetch_checksum IS null"),
        fetched_by_host: fetched_by_host.clone(),
        fingerprint: jade_file_pair
            .fingerprint
            .clone()
            .expect("jade_file_pair.fingerprint IS null"),
        metadata_file: jade_file_pair
            .metadata_file
            .clone()
            .expect("jade_file_pair.metadata_file IS null"),
        origin_checksum: jade_file_pair
            .origin_checksum
            .clone()
            .expect("jade_file_pair.origin_checksum IS null"),
        origin_modification_date: jade_file_pair
            .date_modified_origin
            .expect("jade_file_pair.date_modified_origin IS null")
            .and_utc()
            .timestamp_millis(),
        semaphore_file: jade_file_pair
            .semaphore_file
            .clone()
            .expect("jade_file_pair.semaphore_file IS null"),
        uuid: jade_file_pair
            .jade_file_pair_uuid
            .clone()
            .expect("jade_file_pair.uuid IS null"),
    }
}

pub async fn do_work_cycle(disk_archiver: &DiskArchiver) -> Result<()> {
    // start the work cycle
    info!("Starting DiskArchiver work cycle.");
    // check if the operator has requested manual close for any archival disks
    if let Err(e) = close_on_semaphore(disk_archiver).await {
        error!("Error occured closing archival disks with close semaphores: {e}");
        error!("No further work will be performed until the error is handled.");
        return Err(e);
    }
    // make sure any orphan files in working directory are reclaimed
    if let Err(e) = reclaim_abandoned_work(disk_archiver).await {
        error!("Error occured reclaiming abandoned work: {e}");
        error!("No further work will be performed until the error is handled.");
        return Err(e);
    }
    // archive files to archival disks
    if let Err(e) = archive_file_pairs_to_archives(disk_archiver).await {
        error!("Error occured archiving files to archival disks: {e}");
        error!("No further work will be performed until the error is handled.");
        return Err(e);
    }
    // clean the cache of any files included on N archival disks
    if let Err(e) = clean_disk_cache(disk_archiver).await {
        error!("Error occured cleaning the disk archival cache: {e}");
        error!("No further work will be performed until the error is handled.");
        return Err(e);
    }
    // finish the work cycle successfully
    info!("End of DiskArchiver work cycle.");
    Ok(())
}

pub async fn ensure_file_pair_metadata(
    disk_archiver: &DiskArchiver,
    disk: &JadeDisk,
) -> Result<()> {
    // ensure that every file pair archived to the disk has a metadata file
    info!(
        "Ensuring metadata for file pairs archived to {}:{} (Copy {})",
        disk.jade_disk_id, disk.uuid, disk.copy_id
    );
    // get a list of all the jade_file_pair_ids archvied to the provided disk
    let pool = &disk_archiver.context.db_pool;
    let jade_file_pair_uuids = find_archived_file_pair_uuids(pool, disk).await?;
    // for each jade_file_pair_id
    for jade_file_pair_uuid in jade_file_pair_uuids {
        // determine the path where the metadata file would live
        // TODO: probably want to consolidate this with save_archival_disk_file
        let device_root = &disk.device_path;
        let hex_1 = &jade_file_pair_uuid[0..1];
        let hex_2 = &jade_file_pair_uuid[1..2];
        let file_path = format!(
            "{}/metadata/{}/{}/{}.json",
            device_root, hex_1, hex_2, jade_file_pair_uuid
        );
        // if the file already exists, skip it
        if fs::exists(file_path)? {
            continue;
        }
        // whoops, we missed one; so let's generate it
        let jade_file_pair = find_file_pair_by_uuid(pool, &jade_file_pair_uuid).await?;
        info!(
            "Generating missing metadata for {}:{} ({})",
            jade_file_pair_uuid,
            jade_file_pair_uuid,
            jade_file_pair
                .archive_file
                .clone()
                .expect("jade_file_pair.archive_file IS null"),
        );
        let archival_disk_file = create_archival_disk_file(disk_archiver, &jade_file_pair).await;
        save_archival_disk_file(device_root, &jade_file_pair_uuid, &archival_disk_file)?;
    }
    // tell the caller that we succeeded
    info!(
        "Finished ensuring metadata for file pairs archived to {}:{} (Copy {})",
        disk.jade_disk_id, disk.uuid, disk.copy_id
    );
    Ok(())
}

fn extract_uuids_from_cache(cache_dir: &Path) -> Result<HashSet<String>> {
    // create the set we'll return to the caller
    let mut uuid_set = HashSet::new();
    // create a RegEx to match JADE's filename pattern `ukey_$UUID_`
    let uuid_regex = Regex::new(r"ukey_([a-f0-9-]{36})_").unwrap();
    // for each directory entry
    for entry in fs::read_dir(cache_dir)? {
        // get the filename
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        // extract the UUID
        if let Some(caps) = uuid_regex.captures(&file_name_str) {
            if let Some(uuid_match) = caps.get(1) {
                // add it to our set of UUIDs
                uuid_set.insert(uuid_match.as_str().to_string());
            } else {
                warn!(
                    "Cannot identify UUID in: {}",
                    cache_dir.join(file_name_str.to_string()).display()
                );
            }
        }
    }
    // return the set of filename UUIDs to the caller
    Ok(uuid_set)
}

/// find an open disk for archive X, copy Y, if it exists
async fn find_archive_copy(
    disk_archiver: &DiskArchiver,
    disk_archive: &DiskArchive,
    copy_id: u64,
) -> Result<Option<JadeDisk>> {
    // query the database for our disk
    let pool = &disk_archiver.context.db_pool;
    let jade_disk = find_open_disk(pool, disk_archiver, disk_archive, copy_id).await?;
    Ok(jade_disk)
}

/// search the paths of the archive to find an available (i.e.: empty) disk
fn find_available_disk(disk_archive: &DiskArchive) -> Option<String> {
    trace!("Found {} archival disk paths", disk_archive.paths.len());
    // shuffle the paths, so we search at random
    let mut rng = rand::rng();
    let mut paths = disk_archive.paths.clone();
    paths.shuffle(&mut rng);
    // for each path, run it through a gauntlet of checks
    for path in &paths {
        // if the path doesn't exist, the disk isn't mounted
        let path_exists = match fs::exists(path) {
            Ok(path_exists) => path_exists,
            Err(_) => {
                trace!("Unable to determine if {} exists; skipping.", path);
                continue;
            }
        };
        if !path_exists {
            trace!("{} is not mounted; skipping.", path);
            continue;
        }
        // if we can't write there, we can't use it
        if !is_writable_dir(path) {
            trace!("{} cannot be written to; skipping.", path);
            continue;
        }
        // if it's not a mount point, we shouldn't use it
        if !is_mount_point(path) {
            trace!("{} is NOT a mount point; skipping.", path);
            continue;
        }
        // if we can't read the filenames in the directory
        let uuid_count = match count_uuid_labels(path) {
            Ok(uuid_count) => uuid_count,
            Err(_) => {
                trace!("{} cannot be checked for UUID labels; skipping.", path);
                continue;
            }
        };
        // if there are any labels there
        if uuid_count > 0 {
            trace!("{} already has a UUID label; skipping.", path);
            continue;
        }
        // we survived the gauntlet; this disk is ready to be a JADE disk!
        return Some(path.clone());
    }
    // ut oh, we exhausted all possible paths; this is bad
    None
}

/// determine which disk can be used to write a file to archive X, copy Y
async fn find_or_create_archive_copy(
    disk_archiver: &DiskArchiver,
    disk_archive: &DiskArchive,
    copy_id: u64,
) -> Result<JadeDisk> {
    info!(
        "Finding a {}:Copy {} disk archive to use.",
        disk_archive.name, copy_id
    );
    let pool = &disk_archiver.context.db_pool;
    trace!(
        "Database connections: idle={}, size={}",
        pool.num_idle(),
        pool.size()
    );

    // if we're able to find an open disk for archive X, copy Y
    trace!(
        "find_or_create_archive_copy(Copy:{}) -- 1 -- checking DB",
        copy_id
    );
    if let Some(jade_disk) = find_archive_copy(disk_archiver, disk_archive, copy_id).await? {
        // return that disk to the caller
        return Ok(jade_disk);
    }
    // there was no open disk for archive X, copy Y, so let's create one
    create_archive_copy(disk_archiver, disk_archive, copy_id).await?;
    // try to find an open disk for archive X, copy Y
    match find_archive_copy(disk_archiver, disk_archive, copy_id).await? {
        // we found the one we created, so return it
        Some(jade_disk) => Ok(jade_disk),
        // whoops, something has gone very seriously wrong...
        None => {
            let msg = format!(
                "Unable to find an open disk for Archive:{} Copy:{}",
                disk_archive.name, copy_id,
            );
            Err(DatamoveError::Critical(msg))
        }
    }
}

fn get_disk_paths(disk_archiver: &DiskArchiver) -> Vec<String> {
    // put all the configured paths into a set
    let mut disk_path_set: BTreeSet<String> = BTreeSet::new();
    for disk_archive in &disk_archiver.disk_archives {
        for path in &disk_archive.paths {
            disk_path_set.insert(path.to_string());
        }
    }
    // gather up all the paths we're configured to use
    disk_path_set.into_iter().collect()
}

// TODO: remove this function
// async fn get_loaded_disk_ids(disk_archiver: &DiskArchiver) -> Vec<i64> {
//     // create a Vec to hold our results
//     let mut loaded_disk_ids = Vec::new();
//     // check all the disks and gather up their IDs
//     let disks = build_archival_disks_status(disk_archiver).await;
//     for disk in disks.values() {
//         if disk.id != crate::status::sps::NO_ID {
//             loaded_disk_ids.push(disk.id);
//         }
//     }
//     // return the Vec of disk IDs to the caller
//     loaded_disk_ids
// }

/// TODO: this function exists to detect when configuration contradicts the
///       cheating we did with assuming there is only one (IceCube) archive
///       if you get an error here, it means the assumption of the single
///       archive built into the cache purge logic is wrong
fn get_required_copies(disk_archiver: &DiskArchiver) -> Result<u64> {
    let required_copies = 2;
    for disk_archive in &disk_archiver.disk_archives {
        if disk_archive.num_copies != required_copies {
            let msg = "Inconsistent num_copies among disk archives!".to_string();
            return Err(DatamoveError::Critical(msg));
        }
    }
    Ok(required_copies)
}

/// determine if a file pair has already been archived to a copy
async fn is_already_archived_to_copy(
    disk_archiver: &DiskArchiver,
    jade_file_pair: &JadeFilePair,
    disk_archive: &DiskArchive,
    copy_id: u64,
) -> Result<bool> {
    let pool = &disk_archiver.context.db_pool;
    let jade_file_pair =
        find_file_pair_on_disk(pool, disk_archiver, disk_archive, copy_id, jade_file_pair).await?;
    Ok(jade_file_pair.is_some())
}

// Verify that the provided JadeDisk is physically present and in good
// condition to be used for archiving a file pair.
fn is_okay_to_archive_to(disk: &JadeDisk) -> bool {
    // the disk tells us where it should be mounted; so check that place
    let disk_path = &disk.device_path;
    // if the path doesn't exist
    match fs::exists(disk_path) {
        Ok(exists) => {
            if !exists {
                error!("{} doesn't exist; unable to write archive.", disk_path);
                return false;
            }
        }
        Err(e) => {
            error!("{} doesn't exist; unable to write archive.", disk_path);
            error!("Error was: {e}");
            return false;
        }
    }
    // if the path isn't writable
    if !is_writable_dir(disk_path) {
        error!(
            "{} cannot be written to; unable to write archive.",
            disk_path
        );
        return false;
    }
    // if the path isn't a mount point (i.e.: a disk isn't mounted there)
    if !is_mount_point(disk_path) {
        error!(
            "{} is NOT a mount point; unable to write archive.",
            disk_path
        );
        return false;
    }
    // get all the UUID label files that we find on the disk
    let disk_labels = match read_disk_labels(Path::new(disk_path)) {
        Ok(disk_labels) => disk_labels,
        Err(e) => {
            error!(
                "{} cannot be checked for UUID labels. (Error: {e})",
                disk_path
            );
            return false;
        }
    };
    // if there isn't EXACTLY one uuid label file on the disk
    if disk_labels.len() != 1 {
        error!(
            "{} doesn't have a unique UUID label; unable to write archive.",
            disk_path
        );
        return false;
    }
    // if the uuid label file on the disk doesn't match the disk the database expects there
    if disk_labels[0] != disk.uuid {
        error!(
            "{} doesn't match UUID label! DB Expects:{}  Disk Has:{}; unable to write archive.",
            disk_path, disk.uuid, disk_labels[0],
        );
        return false;
    }
    // having survived the gauntlet, the archival disk has been
    // properly vetted for use; we may now write the file pair
    true
}

fn parse_uuid_from_filename(path: &Path) -> Option<String> {
    // create a RegEx to match JADE's filename pattern `ukey_$UUID_`
    let uuid_regex = Regex::new(r"ukey_([a-f0-9-]{36})_").unwrap();
    // get the file name
    let os_str = path.as_os_str();
    let os_string = os_str.to_os_string();
    let file_name = os_string.to_string_lossy();
    // look for the UUID in the file name
    if let Some(caps) = uuid_regex.captures(&file_name) {
        if let Some(uuid_match) = caps.get(1) {
            // yay! we found it, let's return it
            let uuid_str = uuid_match.as_str();
            return Some(uuid_str.to_string());
        }
    }
    // And so the poor dog had
    None
}

fn read_disk_labels(path: &Path) -> Result<Vec<String>> {
    // disk labels follow a UUID pattern
    let uuid_pattern = Regex::new(
        r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$",
    )
    .expect("Invalid regex");
    // read the files from the provided path
    let disk_labels = fs::read_dir(path)?
        // filter out any directory entries that we couldn't read due to error
        .filter_map(|entry| entry.ok())
        // keep the ones that match the UUID filename pattern
        .filter(|entry| {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            uuid_pattern.is_match(&name)
        })
        // map the file names of the ones we keep to a String
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        // collect them all up into a Vec<> for the caller
        .collect();

    // return the list of disk label filenames to the caller
    Ok(disk_labels)
}

// move any files from the work directory to the inbox directory
async fn reclaim_abandoned_work(disk_archiver: &DiskArchiver) -> Result<()> {
    // if we're not configured to reclaim work, just bail out
    if !disk_archiver.context.config.sps_disk_archiver.reclaim_work {
        return Ok(());
    }
    // otherwise, we need to move everything from work_dir -> inbox_dir
    let inbox_dir = &disk_archiver.context.config.sps_disk_archiver.inbox_dir;
    let work_dir = &disk_archiver.context.config.sps_disk_archiver.work_dir;
    info!("Moving abandoned work from {work_dir} to {inbox_dir}.");
    let inbox_path = Path::new(inbox_dir);
    // read all the files in the work directory
    let mut rd = tokio::fs::read_dir(work_dir).await?;
    while let Some(entry) = rd.next_entry().await? {
        // get the next file from the work directory
        let work_file: PathBuf = entry.path();
        // if this isn't a file, just skip it
        if !work_file.is_file() {
            continue;
        }
        // get the name of the file in the work directory
        let file_name = work_file.file_name().unwrap();
        // compute the inbox directory path of the file
        let inbox_file = inbox_path.join(file_name);
        // log about what we're doing
        info!("Moving file {:?} to {:?}", work_file, inbox_file);
        // attempt to move the file, and log if we fail
        if let Err(e) = tokio::fs::rename(&work_file, &inbox_file).await {
            error!("Failed to move {:?} to {:?}: {}", work_file, inbox_file, e);
        }
    }
    // tell the caller that everything went according to plan
    info!("Finished reclaiming abandoned work.");
    Ok(())
}

fn remove_uuids_from_cache(cache_path: &Path, delete_set: &HashSet<String>) -> std::io::Result<()> {
    // create a RegEx to match JADE's filename pattern `ukey_$UUID_`
    let uuid_regex = Regex::new(r"ukey_([a-f0-9-]{36})_").unwrap();
    // for each directory entry
    for entry in fs::read_dir(cache_path)? {
        // get the filename
        let entry = entry?;
        let file_path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        // extract the UUID from the filename
        if let Some(caps) = uuid_regex.captures(&file_name_str) {
            if let Some(uuid_match) = caps.get(1) {
                let uuid_str = uuid_match.as_str();
                // if this uuid is contained in the delete set
                if delete_set.contains(uuid_str) {
                    info!("Removing file: {}", file_path.display());
                    fs::remove_file(&file_path)?;
                }
            }
        }
    }
    // tell the caller that we successfully removed files from the cache
    Ok(())
}

/// Serializes an ArchivalDiskFile to JSON and writes it to a structured directory.
pub fn save_archival_disk_file(
    device_root: &str,
    uuid: &str,
    archival_disk_file: &ArchivalDiskFile,
) -> Result<()> {
    // TODO: probably want to consolidate this logic
    let hex_1 = &uuid[0..1];
    let hex_2 = &uuid[1..2];

    let dir_path = format!("{}/metadata/{}/{}", device_root, hex_1, hex_2);
    create_directory(Path::new(&dir_path))?;

    let file_path = format!("{}/{}.json", dir_path, uuid);
    let json_data = serde_json::to_string_pretty(archival_disk_file)?;

    let mut file = File::create(&file_path)?;
    file.write_all(json_data.as_bytes())?;

    Ok(())
}

fn write_archival_disk_metadata(label_path: &Path, disk: &JadeDisk) -> Result<()> {
    // create a structure for the serialized form of disk metadata
    let metadata: ArchivalDiskMetadata = disk.into();

    // serialize the metadata struct to the label path
    let json = serde_json::to_string_pretty(&metadata)?;
    let mut file = File::create(label_path)?;
    file.write_all(json.as_bytes())?;

    // inform the caller that we succeeded
    Ok(())
}

//---------------------------------------------------------------------------
//---------------------------------------------------------------------------
//---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }

    #[test]
    fn test_close_semaphore_join() {
        let path = Path::new("/mnt/slot1");
        let close_semaphore_path = path.join(CLOSE_SEMAPHORE_NAME);
        let close_path = close_semaphore_path.to_str().unwrap();
        assert_eq!(close_path, "/mnt/slot1/close.me");
    }
}
