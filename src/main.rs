mod config;
mod build;
mod cli_parser;
mod server;
mod util;

use clap::arg;
use conf::{Conf, Subcommands};
use partially::Partial;
use toml::de::Error;
use toml::Deserializer;
use crate::build::build;
use crate::Command::Build;
use crate::server::serve;
use crate::config::{reSsgConfig, PartialreSsgConfig, PartialBuildConfig, Mergable};


/// Simple program to greet a person
#[derive(Debug)]
pub struct Arguments {
    command: Command
}

impl ::conf::Conf for Arguments {
    fn get_parser_config() -> Result<::conf::ParserConfig, ::conf::Error> {
        let parser_config = conf::ParserConfig { about: Some("Simple program to greet a person"), name: "reSsg", no_help_flag: false, styles: None, version: None };
        Ok(parser_config)
    }
    const PROGRAM_OPTIONS: ::conf::lazybuf::LazyBuf<::conf::ProgramOption> = {
        <PartialreSsgConfig as Conf>::PROGRAM_OPTIONS
    };
    fn get_subcommands(__parsed_env__: &::conf::ParsedEnv) -> Result<Vec<::conf::Parser>, ::conf::Error> {
        let mut __parsers__ = vec![];
        if !__parsers__.is_empty() { panic!("Not supported to have multiple subcommands fields on the same struct: at field 'command'"); }
        __parsers__.extend(<Command
        as ::conf::Subcommands>::get_parsers(__parsed_env__)?);
        Ok(__parsers__)
    }
    fn from_conf_context<'a>(__conf_context__: ::conf::ConfContext<'a>) -> Result<Self, Vec<::conf::InnerError>> {
        let mut __errors__ = Vec::<::conf::InnerError>::new();
        let __conf_context__ = &__conf_context__;
        let command = {
            fn command(__conf_context__: &::conf::ConfContext<'_>) -> Result<Command
                , ::std::vec::Vec<::conf::InnerError>> {
                use ::conf::{InnerError, Subcommands};
                let Some((name, conf_context)) = __conf_context__.for_subcommand() else {
                    return Err(vec![InnerError::missing_required_subcommand("Arguments", "command", <Command
                    as Subcommands>::get_subcommand_names())]);
                };
                <Command
                as Subcommands>::from_conf_context(name, conf_context)
            }
            match command(__conf_context__) {
                Ok(val) => Some(val),
                Err(errs) => {
                    __errors__.extend(errs);
                    None
                }
            }
        };
        if !__errors__.is_empty() { return Err(__errors__); }
        let return_value = match (command) {
            ( Some(command) ) => Arguments {
                command
            },
            _ => panic!("Internal error: no errors encountered but struct was incomplete")
        };   fn validation<'ctxctx>(__instance__: &Arguments, __conf_context__: &::conf::ConfContext<'ctxctx>) -> Result<(), Vec<::conf::InnerError>> { Ok(()) }
        validation(&return_value, __conf_context__)?;
        Ok(return_value)
    }
    fn get_name() -> &'static str { "Arguments" }
    fn debug_asserts() {
        {
            let mut short_forms = ::std::collections::HashMap::<char, String>::new();
            for opt in Self::PROGRAM_OPTIONS.iter() { if let Some(short) = opt.short_form { if let Some(existing_id) = short_forms.insert(short, opt.id.to_string()) { panic!("Short option '{}' is used by both '{}' and '{}' in {}", short, existing_id, opt.id, stringify!(Self )); } } }
        }
        <Command
        as ::conf::Subcommands>::debug_asserts();
    }
}

#[derive(Subcommands, Debug)]
enum Command {
    Build(PartialBuildConfig),
    Serve(PartialreSsgConfig),
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Arguments::parse();

    std::env::var("RESSG_ROOT").map_or(Ok(()), |dir| {
        std::env::set_current_dir(dir)
    })?;

    let config_file = std::env::current_dir()?
        .join("config.toml");

    let mut config: reSsgConfig = toml::from_slice(&std::fs::read(config_file)?)?;
    match args.command {
        Command::Build(cfg) => {
            config.build.merge(cfg);
            build(&config.build, &mut rsfs::disk::FS {})?;
        }
        Command::Serve(cfg) => {
            config.merge(cfg);
            serve(&config)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::sync::Arc;
    use minijinja::value::{Value, Object, Enumerator};
    use minijinja::{Environment, context};
    use crate::util::md_parser::MdValue;

    #[derive(Debug)]
    struct User {
        username: String,
        roles: Vec<String>,
    }

    impl Object for User {
        // The get_value method is called when accessing attributes in a template
        // (e.g., `user.username`). The key is passed as a &Value.
        fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
            match key.as_str()? {
                "username" => Some(Value::from(&self.username)),
                "roles" => Some(Value::from(self.roles.clone())),
                _ => None,
            }
        }

        // The enumerate method is used for iteration (e.g., `for key in user`).
        // It should return an Enumerator over the available keys.
        fn enumerate(self: &Arc<Self>) -> Enumerator {
            Enumerator::Str(&["username", "roles"])
        }
    }

    #[test]
    fn main() {
        let mut env = Environment::new();
        let user = User {
            username: "johndoe".to_string(),
            roles: vec!["user".to_string(), "admin".to_string()],
        };

        let mdval0 = MdValue::new("asd".to_string());
        let mdval1 = MdValue::list(vec!["asd".to_string()]);
        let mdval2 = MdValue::list(vec!["asd".to_string(), "dsa".to_string()]);
        let mdvalm = MdValue::just_attrs({
            let mut map = HashMap::new();
            map.insert(String::from("a"), MdValue::new("asd".to_string()));
            map.insert(String::from("b"), MdValue::new("bsd".to_string()));
            map
        });

        // Add the object to the environment as a global or in the render context
        env.add_template_owned("profile", r#"
Hello {{ user.username }}!
Roles: {{ user.roles }}
{{ mdval0 }}
{{ mdval1 }}
{{ mdval2 }}
{{ mdvalm }}
{{ mdval1[0] }}
{{ mdval2[0] }}
{{ mdval2[1] }}
{{ mdvalm["a"] }}
{{ mdvalm["b"] }}
        "#).unwrap();

        // The object must be wrapped in an Arc to be managed by MiniJinja's reference counting system
        let value = Value::from_object(user);

        let tmpl = env.get_template("profile").unwrap();
        let render_result = tmpl.render(context! {
            user => value,
            mdval0 => Value::from_object(mdval0),
            mdval1 => Value::from_object(mdval1),
            mdval2 => Value::from_object(mdval2),
            mdvalm => Value::from_object(mdvalm),
        }).unwrap();

        println!("{}", render_result);
        // Output: Hello johndoe! Roles: user, admin
    }
}