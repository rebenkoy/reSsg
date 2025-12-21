use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use itertools::Itertools;
use minijinja::value::Object;
use crate::config::BuildConfig;

pub static RENDERER_STATE: &str = "RENDERER_STATE";

#[derive(Debug)]
pub struct RendererStateParams {
    pub config: BuildConfig,
    pub target_path: PathBuf,
    pub static_hashes: HashMap<PathBuf, String>,
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
    pub fn get(&self) -> MutexGuard<'_, _RendererState> {
        self.s.lock().unwrap()
    }

}
impl Object for RendererState {}

#[derive(Debug)]
pub struct _RendererState {
    pub config: BuildConfig,
    pub target_path: PathBuf,
    pub static_hashes: HashMap<PathBuf, String>,
}
impl _RendererState {
    pub fn new(p: RendererStateParams) -> Self {
        let RendererStateParams { config, target_path, static_hashes } = p;
        Self {
            config,
            target_path,
            static_hashes,
        }
    }
}