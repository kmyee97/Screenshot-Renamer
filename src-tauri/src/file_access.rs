use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
};

/// Exclude active/future Windows writers while allowing readers and moves.
/// Undo also uses this primitive to restore empty files from existing history.
pub(crate) fn open_without_writers(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ};
        options.share_mode((FILE_SHARE_READ | FILE_SHARE_DELETE).0);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The screenshot is not a regular file.",
        ));
    }
    Ok(file)
}

/// Forward processing also requires screenshot data. Hold through the move.
pub(crate) fn open_saved_screenshot(path: &Path) -> io::Result<File> {
    let file = open_without_writers(path)?;
    if file.metadata()?.len() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The screenshot is empty or has not finished saving.",
        ));
    }
    Ok(file)
}
