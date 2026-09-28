use anyhow::Result;
use std::path::Path;

pub(crate) fn replace_file(source: &Path, target: &Path) -> Result<()> {
    // macOS rename replaces an existing file in one operation. Removing the
    // target first loses the old cache on failure and exposes a missing-file gap.
    std::fs::rename(source, target).map_err(Into::into)
}
