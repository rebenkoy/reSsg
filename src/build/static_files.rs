use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use rsfs::{DirEntry, FileType, GenFS, Metadata};
use sha1::{Digest, Sha1};
use crate::build::build;
use crate::config::BuildConfig;

fn copy_all<FS: GenFS>(from: &PathBuf, to: &PathBuf, fs: &mut FS) -> anyhow::Result<()> {
    if from.is_dir() {
        fs.create_dir_all(&to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_all(&entry.path(), &to.join(entry.file_name()), fs)?;
        }
    } else if from.is_file() {
        let bytes = std::fs::read(from)?;
        let mut f = fs.create_file(to)?;
        f.write_all(&bytes)?;
    } else if from.is_symlink() {
        copy_all(&std::fs::read_link(from)?, to, fs)?;
    }
    Ok(())
}

fn collect_hashes<FS: GenFS>(root: &PathBuf, config: &BuildConfig, fs: &FS) -> anyhow::Result<HashMap<PathBuf, String>> {
    fn _collect_hashes<FS: GenFS>(map: &mut HashMap<PathBuf, String>, path: &PathBuf, config: &BuildConfig, fs: &FS) -> anyhow::Result<()> {
        let meta = fs.metadata(path)?.file_type();
        if meta.is_dir() {
            for entry in fs.read_dir(path)? {
                let entry = entry?;
                _collect_hashes(map, &entry.path(), config, fs)?;
            }
        } else if meta.is_file() {
            let mut hasher = Sha1::new();
            let mut bytes = Vec::new();
            fs.open_file(path)?.read_to_end(&mut bytes)?;
            hasher.update(bytes);
            map.insert(path.to_path_buf().strip_prefix(&config.output)?.to_path_buf(), hex::encode(hasher.finalize().as_slice()));
        }

        Ok(())
    }
    let mut hashes = HashMap::new();
    _collect_hashes(&mut hashes, root, config, fs)?;
    Ok(hashes)
}

pub fn build_static<FS: GenFS>(config: &BuildConfig, fs: &mut FS) -> anyhow::Result<HashMap<PathBuf, String>> {
    let static_output = PathBuf::from(&config.output).join(&config.static_output);
    copy_all(&PathBuf::from(&config.static_path), &static_output, fs)?;
    // build_sass();
    Ok(collect_hashes(&static_output, config, fs)?)
}