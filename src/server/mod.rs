mod watcher;
mod control;
mod fileserver;

use std::ops::Deref;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::{Arc, RwLock};
use actix_web::{App, HttpServer, Scope};
use actix_web::dev::Server;
use actix_web::middleware::Logger;
use futures::{FutureExt, StreamExt, TryFutureExt};
use futures::future::Either;
use rsfs::GenFS;
use crate::build::build;
use crate::config::{reSsgConfig, EndpointConfig, ControlConfig};
use crate::server::watcher::Tx;

#[actix_web::main]
pub async fn serve(config: &reSsgConfig) -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    match &config.server.control {
        ControlConfig::None => {
            no_autoreload_serve(config.clone()).await
        }
        ControlConfig::Endpoint(additional) => {
            multi_server_serve(config.clone(), additional.clone()).await
        }
        ControlConfig::Prefix(prefix) => {
            single_server_serve(config.clone(), prefix.clone()).await
        }
    }
}

fn build_output_server(config: &reSsgConfig, fs: Arc<RwLock<rsfs::mem::FS>>) -> std::io::Result<Server> {
    let config_clone = config.clone();
    Ok(HttpServer::new(move || {
        App::new()
            .service(
                Scope::new(&config_clone.build.prefix)
                    .service(fileserver::build_output_scope(&config_clone, fs.clone()))
            )
            .wrap(Logger::new("%r %s %Dms"))
    })
        .bind(format!("{}:{}", &config.server.output.interface, &config.server.output.port))?
        .run())
}

async fn no_autoreload_serve(config: reSsgConfig) -> anyhow::Result<()> {
    let mut fs = rsfs::mem::FS::new();
    build(&config.build, &mut fs).map_err(anyhow::Error::from_boxed)?;
    let fs = Arc::new(RwLock::new(fs));
    let (_, watcher) = watcher::build_watcher_tread(&config, fs.clone())?;

    let output_server = build_output_server(&config, fs)?;


    match futures::future::select(
        pin!(watcher),
        output_server,
    ).await {
        Either::Left((res, _)) => {res}
        Either::Right((res, _)) => {
            res.map_err(anyhow::Error::from)
        }
    }?;
    Ok(())
}
async fn single_server_serve(config: reSsgConfig, socket_prefix: String) -> anyhow::Result<()>  {
    let mut fs = rsfs::mem::FS::new();
    build(&config.build, &mut fs).map_err(anyhow::Error::from_boxed)?;
    let fs = Arc::new(RwLock::new(fs));
    let (rx, watcher) = watcher::build_watcher_tread(&config, fs.clone())?;

    let config_clone = config.clone();
    let combined_server = HttpServer::new(move || {
        App::new()
            .service(
                Scope::new(&socket_prefix)
                    .service(control::build_control_scope(rx.clone()))
            )
            .service(
                Scope::new(&config_clone.build.prefix)
                    .service(fileserver::build_output_scope(&config_clone, fs.clone()))
            )
            .wrap(Logger::new("%r %s %Dms"))
    })
        .bind(format!("{}:{}", &config.server.output.interface, &config.server.output.port))?
        .run();

    match futures::future::select(
        pin!(watcher),
        combined_server,
    ).await {
        Either::Left((res, _)) => {res}
        Either::Right((res, _)) => {
            res.map_err(anyhow::Error::from)
        }
    }?;
    Ok(())
}
async fn multi_server_serve(config: reSsgConfig, socket_config: EndpointConfig) -> anyhow::Result<()> {
    let mut fs = rsfs::mem::FS::new();
    build(&config.build, &mut fs).map_err(anyhow::Error::from_boxed)?;
    let fs = Arc::new(RwLock::new(fs));
    let (rx, watcher) = watcher::build_watcher_tread(&config, fs.clone())?;

    let control_server = HttpServer::new(move || {
        App::new()
            .service(control::build_control_scope(rx.clone()))
            .wrap(Logger::new("%r %s %Dms"))
    })
        .bind(format!("{}:{}", &socket_config.interface, &socket_config.port))?
        .run();

    let output_server = build_output_server(&config, fs)?;

    use futures::stream::FuturesUnordered;

    let servers = [control_server, output_server].into_iter().collect::<FuturesUnordered<_>>();

    match futures::future::select(
        pin!(watcher),
        servers.collect::<Vec<_>>(),
    ).await {
        Either::Left((res, _)) => {res}
        Either::Right((res, _)) => {
            res
                .into_iter()
                .map(|r| r.map_err(anyhow::Error::from))
                .collect()
        }
    }?;
    Ok(())

}