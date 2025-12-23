use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use anyhow::anyhow;
use markdown::{Constructs, ParseOptions};
use markdown::mdast::{Html, Text, Node};
use minijinja::{Error, State, Value};
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use toml::Table;
use crate::build::renderer::{RendererState, RENDERER_STATE};

pub struct IOError {
    e: std::io::Error,
}

impl From<std::io::Error> for IOError {
    fn from(e: std::io::Error) -> Self {
        IOError { e }
    }
}
impl Into<Error> for IOError {
    fn into(self) -> Error {
        Error::custom(format!("IO Error: {}", self.e)).with_source(self.e)
    }
}

pub struct TomlError {
    e: toml::de::Error,
}

impl From<toml::de::Error> for TomlError {
    fn from(e: toml::de::Error) -> Self {
        TomlError { e }
    }
}
impl Into<Error> for TomlError {
    fn into(self) -> Error {
        Error::custom(format!("Toml Error: {}", self.e)).with_source(self.e)
    }
}
pub fn map_toml_error(e: toml::de::Error) -> Error {
    TomlError::from(e).into()
}


pub fn map_io_error(e: std::io::Error) -> Error {
    IOError::from(e).into()
}

struct ContextBuilder {
    template: Option<String>,
    user_config: Option<toml::Table>,
    first: bool,
    data: HashMap<String, String>,
    current_section: Option<String>,
}

impl ContextBuilder {
    pub fn new(default_template: &Option<String>) -> Self {

        Self {
            template: default_template.clone(),
            user_config: None,
            first: true,
            data: HashMap::new(),
            current_section: None,
        }
    }
    pub fn add(&mut self, node: Node) -> Result<(), Error> {
        let res = match node {
            Node::Toml(data) => {
                if !self.first {
                    Err(Error::custom("Duplicating toml entry".to_string()))
                } else {
                    let mut table: Table = toml::from_str(data.value.as_str()).map_err(map_toml_error)?;
                    if let Some(toml::Value::String(template)) = table.remove("template") {
                        self.template = Some(template);
                    }
                    self.user_config = Some(table);
                    Ok(())
                }
            }
            Node::Heading(data) => {
                let [data] = data.children.try_into().map_err(|e| Error::custom("Multiple heading children not supported."))?;
                match data {
                    Node::Text(text) => {
                        self.current_section = Some(text.value);
                        Ok(())
                    }
                    _ => {
                        Err(Error::custom("Heading must contain exactly one Text node."))
                    }
                }
            }
            Node::Paragraph(data) => {
                match &self.current_section {
                    None => Err(Error::custom("No heading for paragraph found.")),
                    Some(heading) => {
                        let mut err = false;
                        for data in data.children.into_iter() {
                            match data {
                                Node::Text(Text {value: text, ..}) | Node::Html(Html {value: text, ..}) => {
                                    match self.data.get_mut(heading) {
                                        None => {
                                            self.data.insert(heading.to_string(), text);
                                        }
                                        Some(value) => {
                                            value.push_str(&text);
                                        }
                                    }
                                }
                                _ => {
                                    err = true
                                }
                            }
                        }
                        match err {
                            false => Ok(()),
                            true => Err(Error::custom("Could not parse document")),
                        }
                    }
                }
            }
            Node::Html(data) => {
                match &self.current_section {
                    None => Err(Error::custom("No heading for paragraph found.")),
                    Some(heading) => {
                        match self.data.get_mut(heading) {
                            None => {
                                self.data.insert(heading.to_string(), data.value);
                            }
                            Some(value) => {
                                value.push_str(&data.value);
                            }
                        }
                        Ok(())
                    }
                }
            }
            _ => {
                Err(Error::custom(format!("Unsupported node type: {:?}", node)))
            }
        };
        self.first = false;
        res
    }

    pub fn finalize(self, state: &State) -> Result<Context, Error> {
        let mut data = HashMap::new();
        for (k, v) in self.data {
            data.insert(k, Value::from_safe_string(state.env().render_str(v.as_str(), ())?));
        }
        Ok(Context {
            template: self.template.ok_or(Error::custom("No template specified."))?,
            config: self.user_config.unwrap_or(Default::default()),
            data,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Context {
    template: String,
    config: Table,
    data: HashMap<String, Value>,
}

pub fn blocks(state: &State, mut dir: String, default_template: Option<String>) -> Result<Value, Error> {
    if dir.starts_with("./") {
        dir = PathBuf::from(state.name()).parent().unwrap_or(Path::new("../../..")).join(dir).to_str().ok_or(
            Error::custom("Not a valid unicode")
        )?.to_string();
    }
    let state_binding= state.lookup(RENDERER_STATE).ok_or_else(|| {
        Error::custom(format!("`{}` variable not found in env", RENDERER_STATE))
    })?;
    let target_root = &state_binding.downcast_object_ref::<RendererState>()
        .ok_or(anyhow!("No renderer state is present"))
        .and_then(|x| x.get())
        .map_err(|e|{
        Error::custom(e)
    })?.target_path.clone();

    let blocks_dir = target_root.join(dir);
    if !blocks_dir.exists() {
        return Err(Error::custom(format!("Blocks directory `{}` not found.", blocks_dir.display())));
    }
    if !blocks_dir.is_dir() {
        return Err(Error::custom(format!("Blocks directory `{}` is not a directory.", blocks_dir.display())));
    }
    let mut files = vec![];
    for entry in blocks_dir.read_dir().map_err(map_io_error)? {
        let entry = entry.map_err(map_io_error)?.path();

        if !entry.is_file() {
            continue;
        }
        match entry.extension() {
            Some(ext) if ext == "md" || ext == "html" => {
                files.push(entry);
            }
            _ => {}
        }
    }
    let mut results = vec![];
    for entry in itertools::sorted(files.into_iter()) {
        if let Some(ext) = entry.extension() && ext == "html"  {
            let entry = entry.strip_prefix(target_root.as_path()).map_err(|_| Error::custom(format!("Failed to strip prefix `{}` for `{}` .", target_root.display(), entry.display())))?;
            results.push(state.env().get_template(entry.to_str().ok_or(
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Not utf-8 path")
            ).map_err(|e| {Error::custom(format!("{}", e))})?)?.render(())?);
            continue;
        }
        let content = markdown::to_mdast(&std::fs::read_to_string(&entry).map_err(map_io_error)?, &ParseOptions {
            constructs: Constructs {
                frontmatter: true,
                heading_atx: true,
                ..Constructs::default()
            },
            ..ParseOptions::default()
        }).map_err(|e| {Error::custom(format!("{}", e))})?;
        let content = match content {
            Node::Root(c) => {c}
            _ => {
                return Err(Error::custom(format!("Can not find root node for file `{}`", entry.display())));
            }
        };
        let mut context_builder = ContextBuilder::new(&default_template);
        for node in content.children {
            context_builder.add(node)?;
        }

        let context = context_builder.finalize(state)?;

        let template = state.env().get_template(context.template.as_str())?;
        results.push(template.render(&context)?);
    }

    Ok(Value::from_safe_string(results.join("\n")))
}