// utils.rs

use std::cmp::max;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

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

/// Determine if the provided path is a mount point or not.
///
/// This function runs the system command `mountpoint`, which reads the
/// file `/proc/self/mountinfo` to determine if the mount is listed there.
/// If it finds the path listed there, it will end with status code 0.
/// A non-zero status code indicates an error (1) or not a mountpoint (32).
///
/// If no files are found, it returns 0 as the age.
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
