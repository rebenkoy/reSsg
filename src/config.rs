use conf::{Conf, ConfContext, ConfSerde, ConfSerdeContext, Error, InitializationStateMachine, InnerError, NextValueProducer, ParsedEnv, Parser, ParserConfig, ProgramOption};
use conf::lazybuf::LazyBuf;
use serde::{Deserialize, Serialize};
use minijinja::value::Object;
use partially::Partial;

pub trait Mergable {
    type Partial;
    fn merge(&mut self, part: Self::Partial);
}

#[derive(Partial)]
#[partially(derive(Conf, Debug))]
#[derive(Debug, Clone, Serialize, Deserialize, Conf)]
pub struct reSsgConfig {
    #[partially(as_type = "Option<PartialServerConfig>")]
    #[conf(flatten, long_prefix="server.")]
    pub server: ServerConfig,
    #[partially(as_type = "Option<PartialBuildConfig>")]
    #[conf(flatten, long_prefix="build.")]
    pub build: BuildConfig,
}

impl Mergable for reSsgConfig {
    type Partial = PartialreSsgConfig;
    fn merge(&mut self, part: Self::Partial) {
        part.server.map(|c| self.server.merge(c));
        part.build.map(|c| self.build.merge(c));
    }
}

#[derive(Partial)]
#[partially(derive(Conf, Debug))]
#[derive(Debug, Clone, Serialize, Deserialize, Conf)]
pub struct ServerConfig {
    #[partially(as_type = "Option<PartialControlConfig>")]
    #[conf(flatten, long_prefix="control.")]
    pub control: ControlConfig,
    #[partially(as_type = "Option<PartialEndpointConfig>")]
    #[conf(flatten, long_prefix="output.")]
    pub output: EndpointConfig,
    #[conf(repeat, long)]
    #[partially(omit)]
    pub watch_excludes: Vec<String>,
}

impl Mergable for ServerConfig {
    type Partial = PartialServerConfig;
    fn merge(&mut self, part: Self::Partial) {
        part.control.map(|c| self.control.merge(c));
        part.output.map(|c| self.output.merge(c));
    }
}

impl From<PartialServerConfig> for ServerConfig {
    fn from(s: PartialServerConfig) -> Self {
        todo!()
    }
}

#[derive(Partial)]
#[partially(derive(Conf, Debug))]
#[derive(Debug, Clone, Serialize, Deserialize, Conf)]
pub struct EndpointConfig {
    #[arg(long)]
    pub port: String,
    #[arg(long)]
    pub interface: String,
}

impl Mergable for EndpointConfig {
    type Partial = PartialEndpointConfig;
    fn merge(&mut self, part: Self::Partial) {
        part.port.map(|p| self.port = p);
        part.interface.map(|i| self.interface = i);
    }
}

impl From<PartialEndpointConfig> for EndpointConfig {
    fn from(p: PartialEndpointConfig) -> Self {
        p.port.map(|port|
            p.interface.map(|interface| {
                Self {
                    port,
                    interface,
                }
            }).expect("Unsaturated partial conversion")
        ).expect("Unsaturated partial conversion")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlConfig {
    None,
    Endpoint(EndpointConfig),
    Prefix(String),
}

impl Mergable for ControlConfig {
    type Partial = PartialControlConfig;
    fn merge(&mut self, part: Self::Partial) {
        match (part.endpoint, part.prefix) {
            (None, None) => {
                return;
            }
            (None, Some(prefix)) => {
                *self = Self::Prefix(prefix);
            }
            (Some(part), None) => {
                match self {
                    ControlConfig::None => {}
                    ControlConfig::Endpoint(ep) => {
                        ep.merge(part);
                    }
                    ControlConfig::Prefix(_) => {
                        Self::Endpoint(part.into());
                    }
                }
            }
            (Some(p), Some(p_prefix)) => {
                panic!("Double assignment to control config")
            }
        }
    }
}

#[derive(Debug, Conf)]
#[conf(at_most_one_of_fields(endpoint, prefix))]
pub struct PartialControlConfig {
    #[conf(flatten, long_prefix = "endpoint.")]
    endpoint: Option<PartialEndpointConfig>,
    #[arg(long)]
    prefix: Option<String>,
}

impl From<PartialControlConfig> for ControlConfig {
    fn from(value: PartialControlConfig) -> Self {
        if let Some(config) = value.endpoint {
            ControlConfig::Endpoint(config.try_into().expect("Not full endpoint config"))
        } else if let Some(config) = value.prefix {
            ControlConfig::Prefix(config)
        } else {
            ControlConfig::None
        }
    }
}

impl Conf for ControlConfig {
    fn debug_asserts() {
        PartialControlConfig::debug_asserts();
    }

    fn get_parser_config() -> Result<ParserConfig, Error> {
        PartialControlConfig::get_parser_config()
    }

    const PROGRAM_OPTIONS: LazyBuf<ProgramOption> = PartialControlConfig::PROGRAM_OPTIONS;

    fn get_subcommands(parsed_env: &ParsedEnv) -> Result<Vec<Parser>, Error> {
        PartialControlConfig::get_subcommands(parsed_env)
    }

    fn from_conf_context(conf_context: ConfContext<'_>) -> Result<Self, Vec<InnerError>> {
        let x = PartialControlConfig::from_conf_context(conf_context).map(|x| x.into());
        println!("{:#?}", x);
        x
    }

    fn get_name() -> &'static str {
        PartialControlConfig::get_name()
    }
}

#[derive(Partial)]
#[partially(derive(Conf, Debug))]
#[derive(Debug, Clone, Serialize, Deserialize, Conf)]
pub struct BuildConfig {
    #[arg(long)]
    pub source: String,
    #[arg(long)]
    pub index_toml_name: String,
    #[arg(long)]
    pub output: String,
    #[arg(long)]
    pub prefix: String,
    #[arg(long)]
    pub static_path: String,
    #[arg(long)]
    pub static_output: String,
    #[partially(as_type = "Option<PartialSassConfig>")]
    #[conf(flatten, long_prefix="sass.")]
    pub sass: SassConfig,
}

impl Mergable for BuildConfig {
    type Partial = PartialBuildConfig;

    fn merge(&mut self, part: Self::Partial) {
        part.source.map(|p| self.source = p);
        part.index_toml_name.map(|p| self.index_toml_name = p);
        part.output.map(|p| self.output = p);
        part.prefix.map(|p| self.prefix = p);
        part.static_path.map(|p| self.static_path = p);
        part.static_output.map(|p| self.static_output = p);
        part.sass.map(|p| self.sass.merge(p));
    }
}

impl From<PartialBuildConfig> for BuildConfig {
    fn from(value: PartialBuildConfig) -> Self {
        panic!()
    }
}

impl Object for BuildConfig {}

#[derive(Partial)]
#[partially(derive(Conf, Debug))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Conf)]
pub struct SassConfig {
    #[arg(long)]
    pub source: String,
    #[arg(long)]
    pub destination: String,
}

impl Mergable for SassConfig {
    type Partial = PartialSassConfig;
    fn merge(&mut self, part: Self::Partial) {
        part.source.map(|p| self.source = p);
        part.destination.map(|p| self.destination = p);
    }
}
impl From<PartialSassConfig> for SassConfig {
    fn from(value: PartialSassConfig) -> Self {
        todo!()
    }
}