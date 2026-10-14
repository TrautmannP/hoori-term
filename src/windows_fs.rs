//! Windows file replacement and pinned native start directories.
use std::fs::{File, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetFinalPathNameByHandleW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};

pub fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let wide = |path: &Path| -> std::io::Result<Vec<u16>> {
        let mut value: Vec<_> = path.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        value.push(0);
        Ok(value)
    };
    let source = wide(source)?;
    let destination = wide(destination)?;
    // Both buffers are live, NUL-terminated paths. No cross-volume copy or
    // delayed/reboot action is allowed; the caller uses a same-directory file.
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn pin_directory(path: &Path) -> Result<(File, PathBuf), String> {
    let file = OpenOptions::new()
        .read(true)
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|e| format!("Startverzeichnis nicht erreichbar: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_dir() {
        return Err("Startverzeichnis ist kein Verzeichnis".into());
    }
    // The open handle deliberately omits delete sharing. Resolve that handle,
    // not the original pathname, and retain it until CreateProcess completes.
    let mut buffer = vec![0u16; 32_768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            0,
        )
    } as usize;
    if length == 0 {
        return Err(format!(
            "Startverzeichnis kann nicht aufgelöst werden: {}",
            std::io::Error::last_os_error()
        ));
    }
    if length >= buffer.len() {
        return Err("Startverzeichnis ist zu lang".into());
    }
    let resolved = String::from_utf16(&buffer[..length])
        .map_err(|_| "Startverzeichnis ist nicht als Unicode-Pfad darstellbar")?;
    // Conventional DOS/UNC paths also work with the supported PowerShell 5.1.
    let resolved = if let Some(unc) = resolved.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        resolved
            .strip_prefix(r"\\?\")
            .unwrap_or(&resolved)
            .to_owned()
    };
    Ok((file, PathBuf::from(resolved)))
}
