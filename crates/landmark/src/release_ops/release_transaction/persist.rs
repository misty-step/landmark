use crate::*;
use fs2::FileExt as Fs2FileExt;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
pub(crate) fn secure_state_path(path: &Path, create_parent: bool) -> Result<PathBuf> {
    let filename = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or("transaction path must name a file")?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if create_parent {
        fs::create_dir_all(parent)?;
    }
    let parent = secure_existing_directory(parent, "transaction parent")?;
    let target = parent.join(filename);
    if let Ok(metadata) = fs::symlink_metadata(&target)
        && metadata.file_type().is_symlink()
    {
        return Err("transaction path must not be a symlink".into());
    }
    Ok(target)
}

pub(crate) fn lock_transaction(path: &Path) -> Result<fs::File> {
    let filename = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("invalid transaction filename")?;
    let lock_path = path.with_file_name(format!(".{filename}.lock"));
    if let Ok(metadata) = fs::symlink_metadata(&lock_path)
        && metadata.file_type().is_symlink()
    {
        return Err("transaction lock path must not be a symlink".into());
    }
    let lock = secure_write_options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    lock.lock_exclusive()?;
    Ok(lock)
}

pub(crate) fn read_transaction(path: &Path) -> Result<ReleaseTransaction> {
    let transaction: ReleaseTransaction = serde_json::from_slice(&fs::read(path)?)?;
    validate_transaction(&transaction)?;
    Ok(transaction)
}

pub(crate) fn write_transaction_cas(
    path: &Path,
    expected: Option<&[u8]>,
    transaction: &ReleaseTransaction,
    crash: InjectedCrash,
) -> Result<()> {
    match expected {
        Some(expected) if fs::read(path)? != expected => {
            return Err("canonical transaction changed during compare-and-swap".into());
        }
        None if path.exists() => {
            return Err("canonical transaction appeared during compare-and-swap".into());
        }
        _ => {}
    }
    let bytes = serde_json::to_vec_pretty(transaction)?;
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random)?;
    let filename = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("invalid transaction filename")?;
    let temporary = path.with_file_name(format!(".{filename}.{}.tmp", hex::encode(random)));
    let result = (|| -> Result<()> {
        let mut file = secure_write_options()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        if crash == InjectedCrash::BeforeRename {
            return Err("injected crash before rename".into());
        }
        fs::rename(&temporary, path)?;
        if crash == InjectedCrash::AfterRename {
            return Err("injected crash after rename".into());
        }
        fs::File::open(path.parent().ok_or("transaction path has no parent")?)?.sync_all()?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(crate) fn secure_write_options() -> fs::OpenOptions {
    let mut options = fs::OpenOptions::new();
    #[cfg(unix)]
    {
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    options
}

pub(crate) fn identity_digest<T: Serialize>(value: &T) -> Result<String> {
    Ok(format!(
        "sha256:{}",
        sha256_hex(&serde_json::to_vec(value)?)
    ))
}

pub(crate) fn emit_transaction(transaction: &ReleaseTransaction) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(transaction)?);
    Ok(())
}
