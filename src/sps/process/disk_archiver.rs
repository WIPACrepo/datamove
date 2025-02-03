// disk_archiver.rs

use log::{error, info, warn};
use regex::Regex;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tera::Tera;
use tokio::time::{sleep, Duration};

use crate::config::{load_contacts, load_disk_archives, Contacts, DiskArchives};
use crate::metadata::ArchivalDiskMetadata;
use crate::sps::email::{comma_separated_filter, compile_templates, send_email_disk_full};
use crate::sps::jade_db::service::disk::close as close_disk;
use crate::sps::jade_db::service::disk::find_by_uuid as find_disk_by_uuid;
use crate::sps::jade_db::service::disk::get_removable_files;
use crate::sps::jade_db::service::disk::JadeDisk;
use crate::sps::jade_db::service::host::ensure_host;
use crate::sps::jade_db::service::host::JadeHost;
use crate::sps::utils::{get_file_count, get_oldest_file_age_in_secs, is_mount_point};
use crate::status::sps::{Disk, DiskArchiverStatus, DiskStatus};
use crate::{sps::context::Context, status::sps::DiskArchiverWorkerStatus};

pub const CLOSE_SEMAPHORE_NAME: &str = "close.me";

pub type SharedFlag = Arc<Mutex<bool>>;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone)]
pub struct DiskArchiver {
    pub contacts: Contacts,
    pub context: Context,
    pub disk_archives: DiskArchives,
    pub host: JadeHost,
    pub shutdown: SharedFlag,
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

        let disk_archives_json_path = &context.config.sps_disk_archiver.disk_archives_json_path;
        let disk_archives = load_disk_archives(disk_archives_json_path)
            .expect("Unable to load disk_archives from JSON configuration");

        let tera_template_glob = &context.config.sps_disk_archiver.tera_template_glob;
        let mut tera =
            compile_templates(tera_template_glob).expect("Unable to compile Tera templates");
        tera.register_filter("comma", comma_separated_filter);

        Self {
            contacts,
            context,
            disk_archives,
            host,
            shutdown: Arc::new(Mutex::new(false)),
            tera,
        }
    }

    pub async fn get_status(&self) -> DiskArchiverStatus {
        build_disk_archiver_status(self).await
    }

    pub async fn run(&self) {
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

async fn build_archival_disk_status(disk_archiver: &DiskArchiver, disk_path: &str) -> Disk {
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

pub async fn clean_disk_cache(disk_archiver: &DiskArchiver) -> Result<()> {
    // goal: clean the disk cache of files we no longer need to retain
    let cache_dir = &disk_archiver.context.config.sps_disk_archiver.cache_dir;
    info!("Cleaning disk cache: {}", cache_dir);

    // get all the UUIDs of the files currently on disk
    let cache_path = PathBuf::from(cache_dir);
    let disk_set = extract_uuids_from_cache(&cache_path)?;
    info!("Cache: Found {} files to check.", disk_set.len());

    // ask the database which files are copied to N archival disks, limited to files
    // contained on the finished disks that are currently loaded on the JADE machine
    let pool = &disk_archiver.context.db_pool;
    let disk_ids = get_loaded_disk_ids(disk_archiver).await;
    let required_copies = get_required_copies(disk_archiver)?;
    let database_set = get_removable_files(pool, &disk_ids, required_copies).await?;
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
        error!("Attempted to read_disk_labels for {disk_path}, but no labels were found!");
        return Err(format!(
            "Attempted to read_disk_labels for {disk_path}, but no labels were found!"
        )
        .into());
    }
    let find_uuid = &labels[0];
    // look up the disk in the database
    let pool = &disk_archiver.context.db_pool;
    let disk = find_disk_by_uuid(pool, find_uuid).await?;
    // write disk metadata to the UUID label
    let label_path = path.join(find_uuid);
    if let Err(e) = write_archival_disk_metadata(&label_path, &disk) {
        error!("Unable to write disk metadata to disk {disk_path} label file {find_uuid}: {e}.");
        return Err(format!(
            "Unable to write disk metadata to disk {disk_path} label file {find_uuid}: {e}."
        )
        .into());
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

pub async fn do_work_cycle(disk_archiver: &DiskArchiver) -> Result<()> {
    // start the work cycle
    info!("Starting DiskArchiver work cycle.");

    // check if the operator has requested manual close for any archival disks
    if let Err(e) = close_on_semaphore(disk_archiver).await {
        error!("Error occured closing archival disks with close semaphores: {e}");
        error!("No further work will be performed until the error is handled.");
        return Err(e);
    }

    // TODO: the core work cycle of the DiskArchiver
    warn!("Doing some important DiskArchiver work here.");

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

fn get_disk_paths(disk_archiver: &DiskArchiver) -> Vec<String> {
    // put all the configured paths into a set
    let mut disk_path_set: BTreeSet<String> = BTreeSet::new();
    for disk_archive in &disk_archiver.disk_archives.disk_archives {
        for path in &disk_archive.paths {
            disk_path_set.insert(path.to_string());
        }
    }
    // gather up all the paths we're configured to use
    disk_path_set.into_iter().collect()
}

async fn get_loaded_disk_ids(disk_archiver: &DiskArchiver) -> Vec<i64> {
    // create a Vec to hold our results
    let mut loaded_disk_ids = Vec::new();
    // check all the disks and gather up their IDs
    let disks = build_archival_disks_status(disk_archiver).await;
    for disk in disks.values() {
        if disk.id != crate::status::sps::NO_ID {
            loaded_disk_ids.push(disk.id);
        }
    }
    // return the Vec of disk IDs to the caller
    loaded_disk_ids
}

fn get_required_copies(disk_archiver: &DiskArchiver) -> Result<u64> {
    // get the collection of disk archives
    let archives = &disk_archiver.disk_archives.disk_archives;
    // handle the case where there are no disk archives
    let first_value = archives
        .first()
        .ok_or("No disk archives provided!")?
        .num_copies;
    // check that all disk archives have the same num_copies value
    if archives
        .iter()
        .all(|archive| archive.num_copies == first_value)
    {
        Ok(first_value)
    } else {
        Err("Inconsistent num_copies among disk archives!".into())
    }
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
