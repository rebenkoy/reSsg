use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::{Deref, Range};
use std::path::{Path, PathBuf};
use actix_web::web::head;
use clap::builder::Str;
use minijinja::{Error, State, Value};
use pulldown_cmark::{CowStr, Event, HeadingLevel, MetadataBlockKind, Tag, TagEnd};
use pulldown_cmark_to_cmark::{cmark, Error as CmarkError};
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use toml::Table;
use crate::build::renderer_state::{get_state, lock_state, RendererState, RENDERER_STATE};

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

#[derive(Debug, Copy, Clone)]
enum SectionType {
    Literal,
    HTML,
}
impl TryFrom<u8> for SectionType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SectionType::Literal),
            2 => Ok(SectionType::HTML),
            _ => Err(Error::custom("Unknown heading type, currently only depth 0 (Literal) and depth 1 (HTML) are supported")),
        }
    }
}

#[derive(Debug)]
enum ParsingMode<'a> {
    None,
    Text{
        events: Vec<Event<'a>>,
        range: Option<Range<usize>>,
    },
    Frontmatter{
        style: MetadataBlockKind,
        events: Vec<Event<'a>>,
    },
    Heading{
        section_type: SectionType,
        level: HeadingLevel,
        events: Vec<Event<'a>>,
        id: Option<CowStr<'a>>,
        classes: Vec<CowStr<'a>>,
        attrs: Vec<(CowStr<'a>, Option<CowStr<'a>>)>,
    },
}

struct Heading<'a> {
    section_type: SectionType,
    name: String,
    id: Option<CowStr<'a>>,
    classes: Vec<CowStr<'a>>,
    attrs: Vec<(CowStr<'a>, Option<CowStr<'a>>)>,
}

struct HeadingData {
    section_type: SectionType,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(String, Option<String>)>,
}

impl From<&Heading<'_>> for HeadingData {
    fn from(heading: &Heading) -> Self {
        let Heading { section_type, name, id, classes, attrs } = heading;
        Self {
            section_type: *section_type,
            id: id.as_ref().map(|id| id.to_string()),
            classes: classes.iter().map(|c| c.to_string()).collect(),
            attrs: attrs.iter().map(|(a,v)| (a.to_string(), v.as_ref().map(|v| v.to_string()))).collect(),
        }
    }
}
struct ContextBuilder<'a> {
    source: &'a String,
    template: Option<String>,
    user_config: Option<Table>,
    data: HashMap<String, String>,
    meta: HashMap<String, HeadingData>,
    current_section: Option<Heading<'a>>,
    parsing_mode: ParsingMode<'a>,
}

impl<'a> ContextBuilder<'a> {
    pub fn new(source: &'a String, default_template: &Option<String>) -> Self {
        Self {
            source,
            template: default_template.clone(),
            user_config: None,
            data: HashMap::new(),
            meta: HashMap::new(),
            current_section: None,
            parsing_mode: ParsingMode::None,
        }
    }
    fn finalize_section(&mut self) -> Result<(), Error> {
        let mut mode = ParsingMode::None;
        std::mem::swap(&mut self.parsing_mode, &mut mode);
        match mode {
            ParsingMode::None => {}
            ParsingMode::Text { events, range } => {
                let Some(range) = range else {
                    if !events.is_empty() {
                        return Err(Error::custom("Internal Error: Empty range in text section"));
                    }
                    return Ok(());
                };
                match &self.current_section {
                    None => {
                        return Err(Error::custom("No section set for entry"))
                    }
                    Some(heading) => {
                        match heading.section_type {
                            SectionType::Literal => {
                                self.data.insert(heading.name.clone(), self.source[range].to_string());
                                self.meta.insert(heading.name.clone(), HeadingData::from(heading));
                            }
                            SectionType::HTML => {
                                let mut html = String::new();
                                pulldown_cmark::html::write_html_fmt(&mut html, events.into_iter())?;
                                self.data.insert(heading.name.clone(), html);
                                self.meta.insert(heading.name.clone(), HeadingData::from(heading));
                            }
                        }
                    }
                }
            }
            ParsingMode::Frontmatter { events, style } => {
                let mut frontmatter = String::new();
                for event in events {
                    match event {
                        Event::Text(text) => {
                            frontmatter.push_str(&text);
                        }
                        _ => {return Err(Error::custom(format!("Invalid event in frontmatter: {:?}", event)))}
                    }
                }
                match style {
                    MetadataBlockKind::YamlStyle => {
                        return Err(Error::custom("Yaml style frontmatter is not currently supported"));
                    }
                    MetadataBlockKind::PlusesStyle => {
                        let mut table: Table = toml::from_str(&frontmatter).map_err(map_toml_error)?;
                        if let Some(toml::Value::String(template)) = table.remove("template") {
                            self.template = Some(template);
                        }
                        self.user_config = Some(table);
                    }
                }
            }
            ParsingMode::Heading { section_type, events, id, classes, attrs, .. } => {
                let mut name = String::new();
                for event in events {
                    match event {
                        Event::Text(text) => {
                            name.push_str(&text);
                        }
                        _ => {return Err(Error::custom(format!("Invalid event in section name: {:?}", event)))}
                    }
                }
                self.current_section = Some(Heading {
                    section_type,
                    name,
                    id,
                    classes,
                    attrs,
                })
            }
        }
        Ok(())
    }

    pub fn finalize(mut self, state: &State) -> Result<Context, Error> {
        self.finalize_section()?;
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
    pub fn handle(&mut self, node: Event<'a>, event_range: Range<usize>) -> Result<(), Error> {
        match node {
            Event::Start(Tag::MetadataBlock(style)) => {
                self.finalize_section()?;
                self.parsing_mode = ParsingMode::Frontmatter {
                    style,
                    events: Vec::new(),
                };
            }
            Event::End(TagEnd::MetadataBlock(end_style)) => {
                match &self.parsing_mode {
                    ParsingMode::Frontmatter{ style, events } => {
                        if !end_style.eq(style) {
                            return Err(Error::custom("Internal Error: Frontmatter style mismatch"));
                        }
                        self.finalize_section()?;
                        self.parsing_mode = ParsingMode::None;
                    }
                    _ => {
                        return Err(Error::custom("Internal Error: Frontmatter ending without start"))
                    }
                }
            }
            Event::Start(Tag::Heading { level, id, classes, attrs }) => {
                self.finalize_section()?;
                let mut new_attrs = Vec::new();
                let mut st = SectionType::Literal;
                for (attr, val) in attrs {
                    if "html".eq(attr.as_ref()) && val.is_none() {
                        st = SectionType::HTML;
                    } else {
                        new_attrs.push((attr, val));
                    }
                }
                self.parsing_mode = ParsingMode::Heading{
                    level,
                    id,
                    classes,
                    attrs: new_attrs,
                    events: Vec::new(),
                    section_type: st,
                }
            }
            Event::End(TagEnd::Heading(end_level)) => {
                match &self.parsing_mode {
                    ParsingMode::Heading{ level, .. } => {
                        if !end_level.eq(level) {
                            return Err(Error::custom("Internal Error: Heading level mismatch"));
                        }
                    self.finalize_section()?;
                        self.parsing_mode = ParsingMode::Text{ events: Vec::new(), range: None};
                    }
                    _ => {
                        return Err(Error::custom("Internal Error: Heading ending without start"))
                    }
                }
            }
            Event::SoftBreak => {}
            _ => {
                match &mut self.parsing_mode {
                    ParsingMode::Text { events, range } => {
                        events.push(node);
                        match range {
                            None => {
                                *range = Some(event_range);
                            }
                            Some(r) => {
                                r.end = event_range.end;
                            }
                        }
                    }
                    ParsingMode::Frontmatter { events, ..} | ParsingMode::Heading { events, ..} => {
                        events.push(node);
                    }
                    ParsingMode::None => {
                        return Err(Error::custom("Internal Error: Encountered data node with ParsingMode::None."))
                    }
                }
            }
        }
        Ok(())
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

    let renderer_state = get_state(state)?;
    let locked_state = lock_state(&renderer_state)?;
    let target_root = locked_state.target_path.clone();
    drop(locked_state);
    drop(renderer_state);

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

        let text = std::fs::read_to_string(&entry).map_err(map_io_error)?;
        let parser = pulldown_cmark::Parser::new_ext(&text, {
            let mut opt = pulldown_cmark::Options::empty();
            opt.insert(pulldown_cmark::Options::ENABLE_TABLES);
            opt.insert(pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION);
            opt.insert(pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES);
            opt.insert(pulldown_cmark::Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);
            opt
        });

        let mut context_builder = ContextBuilder::new(&text, &default_template);
        for (event, range) in parser.into_offset_iter() {
            context_builder.handle(event, range)?;
        }

        let context = context_builder.finalize(state)?;

        let template = state.env().get_template(context.template.as_str())?;
        results.push(template.render(&context)?);
    }

    Ok(Value::from_safe_string(results.join("\n")))
}