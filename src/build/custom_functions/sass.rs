use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{PathBuf};
use anyhow::anyhow;
use itertools::Itertools;
use minijinja::{Error, State, Value};
use rsfs::GenFS;
use serde::de::Error as _;
use sha1::{Digest, Sha1};
use crate::build::renderer_state::{RendererState, _RendererState, get_state, lock_state, RENDERER_STATE};

pub fn sass(state: &State, source_path: String) -> Result<Value, Error> {
    let renderer_state = get_state(state)?;
    let mut locked_state = lock_state(&renderer_state)?;
    let sass_state = &mut locked_state.requested_sass.files;
    sass_state.insert(PathBuf::from(source_path));

    Ok(Value::from_bytes(vec![]))
}

pub fn include_sass(state: &State) -> Result<Value, Error> {
    let renderer_state = get_state(state)?;
    let mut locked_state = lock_state(&renderer_state)?;
    locked_state.requested_sass.requested = true;
    let index_file = locked_state.out_prefix.join(SassState::OUT_NAME);

    let link_elem = format!("<link rel=\"stylesheet\" href=\"{}?hash={}\">", index_file.to_string_lossy(), locked_state.requested_sass.hash);
    Ok(Value::from_safe_string(link_elem))
}

#[derive(Default, Clone, Debug)]
pub struct SassState{
    files: HashSet<PathBuf>,
    requested: bool,
    hash: String,
}

impl SassState {
    const OUT_NAME: &'static str = "index.css";

    pub fn with_hash(hash: Option<String>) -> Self {
        let mut res: Self = Default::default();
        if let Some(hash) = hash {
            res.hash = hash;
        }
        res
    }
    fn compile_to_string(&self) -> Result<String, anyhow::Error> {
        if self.files.is_empty() {
            Ok(String::new())
        } else {
            let mut opts = Default::default();
            // opts.load_paths("");
            grass::from_string(
                format!("@import {};",
                        self.files.iter().map(|p| format!("'{}'", p.to_string_lossy())).join(", ")
                ).to_owned(),
                &opts
            )
        }.map_err(
            |e| {
                anyhow!(e)
            }
        )
    }

    pub fn build<FS: GenFS>(state: &State, dir: &PathBuf, fs: &mut FS) -> Result<Option<String>, anyhow::Error> {
        let renderer_state = get_state(state)?;
        let locked_state = lock_state(&renderer_state)?;
        let s = &locked_state.requested_sass;
        if !s.requested {
            return Ok(None);
        }
        let mut file_writer = fs.create_file(dir.join("index.css").as_path())?;
        let res = s.compile_to_string()?;
        let bytes = res.as_bytes();

        let mut hasher = Sha1::new();
        hasher.update(&bytes);
        file_writer.write(bytes)?;
        Ok(Some(hex::encode(hasher.finalize().as_slice())))
    }
}