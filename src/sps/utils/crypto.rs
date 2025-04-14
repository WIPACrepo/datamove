// crypto.rs

use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha512};

use crate::error::Result;

/// TODO: write documentation comment
pub fn compute_sha512(file_path: &Path) -> Result<String> {
    let mut file = File::open(file_path)?;
    let mut hasher = Sha512::new();
    let mut buffer = [0; 8192];

    while let Ok(n) = file.read(&mut buffer) {
        if n == 0 {
            break;
        } // EOF
        hasher.update(&buffer[..n]);
    }

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}
