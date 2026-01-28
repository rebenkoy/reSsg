use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::config::BuildConfig;
use minijinja::{context, AutoEscape, Environment, UndefinedBehavior, Value};
use rsfs::GenFS;
use serde::{Deserialize, Serialize};
use crate::build::custom_functions::{blocks, include_sass, sass, try_add_class, SassState};
use crate::build::custom_functions::static_ref;
use crate::build::renderer_state::{RendererState, RendererStateParams, RENDERER_STATE};
use crate::util::md_parser::MdValue;

pub struct BuildTarget {
    pub path: PathBuf,
    pub config: TargetConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TargetConfig {
    pub path: String,
    pub template: String,
}

impl BuildTarget {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        Ok(Self {
            config: toml::from_slice(&fs::read(&path)?)?,
            path,
        })
    }
    pub fn dir(&self) -> std::io::Result<PathBuf> {
        Ok(self.path.parent().ok_or(
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "No parent dir found")
        )?.to_path_buf())
    }
}

pub fn prepare_target_env<'a>(config: &BuildConfig, static_hashes: &HashMap<PathBuf, String>, target: &'a BuildTarget, out_dir: PathBuf, out_prefix: PathBuf, sass_hash: Option<String>) -> anyhow::Result<Environment<'a>> {
    fn setup_loader(env: &mut Environment, config: &BuildConfig, target: &BuildTarget) -> anyhow::Result<()> {
        let root_loader = minijinja::path_loader(&config.source);
        let target_loader = minijinja::path_loader(target.dir()?);

        env.set_loader(move |name| {
            Ok(if name.starts_with("~/") {
                target_loader(&name[2..])?
            } else {None}
                .or(root_loader(name)?)
                .or(target_loader(name)?)
            )
        });
        Ok(())
    }
    fn setup_filters(env: &mut Environment, config: &BuildConfig) {
        env.add_filter("try_add_class", try_add_class);
    }
    fn setup_functions(env: &mut Environment, config: &BuildConfig) {
        env.add_function("blocks", blocks);
        env.add_function("static", static_ref);
        env.add_function("sass", sass);
        env.add_function("include_sass", include_sass);
    }
    fn setup_state(env: &mut Environment, config: &BuildConfig, target: &BuildTarget, out_dir: PathBuf, out_prefix: PathBuf, static_hashes: &HashMap<PathBuf, String>, sass_hash: Option<String>) -> anyhow::Result<()> {
        env.add_global(RENDERER_STATE, Value::from_object(RendererState::new(RendererStateParams {
            config: config.clone(),
            target_path: target.dir()?.to_path_buf(),
            out_dir,
            out_prefix,
            static_hashes: static_hashes.clone(),
            sass_hash,
        })));
        Ok(())
    }

    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Lenient);
    env.set_auto_escape_callback(|name| AutoEscape::None);
    setup_state(&mut env, &config, &target, out_dir, out_prefix, static_hashes, sass_hash)?;
    setup_loader(&mut env, &config, &target)?;
    setup_filters(&mut env, &config);
    setup_functions(&mut env, &config);
    Ok(env)
}

pub fn build_target<FS: GenFS>(config: &BuildConfig, static_hashes: &HashMap<PathBuf, String>, target: &BuildTarget, fs: &mut FS) -> anyhow::Result<()> {
    let out_prefix = target.config.path.trim_start_matches("/");
    let dir = PathBuf::from(&config.output).join(out_prefix);
    let index = dir.join("index.html");
    fs.create_dir_all(&dir)?;

    let env = prepare_target_env(&config, &static_hashes, &target, dir.clone(), PathBuf::from(out_prefix), None)?;
    let template = env.get_template(&target.config.template)?;
    let ctx = ();
    let (_, state) = template.render_and_return_state(ctx.clone())?;  // Prerender to collect all deferred values.
    let sass_hash = SassState::build(&state, &dir, fs)?;

    let env = prepare_target_env(&config, &static_hashes, &target, dir.clone(), PathBuf::from(out_prefix), sass_hash)?;
    let template = env.get_template(&target.config.template)?;
    let state = template.render_to_write(ctx, fs.create_file(index)?)?;
    Ok(())
}