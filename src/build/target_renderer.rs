use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use anyhow::anyhow;
use crate::config::BuildConfig;
use minijinja::{context, AutoEscape, Environment, UndefinedBehavior, Value};
use rsfs::GenFS;
use serde::{Deserialize, Serialize};
use crate::build::custom_functions::{
    StatefulFunction,
    blocks,
    include_sass,
    sass,
    try_add_class,
    RemValueState,
    push_to_array, collected_array,
    SassState
};
use crate::build::custom_functions::static_ref;
use crate::build::renderer_state::{RendererState, RendererStateParams, RENDERER_STATE};
use crate::util::md_parser::MdValue;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjPaths {
    pub proj_root: PathBuf,
    pub src_root: PathBuf,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BuildTarget {
    pub proj_paths: ProjPaths,
    pub self_root: PathBuf,
    pub config: TargetConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    pub path: String,
    pub template: String,
}

impl BuildTarget {
    pub fn new(proj_paths: ProjPaths, path: PathBuf) -> anyhow::Result<Self> {
        Ok(Self {
            proj_paths,
            config: toml::from_slice(&fs::read(&path)?)?,
            self_root: path.parent().ok_or(anyhow!("Can not get target root {}", path.to_string_lossy()))?.into(),
        })
    }

    pub fn canonical_path<F: AsRef<str>>(&self, file: F) -> PathBuf {
        let file = file.as_ref();
        let file = file.strip_prefix(self.proj_paths.proj_root.to_string_lossy().as_ref()).unwrap_or(file);
        match file {
            _ if file.starts_with("/") => {
                self.proj_paths.proj_root.join(&file[1..])
            }
            _ if file.starts_with("$/") => {
                self.proj_paths.src_root.join(&file[2..])
            }
            _ if file.starts_with("~/") => {
                self.self_root.join(&file[2..])
            }
            _ => {
                self.self_root.join(&file)
            }
        }
    }
}

pub fn prepare_target_env<'a>(
    config: &BuildConfig,
    static_hashes: &HashMap<PathBuf, String>,
    target: &'a BuildTarget,
    out_dir: PathBuf,
    out_prefix: PathBuf,
    sass_hash: Option<String>,
    remvalue: RemValueState,
) -> anyhow::Result<Environment<'a>> {
    fn setup_loader(env: &mut Environment, config: &BuildConfig, target: &BuildTarget) -> anyhow::Result<()> {
        let target = (*target).clone();
        let proj_root_loader = minijinja::path_loader(&target.proj_paths.proj_root);
        let proj_root = target.proj_paths.proj_root.to_str().ok_or(anyhow!("Can not get proj_root"))?.to_string();

        env.set_loader(move |name| {
            let real_name = target.canonical_path(name);
            let tmp = real_name.to_string_lossy();
            let lookup_name = tmp.as_ref();

            Ok(proj_root_loader(lookup_name.strip_prefix(proj_root.as_str()).unwrap_or(lookup_name))?)
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
        env.add_function("push_to_array", push_to_array);
        env.add_function("collected_array", collected_array);
}
    fn setup_state(
        env: &mut Environment,
        config: &BuildConfig,
        target: &BuildTarget,
        out_dir: PathBuf,
        out_prefix: PathBuf,
        static_hashes: &HashMap<PathBuf, String>,
        sass_hash: Option<String>,
        remvalue: RemValueState,
    ) -> anyhow::Result<()> {
        let mut state = Value::from_object(RendererState::new(RendererStateParams {
            config: config.clone(),
            target: target.clone(),
            target_path: target.self_root.clone(),
            out_dir,
            out_prefix,
            static_hashes: static_hashes.clone(),
            sass_hash,
            remvalue,
        }));
        state.downcast_object::<RendererState>()
            .ok_or(anyhow!("Can not downcast rendererState"))?
            .push_reason(target.self_root.clone())?;
        env.add_global(RENDERER_STATE, state);
        Ok(())
    }

    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Lenient);
    env.set_auto_escape_callback(|name| AutoEscape::None);
    setup_state(&mut env, &config, &target, out_dir, out_prefix, static_hashes, sass_hash, remvalue)?;
    setup_loader(&mut env, &config, &target)?;
    setup_filters(&mut env, &config);
    setup_functions(&mut env, &config);
    Ok(env)
}

pub fn build_target<FS: GenFS>(config: &BuildConfig, static_hashes: &HashMap<PathBuf, String>, target: &BuildTarget, fs: &mut FS) -> anyhow::Result<()> {
    let out_name = target.config.path.trim_start_matches("/");
    let dir = PathBuf::from(&config.output).join(out_name);
    let mut out_prefix = PathBuf::from(&config.prefix).join(out_name);
    if !out_prefix.starts_with("/") {
        out_prefix = PathBuf::from("/").join(out_prefix);
    }
    let index = dir.join("index.html");
    fs.create_dir_all(&dir)?;

    let env = prepare_target_env(&config, &static_hashes, &target, dir.clone(), out_prefix.clone(), None, RemValueState::default())?;
    let template = env.get_template(&target.config.template)?;
    let ctx = ();
    let (_, state) = template.render_and_return_state(ctx.clone())?;  // Prerender to collect all deferred values.
    let sass_hash = SassState::build(&state, fs, &target)?;
    let remvalue = RemValueState::build(&state, fs)?;

    let env = prepare_target_env(&config, &static_hashes, &target, dir.clone(), out_prefix, sass_hash, remvalue)?;
    let template = env.get_template(&target.config.template)?;
    let state = template.render_to_write(ctx, fs.create_file(index)?)?;
    Ok(())
}