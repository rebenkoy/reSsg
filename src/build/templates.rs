use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::linux::raw::stat;
use std::path::PathBuf;
use anyhow::anyhow;
use crate::config::BuildConfig;
use minijinja::{Environment, Value};
use rsfs::GenFS;
use serde::{Deserialize, Serialize};
use crate::build::custom_functions::blocks;
use crate::build::custom_functions::static_ref;
use crate::build::renderer::{RendererState, RendererStateParams, RENDERER_STATE};

fn locate_targets(config: &BuildConfig) -> Result<HashMap<PathBuf, BuildTarget>, Box<dyn std::error::Error + Send + Sync>> {
    fn _locate_targets(config: &BuildConfig, path: &PathBuf, map: &mut HashMap<PathBuf, BuildTarget>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                _locate_targets(config, &entry.unwrap().path(), map)?;
            }
        } else if path.is_file() {
            if path.file_name().unwrap().to_str().unwrap() == config.index_toml_name {
                map.insert(path.clone(), BuildTarget::new(path.clone())?);
            }
        }
        Ok(())
    }

    let mut pages = HashMap::new();
    _locate_targets(config, &PathBuf::from(&config.source), &mut pages)?;
    Ok(pages)
}

struct BuildTarget {
    pub path: PathBuf,
    pub config: TargetConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct TargetConfig {
    path: String,
    template: String,
}

impl BuildTarget {
    pub fn new(path: PathBuf) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            config: toml::from_slice(&fs::read(&path)?)?,
            path,
        })
    }
    pub fn dir(&self) -> PathBuf {
        self.path.parent().unwrap().to_path_buf()
    }
}

fn validate_targets(targets: &HashMap<PathBuf, BuildTarget>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut dests: HashMap<String, &BuildTarget> = HashMap::new();
    for (_, target) in targets.iter() {
        if let Some(conflict) = dests.get(&target.config.path) {
            return Err(anyhow!(
                "Conflicting destination `{}`. first reserved in: `{}`, attempted to reserve in: `{}`",
                target.config.path, conflict.path.to_str().unwrap(), target.path.to_str().unwrap()
            ).into_boxed_dyn_error())
        }
        dests.insert(target.config.path.clone(), target);
    }

    Ok(())
}

pub fn build_templates<FS: GenFS>(config: &BuildConfig, static_hashes: &HashMap<PathBuf, String>, fs: &mut FS) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let targets = locate_targets(config)?;
    validate_targets(&targets)?;

    for (_, target) in targets.iter() {
        let env = prepare_target_env(config, static_hashes, target)?;
        build_target(config, env, target, fs)?
    }

    Ok(())
}

fn prepare_target_env<'a>(config: &BuildConfig, static_hashes: &HashMap<PathBuf, String>, target: &'a BuildTarget) -> Result<Environment<'a>, Box<dyn std::error::Error + Send + Sync>> {
    fn setup_loader(env: &mut Environment, config: &BuildConfig, target: &BuildTarget) {
        let root_loader = minijinja::path_loader(&config.source);
        let target_loader = minijinja::path_loader(target.dir());

        env.set_loader(move |name| {
            Ok(if name.starts_with("~/") {
                target_loader(&name[2..])?
            } else {None}
                .or(root_loader(name)?)
                .or(target_loader(name)?)
            )
        });
    }
    fn setup_filters(env: &mut Environment, config: &BuildConfig) {}
    fn setup_functions(env: &mut Environment, config: &BuildConfig) {
        env.add_function("blocks", blocks);
        env.add_function("static", static_ref);
    }
    fn setup_state(env: &mut Environment, config: &BuildConfig, target: &BuildTarget, static_hashes: &HashMap<PathBuf, String>) {
        env.add_global(RENDERER_STATE, Value::from_object(RendererState::new(RendererStateParams {
            config: config.clone(),
            target_path: target.dir().to_path_buf(),
            static_hashes: static_hashes.clone(),
        })));
    }

    let mut env = Environment::new();
    setup_state(&mut env, &config, &target, static_hashes);
    setup_loader(&mut env, &config, &target);
    setup_filters(&mut env, &config);
    setup_functions(&mut env, &config);
    Ok(env)
}

fn build_target<FS: GenFS>(config: &BuildConfig, env: Environment, target: &BuildTarget, fs: &mut FS) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let template = env.get_template(&target.config.template)?;
    let dir = PathBuf::from(&config.output).join(&target.config.path.trim_start_matches("/"));
    let index = dir.join("index.html");
    fs.create_dir_all(dir)?;
    let state = template.render_to_write((), fs.create_file(index)?)?;
    Ok(())
}