use kuchikikiki::NodeData;
use minijinja::{Error, Value};
use serde::de::Error as _;
use crate::util::html::{parse_html_fragment, serialize_html_fragment};

pub fn try_add_class(value: String, classes: String) -> Result<Value, Error> {

    let html = parse_html_fragment(value);

    for child in html.children() {
        match child.data() {
            NodeData::Element(el) => {
                let existing = el.attributes.borrow().get("class").unwrap_or("").to_string();
                el.attributes.borrow_mut().insert("class", format!("{} {}", existing, classes));
            }
            _ => {}
        }
    }
    Ok(Value::from_safe_string(serialize_html_fragment(html).map_err(|e| {Error::custom(e)})?))
}

