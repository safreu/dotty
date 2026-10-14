use std::path::{Path, PathBuf};

use path_clean::PathClean;

pub mod completions;
pub mod forget;
pub mod init;
pub mod manage;

fn resolve_target_path(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.clean()
    } else {
        cwd.join(path).clean()
    }
}
