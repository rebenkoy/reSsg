use std::path::PathBuf;
use anyhow::anyhow;
use minijinja::{Error, State, Value};
use serde::de::Error as _;
use crate::build::renderer::{RendererState, RENDERER_STATE};

pub fn static_ref(state: &State, file: String) -> Result<Value, Error> {
    let state_binding= state.lookup(RENDERER_STATE).ok_or_else(|| {
        Error::custom(format!("`{}` variable not found in env", RENDERER_STATE))
    })?;
    let locked_state = state_binding.downcast_object_ref::<RendererState>()
        .ok_or(anyhow!("No renderer state is present"))
        .and_then(|x| x.get())
        .map_err(|e|{
        Error::custom(e)
    })?;
    let config = &locked_state.config;
    let static_hashes = &locked_state.static_hashes;

    let static_dir = PathBuf::from(&config.static_output);

    let static_file = static_dir.join(file);
    let static_ref = PathBuf::from(&config.prefix).join(&static_file);
    println!("{}", static_ref.display());
    Ok(Value::from_safe_string(
        format!(
            "{}?hash={}",
            static_ref.display(),
            static_hashes.get(&static_file)
                .ok_or(Error::custom(format!("Can not find hash for static file {}", static_file.display())))?
        )
    ))
}