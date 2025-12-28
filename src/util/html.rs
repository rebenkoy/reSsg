pub use on_kuchikikiki::*;

mod on_kuchikikiki {
    use itertools::Itertools;
    use kuchikikiki::{parse_fragment, parse_html, Attribute, ExpandedName, NodeRef};
    use kuchikikiki::traits::TendrilSink;
    use markup5ever::{local_name, ns, LocalName, QualName};

    pub type HTML = NodeRef;

    pub fn parse_html_fragment(value: String) -> HTML {
        let dom = parse_fragment(
            QualName::new(None, ns!(html), local_name!("body")),
            vec![],
        ).one(value);
        dom.children().next().unwrap()
    }

    pub fn serialize_html_fragment(html: HTML) -> anyhow::Result<String> {
        let mut result = vec![];
        html5ever::serialize(&mut result, &html, Default::default())?;
        result.try_into().map_err(Into::into)
    }

    pub fn parse_html_document(value: String) -> HTML {
        parse_html().one(value)
    }

    pub fn serialize_html_document(html: HTML) -> anyhow::Result<String> {
        let mut result = vec![];
        html5ever::serialize(&mut result, &html, Default::default())?;
        result.try_into().map_err(Into::into)
    }

    pub fn serialize_u8_html_document(html: HTML) -> anyhow::Result<Vec<u8>> {
        let mut result = vec![];
        html5ever::serialize(&mut result, &html, Default::default())?;
        Ok(result)
    }

    pub fn insert_node(new_parent: &mut HTML, index: usize, child: HTML) -> () {
        let sibling = new_parent.children().get(index..=index).next();
        match sibling {
            Some(sibling) => sibling.insert_before(child),
            None => new_parent.append(child),
        };
    }

    pub fn create_element(element: String, attrs: Vec<(String, Option<String>)>) -> HTML {
        NodeRef::new_element(
            QualName::new(None, ns!(html), LocalName::from(element)),
            attrs.into_iter().map(|(n, v)| {
                (
                    ExpandedName {
                        ns: ns!(html),
                        local: LocalName::from(n),
                    },
                    Attribute {
                        prefix: None,
                        value: v.unwrap_or("".to_string()),
                    }
                )
            }).collect::<Vec<_>>()
        )
    }

    pub fn append_text<S: Into<String>>(element: &mut HTML, text: S) {
        element.append(NodeRef::new_text(text));
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_parse_html_fragment() {
            let s = r#"<div>Some <span id="s">text</span></div><div>Some <span id="s">other text</span></div>"#;
            let dom = parse_html_fragment(s.to_string());
            let ser = serialize_html_fragment(dom).unwrap();
            assert_eq!(ser, s);

            let s = r#"<div>Some <span id="s">text</span></div>"#;
            let dom = parse_html_fragment(s.to_string());
            let ser = serialize_html_fragment(dom).unwrap();
            assert_eq!(ser, s);


            let s = r#"<div>Text</div>"#;
            let dom = parse_html_fragment(s.to_string());
            let ser = serialize_html_fragment(dom).unwrap();
            assert_eq!(ser, s);
        }

        #[test]
        fn test_parse_html_document() {
            let s = r#"<html><head></head><body><div>Some <span id="s">text</span></div></body></html>"#;
            let dom = parse_html_document(s.to_string());
            let ser = serialize_html_document(dom).unwrap();
            assert_eq!(ser, s);
        }

        #[test]
        fn test_insert_node() {
            let s = r#"<div>Some <span id="s">text</span></div><div>Some <span id="s">other text</span></div>"#;
            let mut dom = parse_html_fragment(s.to_string());
            insert_node(&mut dom, 0, create_element("div".to_string(), vec![]));
            let ser = serialize_html_fragment(dom).unwrap();
            assert_eq!(ser, format!("<div></div>{}", s));
        }
    }
}

