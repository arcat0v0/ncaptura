use std::path::{Path, PathBuf};

pub fn interpreter_candidates() -> Vec<PathBuf> {
    [
        "/usr/bin/python3.13",
        "/usr/bin/python3.12",
        "/usr/bin/python3.11",
        "/usr/bin/python3.10",
        "/usr/bin/python3",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

pub fn venv_python(venv_dir: &Path) -> PathBuf {
    venv_dir.join("bin/python")
}
