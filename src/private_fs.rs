// ABOUTME: Helpers that create owner-only (0700/0600) directories and files for private run artifacts.
// ABOUTME: An application-level boundary, not a sandbox between processes of the same OS user.
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

/// Creates a new file, failing if it exists, readable and writable only by the owner.
pub fn create_file(path: impl AsRef<Path>) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Creates a new directory, failing if it exists, accessible only by the owner.
pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
