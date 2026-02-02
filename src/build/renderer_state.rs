use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use anyhow::anyhow;
use itertools::Itertools;
use minijinja::{Error, State};
use minijinja::value::Object;
use serde::de::Error as _;
use crate::build::custom_functions::{RemValueState, SassState};
use crate::build::target_renderer::BuildTarget;
use crate::config::BuildConfig;

pub static RENDERER_STATE: &str = "RENDERER_STATE";


pub fn get_state(state: &State) -> Result<Arc<RendererState>, Error> {
    let state_binding = state.lookup(RENDERER_STATE).ok_or_else(|| {
        Error::custom(format!("`{}` variable not found in env", RENDERER_STATE))
    })?;
    let locked_state = state_binding.downcast_object::<RendererState>()
        .ok_or(anyhow!("No renderer state is present"))
        .map_err(|e| {
            Error::custom(e)
        })?;
    Ok(locked_state)
}

pub fn lock_state(arc: &Arc<RendererState>) -> Result<MutexGuard<_RendererState>, Error> {
    Ok(arc.get().map_err(|e| Error::custom(e))?)
}

#[derive(Debug)]
pub struct RendererStateParams {
    pub config: BuildConfig,
    pub target: BuildTarget,
    pub target_path: PathBuf,
    pub out_dir: PathBuf,
    pub out_prefix: PathBuf,
    pub sass_hash: Option<String>,
    pub static_hashes: HashMap<PathBuf, String>,
    pub remvalue: RemValueState,
}

#[derive(Debug)]
pub struct RendererState {
    s: Mutex<_RendererState>
}
impl RendererState {
    pub fn new(p: RendererStateParams) -> Self {
        Self {
            s: Mutex::new(_RendererState::new(p))
        }
    }
    pub fn get(&self) -> anyhow::Result<MutexGuard<'_, _RendererState>> {
        self.s.lock().map_err(|e| anyhow!("Could not lock renderer state"))
    }
    pub fn push_reason(self: &Arc<Self>, path: PathBuf) -> Result<(), Error> {
        Ok(lock_state(self)?.push_reason(path))
    }
    pub fn pop_reason(self: &Arc<Self>) -> Result<(), Error> {
        Ok(lock_state(self)?.pop_reason())
    }

    pub fn current_reason(self: &Arc<Self>) -> Result<PathBuf, Error> {
        lock_state(self)?.current_reason().map_err(|e| Error::custom(e))
    }
}
impl Object for RendererState {}

#[derive(Debug)]
pub struct _RendererState {
    pub config: BuildConfig,
    pub target: BuildTarget,
    pub reason: Vec<PathBuf>,
    pub target_path: PathBuf,
    pub out_dir: PathBuf,
    pub out_prefix: PathBuf,
    pub static_hashes: HashMap<PathBuf, String>,
    pub requested_sass: SassState,
    pub remvalue: RemValueState,
}
impl _RendererState {
    pub fn new(p: RendererStateParams) -> Self {
        let RendererStateParams {
            config,
            target,
            target_path,
            static_hashes,
            out_dir,
            out_prefix,
            sass_hash,
            remvalue,
        } = p;
        Self {
            config,
            target,
            reason: vec![],
            target_path,
            static_hashes,
            out_dir,
            out_prefix,
            requested_sass: SassState::with_hash(sass_hash),
            remvalue,
        }
    }

    pub fn push_reason(&mut self, path: PathBuf) {
        self.reason.push(path);
    }
    pub fn pop_reason(&mut self) {
        self.reason.pop();
    }

    pub fn current_reason(&self) -> Result<PathBuf, anyhow::Error> {
        self.reason.last().cloned().ok_or(anyhow!("There is no reason to render with this state :P"))
    }
}