// disk_archiver.rs

use log::{error, info, warn};
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

use crate::config::DiskArchives;
use crate::sps::database::select_disk_by_uuid;
use crate::sps::utils::{get_file_count, get_oldest_file_age_in_secs, is_mount_point};
use crate::status::sps::{Disk, DiskArchiverStatus, DiskStatus};
use crate::{
    sps::{context::Context, database::ensure_host, models::JadeHost},
    status::sps::DiskArchiverWorkerStatus,
};

pub type SharedFlag = Arc<Mutex<bool>>;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone)]
pub struct DiskArchiver {
    pub context: Context,
    pub disk_archives: DiskArchives,
    pub host: JadeHost,
    pub shutdown: SharedFlag,
}

impl DiskArchiver {
    pub fn new(context: Context) -> Self {
        let host =
            ensure_host(&context).expect("Unable to determine JadeHost running DiskArchiver");

        let disk_archives_json_path = &context.config.sps_disk_archiver.disk_archives_json_path;
        let disk_archives = load_disk_archives(disk_archives_json_path)
            .expect("Unable to load disk_archives from JSON configuration");

        Self {
            context,
            disk_archives,
            host,
            shutdown: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn get_status(&self) -> DiskArchiverStatus {
        build_disk_archiver_status(self).await
    }

    pub async fn run(&self) -> Result<()> {
        let mut graceful_shutdown = false;

        // until a shutdown is requested
        while !graceful_shutdown {
            // perform the work of the disk archiver
            do_work_cycle(self).await?;
            // sleep until the next work cycle
            let work_cycle_sleep_seconds = self
                .context
                .config
                .sps_disk_archiver
                .work_cycle_sleep_seconds;
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

        // we received a shutdown command
        info!("Initiating graceful shutdown of DiskArchiver.");
        Ok(())
    }

    pub fn request_shutdown(&self) {
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

fn build_archival_disk_status(disk_archiver: &DiskArchiver, disk_path: &str) -> Disk {
    // ostensibly, this is a path to an archival disk. now let's put it
    // through the gauntlet and see what we've really got here...
    let path = Path::new(disk_path);
    // if the path doesn't exist at all, we can't use it
    if !path.exists() {
        error!("Archival disk path '{}' does not exist.", path.display());
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
            error!("Archival disk path '{}' is not writable.", path.display());
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
            error!(
                "Unable to read disk labels from archival disk path '{}'",
                path.display()
            );
            return Disk::for_status(DiskStatus::NotUsable);
        }
    };
    // if there are too many labels, this disk is not usable
    if disk_labels.len() > 1 {
        error!(
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
    let mut conn = match disk_archiver.context.db.lock() {
        Ok(conn) => conn,
        Err(x) => {
            error!("Unable to lock MysqlConnection: {x}");
            return Disk::for_status(DiskStatus::NotUsable);
        }
    };
    let find_uuid = &disk_labels[0];
    let disk = match select_disk_by_uuid(&mut conn, find_uuid) {
        Ok(disk) => {
            if let Some(disk) = disk {
                disk
            } else {
                error!(
                    "Database table jade_disk has no entry for uuid '{}'.",
                    find_uuid
                );
                return Disk::for_status(DiskStatus::NotUsable);
            }
        }
        Err(_) => {
            error!(
                "Unable to read database table jade_disk for uuid '{}'.",
                find_uuid
            );
            return Disk::for_status(DiskStatus::NotUsable);
        }
    };
    // return the fully realized disk status to the caller
    match Disk::try_from(disk) {
        Ok(disk_status) => disk_status,
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

fn build_archival_disks_status(disk_archiver: &DiskArchiver) -> HashMap<String, Disk> {
    // create a hashmap to hold our archival disks
    let mut archival_disks = HashMap::new();

    // gather up all the paths we're configured to use
    let mut disk_path_set: BTreeSet<String> = BTreeSet::new();
    for disk_archive in &disk_archiver.disk_archives.disk_archives {
        for path in &disk_archive.paths {
            disk_path_set.insert(path.to_string());
        }
    }
    let disk_paths: Vec<String> = disk_path_set.into_iter().collect();

    // for each disk path
    for disk_path in disk_paths {
        let disk = build_archival_disk_status(disk_archiver, &disk_path);
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
            error!(
                "Unable to determine age of oldest file in the inbox directory: {}",
                e
            );
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

    let archival_disks = build_archival_disks_status(disk_archiver);

    DiskArchiverStatus {
        workers: vec![DiskArchiverWorkerStatus {
            archival_disks,
            inbox_count,
        }],
        cache_age,
        inbox_age,
        problem_file_count,
        message: None,
        status: Some("OK".to_string()),
    }
}

pub async fn do_work_cycle(_disk_archiver: &DiskArchiver) -> Result<()> {
    // start the work cycle
    info!("Starting DiskArchiver work cycle.");

    warn!("Doing some important DiskArchiver work here.");

    // finish the work cycle successfully
    info!("End of DiskArchiver work cycle.");
    Ok(())
}

fn load_disk_archives(path: &str) -> Result<DiskArchives> {
    // open the disk archives JSON configuration file
    let file = File::open(path).map_err(|e| format!("Failed to open file {}: {}", path, e))?;
    // deserialize the JSON into the DiskArchives structure
    let disk_archives: DiskArchives =
        serde_json::from_reader(&file).map_err(|e| format!("Failed to deserialize JSON: {}", e))?;
    // return the DiskArchives structure to the caller
    Ok(disk_archives)
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

//---------------------------------------------------------------------------
//---------------------------------------------------------------------------
//---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }
}
