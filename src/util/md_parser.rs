use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, Range};
use std::sync::Arc;
use minijinja::{render, Environment, Error, State, Value};
use minijinja::value::{Enumerator, Object, ObjectExt, ObjectRepr};
use pulldown_cmark::{CowStr, Event, HeadingLevel, MetadataBlockKind, Tag, TagEnd};
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use toml::Table;
use crate::util::error_mappers::map_toml_error;

#[derive(Debug, Clone)]
pub struct MdValue {
    list: Vec<MdValueMap>,
}
#[derive(Debug, Clone)]
pub struct MdValueMap {
    lit: Option<String>,
    attrs: HashMap<String, MdValue>,
}

impl Object for MdValue {
    fn is_true(self: &Arc<Self>) -> bool {
        self.list.len() > 0
    }
    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Seq(self.list.len())
    }
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        Some(Value::from_object(self.list.get(key.as_usize()?)?.clone()))
    }

    fn render(self: &Arc<Self>, f: &mut Formatter<'_>) -> std::fmt::Result
    where
        Self: Sized + 'static,
    {
        let mut dbg = f.debug_list();
        for el in self.list.iter() {
            dbg.entry(el);
        }
        dbg.finish()
    }
}

impl Object for MdValueMap {
    fn is_true(self: &Arc<Self>) -> bool {
        self.lit.is_some() || self.attrs.len() > 0
    }
    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Values(self.attrs.keys().map(|k| Value::from_safe_string(k.clone())).collect())
    }
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        match self.lit {
            None => {
                ObjectRepr::Map
            }
            Some(_) => {
                ObjectRepr::Plain
            }
        }
    }
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        match key.as_usize() {
            Some(0) => {
                Some(Value::from_object(self.as_ref().clone()))
            }
            _ => {
                self.attrs.get(key.as_str()?).map(|r| {
                    if r.list.len() != 1 {
                        Value::from_object(r.clone())
                    } else {
                        Value::from_object(r.list[0].clone())
                    }

                })
            }
        }
    }

    fn render(self: &Arc<Self>, f: &mut Formatter<'_>) -> std::fmt::Result
    where
        Self: Sized + 'static,
    {
        match &self.lit {
            Some(lit) => {
                if lit.contains("<p>") {
                    return write!(f, "{}", lit);
                }
                write!(f, "{}", lit)
            },
            None => {
                let mut dbg = f.debug_map();
                for (key, value) in self.attrs.iter() {
                    dbg.entry(&key, &value);
                }
                dbg.finish()
            }
        }
    }
}
struct MdValueIndex {
    name: String,
    idx: usize,
}
struct MdValueDeepIndex {
    rec: Vec<MdValueIndex>,
    idx: usize,
}

impl MdValueDeepIndex {
    fn as_ref(&self) -> MdValueDeepIndexRef {
        MdValueDeepIndexRef {
            rec: &self.rec,
            idx: self.idx,
        }
    }
}

struct MdValueDeepIndexRef<'a> {
    rec: &'a [MdValueIndex],
    idx: usize,
}

impl MdValueDeepIndexRef<'_> {
    fn is_empty(&self) -> bool {
        self.rec.is_empty()
    }
    fn slice(&self, idx: usize)  -> Self {
        Self {
            rec: &self.rec[idx..],
            idx: self.idx,
        }
    }
}

struct MdValueCursor {
    val: MdValue,
    path: MdValueDeepIndex,
}

impl MdValueCursor {
    fn finish(self) -> MdValue {
        self.val
    }
    fn new() -> Self {
        Self {
            val: MdValue::map(None, HashMap::new()),
            path: MdValueDeepIndex {
                rec: vec![],
                idx: 0,
            }
        }
    }

    fn step_out(&mut self) {
        match self.path.rec.pop() {
            None => {}
            Some(MdValueIndex { idx, name }) => {
                self.path.idx = idx;
            }
        }
    }
    fn depth(&self) -> usize {
        self.path.rec.len()
    }
    fn truncate_path(&mut self, new_depth: usize) {
        while self.depth() > new_depth {
            self.step_out();
        }
    }
    fn make_child(&mut self, key: String) -> Result<(), Error> {
        let idx = self.path.idx;
        let mnode = self.val.get_map_mut(self.path.as_ref())?;
        self.path.rec.push(MdValueIndex { idx, name: key.clone() });
        let new_idx = match mnode.attrs.get_mut(&key) {
            None => {
                mnode.attrs.insert(key, MdValue::map(None, HashMap::new()));
                0
            }
            Some(l) => {
                l.list.push(MdValueMap { lit: None, attrs: HashMap::new() });
                l.list.len() - 1
            }
        };
        self.path.idx = new_idx;
        Ok(())
    }
    fn set(&mut self, value: String) -> Result<(), Error> {
        self.val.get_map_mut(self.path.as_ref())?.lit = Some(value);
        Ok(())
    }
}

impl MdValue {
    pub fn new(s: String) -> Self {
        Self::map(Some(s), HashMap::new())
    }
    pub fn empty() -> Self {
        Self{list: vec![]}
    }
    pub fn list(vec: Vec<String>) -> Self {
        Self{list: vec.into_iter().map(|v| MdValueMap{lit: Some(v), attrs: HashMap::new()}).collect()}
    }
    pub fn new_with_attrs(lit: String, attrs: HashMap<String, MdValue>) -> Self {
        Self::map(Some(lit), attrs)
    }
    pub fn just_attrs(attrs: HashMap<String, MdValue>) -> Self {
        Self::map(None, attrs)
    }
    pub fn map(lit: Option<String>, attrs: HashMap<String, MdValue>) -> Self {
        Self{list: vec![MdValueMap{lit, attrs}]}
    }

    fn get_map_mut(&mut self, path: MdValueDeepIndexRef) -> Result<&mut MdValueMap, Error> {
        if path.is_empty() {
            self.list.get_mut(path.idx).ok_or(Error::custom(format!("Index out of bounds: {:?}", path.idx)))
        } else {
            let MdValueIndex{ idx, name } = &path.rec[0];
            match self.list.get_mut(*idx) {
                None => Err(Error::custom(format!("Index out of bounds: {:?}", idx))),
                Some(m) => {
                    match m.attrs.get_mut(name) {
                        None => Err(Error::custom(format!("Name does not exist: {:?}", name))),
                        Some(v) => {
                            Ok(v.get_map_mut(path.slice(1))?)
                        }
                    }
                }
            }
        }
    }

    fn finalize(self, env: &Environment) -> Result<Self, Error> {
        let mut out = Self::empty();
        for m in self.list {
            let lit_res = match m.lit {
                None => None,
                Some(l) => Some(env.render_str(l.as_str(), ())?)
            };
            let mut attrs_res = HashMap::new();
            for (k, v) in m.attrs {
                attrs_res.insert(k, v.finalize(env)?);
            }
            out.list.push(MdValueMap{lit: lit_res, attrs: attrs_res});
        }
        Ok(out)
    }
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
    Text(ParsingModes::Text<'a>),
    Frontmatter(ParsingModes::Frontmatter<'a>),
    Heading(ParsingModes::Heading<'a>),
}

impl<'a> ParsingMode<'a> {
    pub fn text(events: Vec<Event<'a>>, range: Option<Range<usize>>) -> Self {
        Self::Text(ParsingModes::Text{events, range})
    }
    pub fn frontmatter(events: Vec<Event<'a>>, style: MetadataBlockKind) -> Self {
        Self::Frontmatter(ParsingModes::Frontmatter{events, style})
    }
    pub fn ensure_frontmatter(&self) -> Result<&ParsingModes::Frontmatter, Error> {
        match self {
            ParsingMode::Frontmatter(f) => Ok(f),
            _ => Err(Error::custom("Parsing state is not set to Frontmatter")),
        }
    }
    pub fn heading(section_type: SectionType, events: Vec<Event<'a>>, level: HeadingLevel) -> Self {
        Self::Heading(ParsingModes::Heading{section_type, events, level})
    }
    pub fn ensure_heading(&self) -> Result<&ParsingModes::Heading, Error> {
        match self {
            ParsingMode::Heading(h) => Ok(h),
            _ => Err(Error::custom("Parsing state is not set to Heading")),
        }
    }
}
mod ParsingModes {
    use super::*;
    #[derive(Debug)]
    pub struct Text<'a> {
        pub events: Vec<Event<'a>>,
        pub range: Option<Range<usize>>,
    }
    #[derive(Debug)]
    pub struct Frontmatter<'a> {
        pub style: MetadataBlockKind,
        pub events: Vec<Event<'a>>,
    }
    impl<'a> Frontmatter<'a> {
        pub fn ensure_style(&self, other: MetadataBlockKind) -> Result<(), Error> {
            if self.style != other {
                Err(Error::custom("Frontmatter style mismatch"))
            } else {
                Ok(())
            }
        }
    }
    #[derive(Debug)]
    pub struct Heading<'a> {
        pub section_type: SectionType,
        pub level: HeadingLevel,
        pub events: Vec<Event<'a>>,
    }
    impl<'a> Heading<'a> {
        pub fn ensure_level(&self, other: HeadingLevel) -> Result<(), Error> {
            if self.level != other {
                Err(Error::custom("Heading level mismatch"))
            } else {
                Ok(())
            }
        }
    }
}

struct HeadingData {
    name: String,
    section_type: SectionType,
}

struct ParsingContext {
    frontmatter: Option<Table>,
    heading_stack: Vec<HeadingData>,
    cursor: MdValueCursor,
}
impl Default for ParsingContext {
    fn default() -> Self {
        Self {
            frontmatter: None,
            heading_stack: vec![],
            cursor: MdValueCursor::new(),
        }
    }
}

struct ContextBuilder<'a> {
    source: &'a String,
    context: ParsingContext,
    template: Option<String>,
    parsing_mode: ParsingMode<'a>,
    // user_config: Option<Table>,
    // data: HashMap<String, MdValue>,
    // meta: HashMap<String, HeadingData>,
    // stack: Vec<Heading<'a>>,
}

fn heading_to_depth(h: HeadingLevel) -> usize {
    match h {
        HeadingLevel::H1 => 0,
        HeadingLevel::H2 => 1,
        HeadingLevel::H3 => 2,
        HeadingLevel::H4 => 3,
        HeadingLevel::H5 => 4,
        HeadingLevel::H6 => 5,
    }
}

impl<'a> ContextBuilder<'a> {
    fn new(source: &'a String, default_template: &Option<String>) -> Self {
        Self {
            source,
            template: default_template.clone(),
            context: ParsingContext::default(),
            parsing_mode: ParsingMode::None,
        }
    }

    fn finalize_frontmatter(&mut self, fm: ParsingModes::Frontmatter) -> Result<(), Error> {
        let ParsingModes::Frontmatter {events, style} = fm;

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
                self.context.frontmatter = Some(table);
            }
        }
        Ok(())
    }

    fn finalize_text(&mut self, text: ParsingModes::Text) -> Result<(), Error> {
        let ParsingModes::Text{events, range} = text;
        let Some(range) = range else {
            if !events.is_empty() {
                return Err(Error::custom("Internal Error: Empty range in text section"));
            }
            return Ok(());
        };
        if self.context.heading_stack.is_empty() {
            return Err(Error::custom("No section set for entry"))
        }

        let data = match self.context.heading_stack.last().unwrap().section_type {
            SectionType::Literal => {
                self.source[range].to_string()
            }
            SectionType::HTML => {
                let mut html = String::new();
                pulldown_cmark::html::write_html_fmt(&mut html, events.into_iter())?;
                html
            }
        };
        self.context.cursor.set(data)?;
        Ok(())
    }

    fn finalize_heading(&mut self, h: ParsingModes::Heading) -> Result<(), Error> {
        let ParsingModes::Heading{section_type, events, level} = h;
        let target_depth = heading_to_depth(level);
        if self.context.heading_stack.len() < target_depth {
            return Err(Error::custom("Heading stack underflow"));
        }
        self.context.heading_stack.truncate(target_depth);
        self.context.cursor.truncate_path(target_depth);

        let mut name = String::new();
        for event in events {
            match event {
                Event::Text(text) => {
                    name.push_str(&text);
                }
                _ => {return Err(Error::custom(format!("Invalid event in section name: {:?}", event)))}
            }
        }
        self.context.cursor.make_child(name.clone())?;
        self.context.heading_stack.push(HeadingData{name, section_type});
        Ok(())
    }

    fn finalize_section(&mut self) -> Result<(), Error> {
        let mut mode = ParsingMode::None;
        std::mem::swap(&mut self.parsing_mode, &mut mode);
        match mode {
            ParsingMode::None => {}
            ParsingMode::Frontmatter(fm) => self.finalize_frontmatter(fm)?,
            ParsingMode::Text(t) => self.finalize_text(t)?,
            ParsingMode::Heading(h) => self.finalize_heading(h)?,
        }
        Ok(())
    }

    fn finalize(mut self, env: &Environment) -> Result<Context, Error> {
        self.finalize_section()?;
        let data = self.context.cursor.finish().finalize(env)?.list.pop().unwrap();
        Ok(Context {
            template: self.template.ok_or(Error::custom("No template specified."))?,
            config: self.context.frontmatter.unwrap_or(Default::default()),
            data: Value::from_object(data.clone()),
            ser_data: Some(data),
        })
    }

    fn handle_start_metadata(&mut self, style: MetadataBlockKind) -> Result<(), Error> {
        self.finalize_section()?;

        self.parsing_mode = ParsingMode::frontmatter(
            Vec::new(),
            style,
        );
        Ok(())
    }

    fn handle_end_metadata(&mut self, end_style: MetadataBlockKind) -> Result<(), Error> {
        let fm = self.parsing_mode.ensure_frontmatter()?;
        fm.ensure_style(end_style)?;

        self.finalize_section()?;

        self.parsing_mode = ParsingMode::None;
        Ok(())
    }

    fn handle_start_heading(&mut self, level: HeadingLevel, id: Option<CowStr<'a>>, classes: Vec<CowStr<'a>>, attrs: Vec<(CowStr<'a>, Option<CowStr<'a>>)>) -> Result<(), Error> {
        self.finalize_section()?;

        let mut st = SectionType::Literal;
        for (attr, val) in attrs {
            if "html".eq(attr.as_ref()) && val.is_none() {
                st = SectionType::HTML;
            }
        }
        self.parsing_mode = ParsingMode::heading(
            st,
            Vec::new(),
            level,
        );
        Ok(())
    }

    fn handle_end_heading(&mut self, end_level: HeadingLevel) -> Result<(), Error> {
        let head = self.parsing_mode.ensure_heading()?;
        head.ensure_level(end_level)?;

        self.finalize_section()?;

        self.parsing_mode = ParsingMode::text(Vec::new(), None);
        Ok(())
    }

    fn handle_other(&mut self, node: Event<'a>, event_range: Range<usize>) -> Result<(), Error> {
        match &mut self.parsing_mode {
            ParsingMode::Text(ParsingModes::Text { events, range }) => {
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
            ParsingMode::Frontmatter(ParsingModes::Frontmatter { events, ..}) | ParsingMode::Heading(ParsingModes::Heading { events, ..}) => {
                events.push(node);
            }
            ParsingMode::None => {
                return Err(Error::custom("Internal Error: Encountered data node with ParsingMode::None."))
            }
        }
        Ok(())
    }

    fn handle(&mut self, node: Event<'a>, event_range: Range<usize>) -> Result<(), Error> {
        match node {
            Event::Start(Tag::MetadataBlock(style)) => self.handle_start_metadata(style)?,
            Event::End(TagEnd::MetadataBlock(end_style)) => self.handle_end_metadata(end_style)?,
            Event::Start(Tag::Heading { level, id, classes, attrs }) => self.handle_start_heading(level, id, classes, attrs)?,
            Event::End(TagEnd::Heading(end_level)) => self.handle_end_heading(end_level)?,
            Event::SoftBreak => {}
            _ => self.handle_other(node, event_range)?,
        }
        Ok(())
    }

}

#[derive(Debug, Serialize, Deserialize)]
pub struct Context {
    pub template: String,
    pub config: Table,
    pub data: Value,
    #[serde(skip)]
    pub ser_data: Option<MdValueMap>,
}


impl Context {
    pub fn new(text: &String, default_template: &Option<String>, env: &Environment) -> Result<Self, Error> {
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
        Ok(context_builder.finalize(env)?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn foo() {
        let mut env = Environment::new();
        env.add_template("hello", r#"
data.data[0].more[0].nested[0]: {{ data.data[0].more[0].nested[0] }}
data.data[0].more[0].nested: {{ data.data[0].more[0].nested }}
data.data[0].more[1]: {{ data.data[0].more[1] }}
data.data[1]: {{ data.data[1] }}
data.struct: {{ data.struct }}
data.struct.field: {{ data.struct.field }}
data.struct.field.nested: {{ data.struct.field.nested }}
data.struct.another: {{ data.struct.another }}
"#).unwrap();
        let res = Context::new(&r#"+++
a = "string a"
[b]
c = "string c"
+++
# data
lalala
## more
### nested
text
## more
foo
# data
bar
# struct
foo
## field
bar
### nested
baz
## another
bunny
"#.to_string(), &Some("hello".to_string()), &env);
        // println!("{:#?}", res);
        let res = res.unwrap();
        let template = env.get_template(res.template.as_str()).unwrap();
        let render = template.render(res).unwrap();
        println!("{}", render);
        assert_eq!(render, r#"
data.data[0].more[0].nested[0]: text
data.data[0].more[0].nested: text
data.data[0].more[1]: foo
data.data[1]: bar
data.struct: foo
data.struct.field: bar
data.struct.field.nested: baz
data.struct.another: bunny"#)
    }
}