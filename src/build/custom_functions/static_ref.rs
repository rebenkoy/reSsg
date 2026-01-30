use std::path::PathBuf;
use std::sync::MutexGuard;
use anyhow::anyhow;
use minijinja::{Error, State, Value};
use serde::de::Error as _;
use url::Url;
use crate::build::renderer_state::{RendererState, _RendererState, get_state, lock_state, RENDERER_STATE};

pub fn static_ref(state: &State, file: String) -> Result<Value, Error> {
    if Url::parse(&file).is_ok() {
        return Ok(Value::from_safe_string(file))
    }
    let renderer_state = get_state(state)?;
    let locked_state = lock_state(&renderer_state)?;
    let config = &locked_state.config;
    let static_hashes = &locked_state.static_hashes;

    let static_dir = PathBuf::from(&config.static_output);

    let static_file = static_dir.join(file);
    let static_ref = PathBuf::from(&config.prefix).join(&static_file);
    Ok(Value::from_safe_string(match static_hashes.get(&static_file) {
        None => {
            log::warn!("Can not find hash for static file {}", static_file.display());
            format!("{}", static_ref.display())
        }
        Some(hash) => {
            format!(
                "{}?hash={}",
                static_ref.display(),
                hash,
            )
        }
    }))
}

