// utils.rs

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tracing::{error, info, trace};

/// Attempts to claim the next available file from the inbox directory by
/// moving it to the work directory.
///
/// This function returns the path to the claimed file if successful, or
/// `None` if no files are available.
/// If an error occurs during the operation, an `Err` containing the I/O
/// error is returned.
///
/// # Parameters
///
/// - `inbox_dir`: Path to the directory containing incoming files.
/// - `work_dir`: Path to the directory where the file will be moved for processing.
///
/// # Returns
///
/// - `Ok(Some(PathBuf))` if a file is successfully claimed and moved.
/// - `Ok(None)` if no files are available in the inbox.
/// - `Err(io::Error)` if an I/O error occurs during the operation.
pub fn next_file(inbox_dir: &Path, work_dir: &Path) -> io::Result<Option<PathBuf>> {
    // create an iterator over the entries in the inbox_dir
    let entries = fs::read_dir(inbox_dir)?;
    // for each entry in the inbox_dir
    for entry in entries {
        // make sure we got an entry and not an error
        let entry = entry?;
        // get the path to the entry
        let src_path = entry.path();
        // if this entry is a file (and not a directory or a link or something)
        if src_path.is_file() {
            // determine the work_dir path for this file
            let file_name = src_path.file_name().unwrap();
            let dest_path = work_dir.join(file_name);
            // try to move the file from the inbox_dir to the work_dir
            match fs::rename(&src_path, &dest_path) {
                // if we were able to move it to the work_dir
                Ok(_) => {
                    // return the work_dir path of the file
                    info!("Moved {src_path:?} -> {dest_path:?}");
                    return Ok(Some(dest_path));
                }
                // if we couldn't move the file for some reason
                Err(e) => {
                    // if it's not there, another process likely took it
                    if e.kind() == io::ErrorKind::NotFound {
                        // move on to the next one
                        trace!("Unable to move {src_path:?} (Not Found)");
                        continue;
                    }
                    // ut oh, this might be serious...
                    error!("Error moving {src_path:?} -> {dest_path:?}: {e}");
                    return Err(e);
                }
            }
        }
    }
    // NO FILE FOR YOU!
    // https://www.youtube.com/watch?v=zOpfsGrNvnk
    trace!("next_file({inbox_dir:?}): No files present");
    Ok(None)
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
