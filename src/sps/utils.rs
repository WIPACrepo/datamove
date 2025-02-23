// utils.rs

pub mod crypto;
pub mod lsblk;

use fs2::{free_space, total_space};
use log::{error, info};
use std::cmp::max;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;
use uuid::Uuid;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

/// TODO: write documentation comment
pub fn count_uuid_labels(path_str: &str) -> Result<u64> {
    // if not a directory, then an error
    let path = Path::new(path_str);
    if !path.is_dir() {
        let msg = format!("{path_str} is not a directory");
        return Err(msg.into());
    }
    // let's count the UUIDs...
    let mut count = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if let Some(filename) = entry.file_name().to_str() {
            // if the entire filename is a valid UUID, it counts
            if Uuid::parse_str(filename).is_ok() {
                count += 1;
            }
        }
    }
    // return the count to the caller
    Ok(count)
}

/// TODO: write documentation comment
pub fn create_directory(dir_path: &Path) -> Result<()> {
    match fs::create_dir_all(dir_path) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{e}").into()),
    }
}

/// TODO: write documentation comment
pub fn flush_to_disk(path: &Path) -> Result<()> {
    // open the file
    let file = File::open(path)?;
    // ensure all the data and metadata are flushed to disk
    file.sync_all()?;
    Ok(())
}

/// Determine the count of files in the provided directory.
///
/// This function scans the provided directory and determines how many files are
/// present in the directory. It ignores any subdirectories and only considers
/// regular files in the top level of the specified directory.
///
/// If no files are found, it returns 0 as the count.
///
/// # Arguments
///
/// * `directory` - A string slice representing the path to the directory to scan.
///
/// # Returns
///
/// Returns a `Result` containing:
/// - `Ok(u64)` - The count of the number of files if successful.
/// - `Err(Box<dyn core::error::Error>)` - An error if the directory does not exist,
///   is not accessible, or an error occurs while reading file metadata.
pub fn get_file_count(directory: &str) -> Result<u64> {
    // convert the provided directory path to a Path
    let dir_path = Path::new(directory);
    // ensure the provided path is a directory
    if !dir_path.is_dir() {
        return Err("Provided path is not a directory".into());
    }
    // initialize a counter for files
    let mut file_count = 0;
    // iterate over the entries in the directory
    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        // increment the counter if this is a regular file
        if metadata.is_file() {
            file_count += 1;
        }
    }
    // return the total count of files to the caller
    Ok(file_count)
}

/// TODO: write documentation comment
pub fn get_free_space(volume: &str) -> Result<u64> {
    match free_space(volume) {
        Ok(free) => Ok(free),
        Err(e) => {
            error!("Unable to determine free_space for '{volume}' due to: {e}");
            Err(e.into())
        }
    }
}

/// Determine the age of the oldest file in the provided directory, in seconds.
///
/// This function scans the provided directory and determines the file with the oldest
/// modification time. It ignores any subdirectories and only considers regular files
/// in the top level of the specified directory.
///
/// If no files are found, it returns 0 as the age.
///
/// # Arguments
///
/// * `directory` - A string slice representing the path to the directory to scan.
///
/// # Returns
///
/// Returns a `Result` containing:
/// - `Ok(u64)` - The age of the oldest file in seconds if successful.
/// - `Err(Box<dyn core::error::Error>)` - An error if the directory does not exist,
///   is not accessible, or an error occurs while reading file metadata.
pub fn get_oldest_file_age_in_secs(directory: &str) -> Result<u64> {
    // convert the provided directory path to a Path
    let dir_path = Path::new(directory);
    // ensure the provided path is a directory
    if !dir_path.is_dir() {
        return Err("Provided path is not a directory".into());
    }
    // get the current system time
    let now = SystemTime::now();
    // iterate over the entries in the directory
    let mut oldest_age = 0;
    // for each entry in the provided directory
    for entry in fs::read_dir(dir_path)? {
        // get the data about the entry
        let entry = entry?;
        let metadata = entry.metadata()?;
        // if this is a file and not a directory
        if metadata.is_file() {
            // if we can get the modification time of the file
            if let Ok(mtime) = metadata.modified() {
                // if we can calculate the duration between the modification time and now
                if let Ok(mtime_duration) = now.duration_since(mtime) {
                    // calculate the age of the file in seconds
                    let age = mtime_duration.as_secs();
                    // update if this file is the oldest one that we've seen so far
                    oldest_age = max(oldest_age, age);
                }
            }
        }
    }
    // return the age of the oldest file to the caller
    Ok(oldest_age)
}

/// TODO: write documentation comment
pub fn get_total_space(volume: &str) -> Result<u64> {
    match total_space(volume) {
        Ok(free) => Ok(free),
        Err(e) => {
            error!("Unable to determine total_space for '{volume}' due to: {e}");
            Err(e.into())
        }
    }
}

/// Determine if the provided path is a mount point or not.
///
/// This function runs the system command `mountpoint`, which reads the
/// file `/proc/self/mountinfo` to determine if the mount is listed there.
/// If it finds the path listed there, it will end with status code 0.
/// A non-zero status code indicates an error (1) or not a mountpoint (32).
///
/// # Arguments
///
/// * `path` - A string slice representing the path to the directory to check.
///
/// # Returns
///
/// Returns `true` if the provided path is a mount point, otherwise `false`.
pub fn is_mount_point(path: &str) -> bool {
    // if the path doesn't exist, it can't be a mount point
    if !Path::new(path).exists() {
        return false;
    }
    // run the mountpoint command to see if it's a mountpoint or not
    let output = Command::new("mountpoint").arg(path).output();
    // return the result to the caller
    match output {
        // if the exit code was 0, return true, otherwise false
        Ok(output) => output.status.success(),
        // if there was some kind of error, just call it false
        Err(_) => false,
    }
}

/// TODO: write documentation comment
pub fn is_writable_dir(path_str: &str) -> bool {
    // not a directory? then not a writable directory
    let path = Path::new(path_str);
    if !path.is_dir() {
        return false;
    }
    // create a canary file and write something to it
    let test_file = path.join(format!(".writability_test_{}", Uuid::new_v4()));
    let result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&test_file)
        .and_then(|mut file| file.write_all(b"test"));
    // were we able to write something to it after all?
    let writable = result.is_ok();
    // clean up the canary file (if it was created)
    let _ = fs::remove_file(test_file);
    // tell the caller what we discovered
    writable
}

/// TODO: write documentation comment
pub fn move_file(file_path: &Path, dest_path: &Path) {
    // construct the new file path in the destination directory
    let Some(file_name) = file_path.file_name() else {
        error!("Failed to get file name for {:?}", file_path);
        return;
    };
    let target_path = dest_path.join(file_name);
    // log about what we're doing
    info!("Moving file {:?} to {:?}", file_path, target_path);
    // attempt to move the file, and log if we fail
    if let Err(e) = fs::rename(file_path, &target_path) {
        error!("Failed to move {:?} to {:?}: {}", file_path, target_path, e);
    }
}

/// TODO: write documentation comment
pub fn touch_label(label_path: &Path) -> Result<()> {
    // create a new zero byte file to act as a disk label
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(label_path)?;
    // tell the caller that we succeeded at creating the disk label
    Ok(())
}

//---------------------------------------------------------------------------
//---------------------------------------------------------------------------
//---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::FileTime;
    use std::fs::{self, File};
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }

    #[test]
    fn test_get_file_count() -> Result<()> {
        // create a temporary directory for testing
        let temp_dir = "/tmp/test_get_file_count";
        std::fs::create_dir_all(temp_dir)?;
        // create some files in the directory
        std::fs::File::create(format!("{}/file1.txt", temp_dir))?;
        std::fs::File::create(format!("{}/file2.txt", temp_dir))?;
        std::fs::File::create(format!("{}/file3.txt", temp_dir))?;
        // create a subdirectory (should not be counted)
        std::fs::create_dir(format!("{}/subdir", temp_dir))?;
        // test our file counting function
        let count = get_file_count(temp_dir)?;
        // clean up
        std::fs::remove_dir_all(temp_dir)?;
        // check that we got the correct result from our test
        assert_eq!(count, 3);
        Ok(())
    }

    #[test]
    fn test_get_oldest_file_age_in_secs() -> Result<()> {
        // create a temporary directory
        let temp_dir = "/tmp/test_get_oldest_file_age";
        fs::create_dir_all(temp_dir)?;
        // create temporary files with specific creation times
        let now = SystemTime::now();
        // helper function to create files with specific ages
        let create_file_with_age = |file_name: &str, age_in_secs: u64| -> Result<()> {
            let file_path = Path::new(temp_dir).join(file_name);
            File::create(&file_path)?;
            let creation_time = now - Duration::from_secs(age_in_secs);
            let ft = FileTime::from_system_time(creation_time);
            filetime::set_file_times(&file_path, ft, ft)?;
            Ok(())
        };
        // create files with varying ages
        create_file_with_age("file1.txt", 30)?;
        create_file_with_age("file2.txt", 60)?;
        create_file_with_age("file3.txt", 90)?;
        // test our oldest file age function
        let oldest_age = get_oldest_file_age_in_secs(temp_dir)?;
        // clean up
        fs::remove_dir_all(temp_dir)?;
        // check that we got the correct result from our test
        assert_eq!(oldest_age, 90);
        Ok(())
    }
}
