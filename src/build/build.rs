use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use crate::config::BuildConfig;
use sha1::{Sha1, Digest};
use hex;
use rsfs::{DirEntry, FileType, GenFS, Metadata};
use crate::build::templates::build_templates;

fn prepare_output<FS: GenFS>(path: &String, fs: &mut FS) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match fs.remove_dir_all(&path) {
        Ok(_) => {}
        Err(e) => {
            match e.kind() {
                std::io::ErrorKind::DirectoryNotEmpty => {
                    return Err(Box::new(e))
                }
                _ => {}
            }
        }
    };
    fs.create_dir_all(&path)?;
    Ok(())
}

fn copy_all<FS: GenFS>(from: &PathBuf, to: &PathBuf, fs: &mut FS) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

fn collect_hashes<FS: GenFS>(root: &PathBuf, config: &BuildConfig, fs: &FS) -> Result<HashMap<PathBuf, String>, Box<dyn std::error::Error + Send + Sync>> {
    fn _collect_hashes<FS: GenFS>(map: &mut HashMap<PathBuf, String>, path: &PathBuf, config: &BuildConfig, fs: &FS) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

fn build_static<FS: GenFS>(config: &BuildConfig, fs: &mut FS) -> Result<HashMap<PathBuf, String>, Box<dyn std::error::Error + Send + Sync>> {
    let static_output = PathBuf::from(&config.output).join(&config.static_output);
    copy_all(&PathBuf::from(&config.static_path), &static_output, fs)?;
    // build_sass();
    Ok(collect_hashes(&static_output, config, fs)?)
}

pub fn build<FS: GenFS>(config: &BuildConfig, fs: &mut FS) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    prepare_output(&config.output, fs)?;
    let static_hashes = build_static(config, fs)?;
    build_templates(&config, &static_hashes, fs)?;
    Ok(())
}