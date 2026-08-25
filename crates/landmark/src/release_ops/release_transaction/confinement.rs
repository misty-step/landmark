use crate::*;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

pub(crate) fn secure_existing_directory(path: &Path, name: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{name} must be a real directory, not a symlink").into());
    }
    Ok(fs::canonicalize(path)?)
}

#[cfg(unix)]
pub(crate) fn openat_nofollow(
    directory: &File,
    name: &OsStr,
    directory_only: bool,
) -> Result<File> {
    let name = CString::new(name.as_bytes())?;
    let mut flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    if directory_only {
        flags |= libc::O_DIRECTORY;
    }
    let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(unix)]
pub(crate) fn open_directory_path_nofollow(path: &Path, name: &str) -> Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{name} must be a real directory, not a symlink").into());
    }
    let canonical = fs::canonicalize(path)?;
    let mut directory = if canonical.is_absolute() {
        File::open("/")?
    } else {
        File::open(".")?
    };
    for component in canonical.components() {
        match component {
            std::path::Component::RootDir | std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => {
                directory = openat_nofollow(&directory, part, true)?;
            }
            _ => return Err(format!("{name} must not contain parent or prefix components").into()),
        }
    }
    if !directory.metadata()?.is_dir() {
        return Err(format!("{name} must be a directory").into());
    }
    Ok(directory)
}

#[cfg(unix)]
pub(crate) fn open_relative_regular_file_nofollow(root: &File, relative: &Path) -> Result<File> {
    validate_relative_path(relative, "artifact path")?;
    let mut directory = root.try_clone()?;
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let std::path::Component::Normal(part) = component else {
            unreachable!();
        };
        if components.peek().is_some() {
            directory = openat_nofollow(&directory, part, true)?;
        } else {
            let file = openat_nofollow(&directory, part, false)?;
            if !file.metadata()?.is_file() {
                return Err("artifact path must resolve to a regular file".into());
            }
            return Ok(file);
        }
    }
    Err("artifact path must name a file".into())
}

#[cfg(unix)]
pub(crate) fn open_regular_path_nofollow(path: &Path, name: &str) -> Result<File> {
    let filename = path
        .file_name()
        .ok_or_else(|| format!("{name} must name a file"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let directory = open_directory_path_nofollow(parent, name)?;
    let file = openat_nofollow(&directory, filename, false)?;
    if !file.metadata()?.is_file() {
        return Err(format!("{name} must be a regular file").into());
    }
    Ok(file)
}

#[cfg(not(unix))]
pub(crate) fn open_directory_path_nofollow(path: &Path, name: &str) -> Result<File> {
    let canonical = secure_existing_directory(path, name)?;
    Ok(File::open(canonical)?)
}

#[cfg(not(unix))]
pub(crate) fn open_relative_regular_file_nofollow(root: &File, relative: &Path) -> Result<File> {
    let _ = (root, relative);
    Err("release artifact binding requires fd-relative path traversal on Unix".into())
}

#[cfg(not(unix))]
pub(crate) fn open_regular_path_nofollow(path: &Path, name: &str) -> Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{name} must be a real regular file, not a symlink").into());
    }
    Ok(File::open(path)?)
}

pub(crate) fn read_bounded_file(file: File, label: &str) -> Result<Vec<u8>> {
    let size = file.metadata()?.len();
    if size > MAX_LOCAL_ARTIFACT_BYTES {
        return Err(format!("local artifact {label} exceeds 16 MiB").into());
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(MAX_LOCAL_ARTIFACT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_LOCAL_ARTIFACT_BYTES {
        return Err(format!("local artifact {label} exceeds 16 MiB").into());
    }
    Ok(bytes)
}
