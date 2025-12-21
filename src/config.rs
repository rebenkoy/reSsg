use std::path::Prefix;
use serde::{Deserialize, Serialize};
use clap::{ArgMatches, Args, Command, Error, FromArgMatches};
use clap::builder::Resettable;
use minijinja::value::Object;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct reSsgConfig {
    pub server: ServerConfig,
    pub build: BuildConfig,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub control: ControlConfig,
    pub output: EndpointConfig,
    pub watch_excludes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub port: String,
    pub interface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlConfig {
    None,
    Endpoint(EndpointConfig),
    Prefix(String)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub source: String,
    pub index_toml_name: String,
    pub output: String,
    pub prefix: String,
    pub static_path: String,
    pub static_output: String,
    pub sass: SassConfig,
}

impl Object for BuildConfig {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SassConfig {
    pub source: String,
    pub destination: String,
}


//
// impl FromArgMatches for ServerConfig {
//     fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
//         todo!()
//     }
//
//     fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
//         todo!()
//     }
// }
//
// impl Args for ServerConfig {
//     fn augment_args(mut cmd: Command) -> Command {
//         fn add_prefix(prefix: &'static str, value: String) -> String {
//             format!("{}-{}", prefix, value)
//         }
//
//         let socket = EndpointConfig::augment_args(Command::new("socketConfig"));
//         let socket_args = socket.get_arguments().cloned().collect::<Vec<_>>();
//         let socket_groupd = socket.get_groups().cloned().collect::<Vec<_>>();
//
//         for arg in socket_args {
//             let long = add_prefix("socket", arg.get_long().unwrap().to_string());
//             let id = add_prefix("socket", arg.get_id().to_string());
//             cmd = cmd.arg(arg.id(id).long(long).short(Resettable::Reset));
//         }
//         for mut group in socket_groupd {
//             cmd = cmd.group(
//                 clap::ArgGroup::new(add_prefix("socket", group.get_id().to_string()))
//                     .multiple(group.is_multiple())
//                     .args({
//                         group
//                             .get_args()
//                             .map(|arg| clap::Id::from(add_prefix("socket", arg.to_string())))
//                     })
//             )
//         }
//
//         let output = EndpointConfig::augment_args(Command::new("outputConfig"));
//         let output_args = output.get_arguments().cloned().collect::<Vec<_>>();
//         let output_groupd = output.get_groups().cloned().collect::<Vec<_>>();
//
//         for arg in output_args {
//             let long = add_prefix("output", arg.get_long().unwrap().to_string());
//             let id = add_prefix("output", arg.get_id().to_string());
//             cmd = cmd.arg(arg.id(id).long(long).short(Resettable::Reset));
//         }
//         for mut group in output_groupd {
//             cmd = cmd.group(
//                 clap::ArgGroup::new(add_prefix("output", group.get_id().to_string()))
//                     .multiple(group.is_multiple())
//                     .args({
//                         group
//                             .get_args()
//                             .map(|arg| clap::Id::from(add_prefix("output", arg.to_string())))
//                     })
//             )
//         }
//
//         cmd
//     }
//
//     fn augment_args_for_update(cmd: Command) -> Command {
//         todo!()
//     }
// }
