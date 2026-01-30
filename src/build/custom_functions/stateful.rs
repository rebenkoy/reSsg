use conf::{Deserialize, Serialize};
use minijinja::State;
use rsfs::GenFS;

pub trait StatefulFunction: Sized {
    fn build<FS: GenFS>(state: &State, fs: &mut FS) -> Result<Self, anyhow::Error>;
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum RenderState {
    FirstPass,
    LastPass,
}