mod files;
mod named;
mod path_buf;
mod livereload_injector;

use std::path::{PathBuf};
use std::sync::{Arc, RwLock};
use crate::config::reSsgConfig;
use actix_web::Scope;
use rsfs::GenFS;
use files::Files;
use crate::server::fileserver::livereload_injector::LivereloadInjector;

pub fn build_output_scope(config: &reSsgConfig, fs: Arc<RwLock<rsfs::mem::FS>>) -> Scope {
    Scope::new("")
        .service(
            Files::new(
                PathBuf::from(&config.build.output),
                fs,
            )
                .content_mappers(vec![
                    Arc::new(LivereloadInjector::new(&config))
                ])
        )
}
