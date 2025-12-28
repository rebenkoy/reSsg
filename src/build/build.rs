use crate::config::BuildConfig;
use rsfs::GenFS;
use crate::build::{static_files, target_discovery};
use crate::build::target_renderer::build_target;

fn prepare_output<FS: GenFS>(path: &String, fs: &mut FS) -> anyhow::Result<()> {
    match fs.remove_dir_all(&path) {
        Ok(_) => {}
        Err(e) => {
            match e.kind() {
                std::io::ErrorKind::DirectoryNotEmpty => {
                    return Err(anyhow::Error::from(e))
                }
                _ => {}
            }
        }
    };
    fs.create_dir_all(&path)?;
    Ok(())
}

pub fn build<FS: GenFS>(config: &BuildConfig, fs: &mut FS) -> anyhow::Result<()> {
    prepare_output(&config.output, fs)?;
    let static_hashes = static_files::build_static(config, fs)?;

    let targets = target_discovery::locate_targets(config)?;
    target_discovery::validate_targets(&targets)?;

    for (_, target) in targets.iter() {
        build_target(config, &static_hashes, target, fs)?
    }

    Ok(())
}