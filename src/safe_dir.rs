//! Handle-relative directory access for confined local roots (MCP `reports/`).
//!
//! Opens the target with `O_DIRECTORY | O_NOFOLLOW`, holds the directory file
//! descriptor, and creates or opens children only via `openat`. After the
//! directory is open, replacing the `reports` path with a symlink cannot
//! redirect subsequent I/O through that handle.

use anyhow::{Context, Result, bail};
use std::ffi::CString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};

/// A real directory held by file descriptor for path-independent child I/O.
#[derive(Debug)]
pub struct SafeDir {
    dir: File,
    /// Best-effort path recorded at open time (may become stale after renames).
    display_path: PathBuf,
}

impl SafeDir {
    /// Open or create `name` under `parent` as a non-symlink directory and retain its dirfd.
    pub fn open_or_create_under(parent: &Path, name: &str) -> Result<Self> {
        validate_dir_component(name)?;
        let parent_dir = open_directory_path(parent)
            .with_context(|| format!("could not open parent directory {}", parent.display()))?;
        let dir = open_or_mkdir_child(&parent_dir, name).with_context(|| {
            format!(
                "could not open or create confined directory {}/{}",
                parent.display(),
                name
            )
        })?;
        let display_path = parent.join(name);
        let display_path = fs_canonicalize_nofollow_leaf(&display_path).unwrap_or(display_path);
        Ok(Self { dir, display_path })
    }

    pub fn display_path(&self) -> &Path {
        &self.display_path
    }

    pub fn join_display(&self, filename: &str) -> PathBuf {
        self.display_path.join(filename)
    }

    /// Create a new exclusive file under this directory (`O_CREAT|O_EXCL|O_NOFOLLOW`).
    pub fn create_new_file(&self, filename: &str) -> Result<(File, PathBuf)> {
        validate_file_component(filename)?;
        let file = openat_child(
            self.dir.as_raw_fd(),
            filename,
            libc::O_WRONLY
                | libc::O_CREAT
                | libc::O_EXCL
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW,
            0o600,
        )
        .with_context(|| {
            format!(
                "could not create {}; Inquiry never overwrites an existing file under the confined directory",
                self.join_display(filename).display()
            )
        })?;
        Ok((file, self.join_display(filename)))
    }

    pub fn write_new(&self, filename: &str, content: &[u8]) -> Result<PathBuf> {
        let (mut file, path) = self.create_new_file(filename)?;
        file.write_all(content)
            .with_context(|| format!("could not write {}", path.display()))?;
        Ok(path)
    }

    /// Open an existing regular file under this directory (`O_RDONLY|O_NOFOLLOW`).
    pub fn open_existing_file(&self, filename: &str) -> Result<(File, PathBuf)> {
        validate_file_component(filename)?;
        let file = openat_child(
            self.dir.as_raw_fd(),
            filename,
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0,
        )
        .with_context(|| format!("could not open {}", self.join_display(filename).display()))?;
        let metadata = file
            .metadata()
            .with_context(|| format!("could not inspect {}", filename))?;
        if !metadata.is_file() {
            bail!("{filename} is not a regular file");
        }
        Ok((file, self.join_display(filename)))
    }

    pub fn read_file(&self, filename: &str, max_bytes: u64) -> Result<(Vec<u8>, PathBuf)> {
        let (mut file, path) = self.open_existing_file(filename)?;
        let metadata = file
            .metadata()
            .with_context(|| format!("could not inspect {}", path.display()))?;
        if metadata.len() > max_bytes {
            bail!("{} exceeds the {max_bytes}-byte read limit", path.display());
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        Read::by_ref(&mut file)
            .take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .with_context(|| format!("could not read {}", path.display()))?;
        if bytes.len() as u64 > max_bytes {
            bail!("{} exceeds the {max_bytes}-byte read limit", path.display());
        }
        Ok((bytes, path))
    }
}

fn validate_dir_component(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        bail!("directory name must be a single relative component");
    }
    Ok(())
}

fn validate_file_component(name: &str) -> Result<()> {
    let path = Path::new(name);
    let valid_chars = name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || ".-_".contains(character));
    if path.components().count() != 1
        || !valid_chars
        || name.starts_with('.')
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        bail!("filename must be a simple relative name without directories");
    }
    Ok(())
}

fn open_directory_path(path: &Path) -> Result<File> {
    let c_path = path_to_cstring(path)?;
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error())
            .with_context(|| format!("open(O_DIRECTORY) failed for {}", path.display()));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn open_or_mkdir_child(parent: &File, name: &str) -> Result<File> {
    let c_name = CString::new(name).context("directory name contains interior NUL")?;
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    let fd = unsafe { libc::openat(parent.as_raw_fd(), c_name.as_ptr(), flags) };
    if fd >= 0 {
        return Ok(unsafe { File::from_raw_fd(fd) });
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) if code == libc::ENOENT => {}
        Some(code) if code == libc::ELOOP || code == libc::ENOTDIR => {
            bail!("confined directory must be a real directory, not a symlink or special file");
        }
        _ => {
            return Err(err)
                .context("openat(O_DIRECTORY|O_NOFOLLOW) failed for confined directory");
        }
    }

    let mkdir_rc = unsafe { libc::mkdirat(parent.as_raw_fd(), c_name.as_ptr(), 0o700) };
    if mkdir_rc != 0 {
        let mkdir_err = io::Error::last_os_error();
        if mkdir_err.raw_os_error() != Some(libc::EEXIST) {
            return Err(mkdir_err).context("mkdirat failed for confined directory");
        }
    }

    let fd = unsafe { libc::openat(parent.as_raw_fd(), c_name.as_ptr(), flags) };
    if fd < 0 {
        let open_err = io::Error::last_os_error();
        match open_err.raw_os_error() {
            Some(code) if code == libc::ELOOP || code == libc::ENOTDIR => {
                bail!("confined directory must be a real directory, not a symlink or special file");
            }
            _ => {
                return Err(open_err).context("openat after mkdirat failed for confined directory");
            }
        }
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn openat_child(
    dir_fd: libc::c_int,
    name: &str,
    flags: libc::c_int,
    mode: libc::c_uint,
) -> io::Result<File> {
    let c_name = CString::new(name).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "filename contains interior NUL",
        )
    })?;
    let fd = unsafe { libc::openat(dir_fd, c_name.as_ptr(), flags, mode) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn path_to_cstring(path: &Path) -> Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(path.as_os_str().as_bytes())
        .with_context(|| format!("path contains interior NUL: {}", path.display()))
}

/// Canonicalize a path whose final component must not be a symlink.
fn fs_canonicalize_nofollow_leaf(path: &Path) -> Result<PathBuf> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("could not inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("{} must not be a symbolic link", path.display());
    }
    path.canonicalize()
        .with_context(|| format!("could not resolve {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn rejects_symlink_reports_root() {
        let base = tempdir().unwrap();
        let outside = tempdir().unwrap();
        symlink(outside.path(), base.path().join("reports")).unwrap();
        let error = SafeDir::open_or_create_under(base.path(), "reports").unwrap_err();
        assert!(
            error.to_string().contains("real directory")
                || error
                    .chain()
                    .any(|cause| cause.to_string().contains("real directory")),
            "{error:#}"
        );
    }

    #[test]
    fn write_survives_reports_path_swap_after_open() {
        let base = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let root = SafeDir::open_or_create_under(base.path(), "reports").unwrap();
        // Attacker renames the real directory aside and plants a symlink at `reports`.
        // (Unlinking the only name for a directory is not a portable openat race model.)
        let real = base.path().join("reports.real");
        std::fs::rename(base.path().join("reports"), &real).unwrap();
        symlink(outside.path(), base.path().join("reports")).unwrap();

        let written = root.write_new("probe.html", b"<html>safe</html>").unwrap();
        assert!(
            written.ends_with("reports/probe.html") || written.ends_with("reports.real/probe.html"),
            "display path: {}",
            written.display()
        );
        // Bytes must land in the original directory inode, not the symlink target.
        assert!(
            !outside.path().join("probe.html").exists(),
            "write escaped into the substituted symlink target"
        );
        assert!(
            real.join("probe.html").exists(),
            "write should remain in the renamed original directory"
        );
        // Re-open via the still-held dirfd and read back.
        let (bytes, _) = root.read_file("probe.html", 1024).unwrap();
        assert_eq!(bytes, b"<html>safe</html>");
    }

    #[test]
    fn create_after_swap_still_cannot_use_path_reopen_escape() {
        let base = tempdir().unwrap();
        let outside = tempdir().unwrap();
        // Establish a real reports dir, then hold dirfd, then swap via rename+symlink.
        let root = SafeDir::open_or_create_under(base.path(), "reports").unwrap();
        let real = base.path().join("reports.real");
        std::fs::rename(base.path().join("reports"), &real).unwrap();
        symlink(outside.path(), base.path().join("reports")).unwrap();
        // A fresh open must refuse the symlink path.
        assert!(SafeDir::open_or_create_under(base.path(), "reports").is_err());
        // The original handle remains usable and confined.
        root.write_new("still-confined.html", b"ok").unwrap();
        assert!(!outside.path().join("still-confined.html").exists());
        assert!(real.join("still-confined.html").exists());
    }
}
