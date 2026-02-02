use std::path::{Path, PathBuf};
use conf::de::Error as _;
use minijinja::{Error, State};
use crate::build::renderer_state::{get_state, lock_state};

pub fn get_canonical_template_name(state: &State) -> Result<PathBuf, Error> {
    let renderer_state = get_state(state)?;
    let locked_state = lock_state(&renderer_state)?;
    let target = &locked_state.target;
    Ok(target.canonical_path(state.name()))
}
pub fn get_current_template_dir(state: &State) -> Result<PathBuf, Error> {
    Ok(get_canonical_template_name(state)?.parent()
        .ok_or(Error::custom("Could not find parent directory for current template"))?
        .to_path_buf())
}

pub fn get_canonical_reason_name(state: &State) -> Result<PathBuf, Error> {
    let renderer_state = get_state(state)?;
    renderer_state.current_reason()
}
pub fn get_current_reason_dir(state: &State) -> Result<PathBuf, Error> {
    Ok(get_canonical_reason_name(state)?.parent()
        .ok_or(Error::custom("Could not find parent directory for current template"))?
        .to_path_buf())
}

pub fn get_canonical_path_for<S: AsRef<str>>(state: &State, path: S) -> Result<PathBuf, Error> {
    let path = path.as_ref();
    if path.starts_with("&/") {
        Ok(get_current_reason_dir(state)?.join(path.strip_prefix("&/").unwrap()))
    } else {
        let renderer_state = get_state(state)?;
        let locked_state = lock_state(&renderer_state)?;
        let target = &locked_state.target;
        Ok(target.canonical_path(path))
    }
}

pub fn push_reason(state: &State, reason: PathBuf) -> Result<(), Error> {
    let renderer_state = get_state(state)?;
    renderer_state.push_reason(reason)
}
pub fn pop_reason(state: &State) -> Result<(), Error> {
    let renderer_state = get_state(state)?;
    renderer_state.pop_reason()
}