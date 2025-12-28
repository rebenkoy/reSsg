use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use anyhow::anyhow;
use crate::build::target_renderer::BuildTarget;
use crate::config::BuildConfig;

pub fn locate_targets(config: &BuildConfig) -> anyhow::Result<HashMap<PathBuf, BuildTarget>> {
    fn _locate_targets(config: &BuildConfig, path: &PathBuf, map: &mut HashMap<PathBuf, BuildTarget>) -> anyhow::Result<()> {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                _locate_targets(config, &entry?.path(), map)?;
            }
        } else if path.is_file() {
            if path.file_name()
                .ok_or(std::io::Error::new(std::io::ErrorKind::Other, "Filename ends with .."))?
                .to_str()
                .ok_or(std::io::Error::new(std::io::ErrorKind::Other, "Filename ends with .."))?
                == config.index_toml_name {
                map.insert(path.clone(), BuildTarget::new(path.clone())?);
            }
        }
        Ok(())
    }

    let mut pages = HashMap::new();
    _locate_targets(config, &PathBuf::from(&config.source), &mut pages)?;
    Ok(pages)
}

pub fn validate_targets(targets: &HashMap<PathBuf, BuildTarget>) -> anyhow::Result<()> {
    let mut dests: HashMap<String, &BuildTarget> = HashMap::new();
    for (_, target) in targets.iter() {
        if let Some(conflict) = dests.get(&target.config.path) {
            return Err(anyhow!(
                "Conflicting destination `{}`. first reserved in: `{}`, attempted to reserve in: `{}`",
                target.config.path, conflict.path.to_str().ok_or(
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "Not utf-8 path")
                )?, target.path.to_str().ok_or(
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "Not utf-8 path")
                )?
            ))
        }
        dests.insert(target.config.path.clone(), target);
    }

    Ok(())
}