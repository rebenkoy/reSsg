use std::path::{Path, PathBuf};
use minijinja::{Error, State, Value};
use serde::de::Error as _;
use crate::build::renderer_state::{get_state, lock_state, RendererState, RENDERER_STATE};
use crate::util::error_mappers::map_io_error;

use crate::util::md_parser::Context;

pub fn blocks(state: &State, mut dir: String, default_template: Option<String>) -> Result<Value, Error> {
    if dir.starts_with("./") {
        dir = PathBuf::from(state.name()).parent().unwrap_or(Path::new("../../..")).join(dir).to_str().ok_or(
            Error::custom("Not a valid unicode")
        )?.to_string();
    }

    let renderer_state = get_state(state)?;
    let locked_state = lock_state(&renderer_state)?;
    let target_root = locked_state.target_path.clone();
    drop(locked_state);
    drop(renderer_state);

    let blocks_dir = target_root.join(dir);
    if !blocks_dir.exists() {
        return Err(Error::custom(format!("Blocks directory `{}` not found.", blocks_dir.display())));
    }
    if !blocks_dir.is_dir() {
        return Err(Error::custom(format!("Blocks directory `{}` is not a directory.", blocks_dir.display())));
    }
    let mut files = vec![];
    for entry in blocks_dir.read_dir().map_err(map_io_error)? {
        let entry = entry.map_err(map_io_error)?.path();

        if !entry.is_file() {
            continue;
        }
        match entry.extension() {
            Some(ext) if ext == "md" || ext == "html" => {
                files.push(entry);
            }
            _ => {}
        }
    }
    let mut results = vec![];
    for entry in itertools::sorted(files.into_iter()) {
        if let Some(ext) = entry.extension() && ext == "html"  {
            let entry = entry.strip_prefix(target_root.as_path()).map_err(|_| Error::custom(format!("Failed to strip prefix `{}` for `{}` .", target_root.display(), entry.display())))?;
            results.push(state.env().get_template(entry.to_str().ok_or(
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Not utf-8 path")
            ).map_err(|e| {Error::custom(format!("{}", e))})?)?.render(())?);
            continue;
        }

        let text = std::fs::read_to_string(&entry).map_err(map_io_error)?;
        let context = Context::new(&text, &default_template, state.env())?;

        let template = state.env().get_template(context.template.as_str())?;
        results.push(template.render(&context)?);
    }

    Ok(Value::from_safe_string(results.join("\n")))
}