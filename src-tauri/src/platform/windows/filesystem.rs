use anyhow::Result;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

pub(crate) fn replace_file(source: &Path, target: &Path) -> Result<()> {
    let from = to_wide_path(source.as_os_str());
    let to = to_wide_path(target.as_os_str());
    // Preserve the existing Windows replacement and write-through behavior.
    // Do not enable COPY_ALLOWED: replacement must stay on one filesystem.
    unsafe {
        MoveFileExW(
            PCWSTR(from.as_ptr()),
            PCWSTR(to.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(Into::into)
}

fn to_wide_path(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}
