use std::fmt::format;
use std::path::{Path, PathBuf, StripPrefixError};
use actix_web::HttpRequest;
use markup5ever::{local_name, ns, namespace_url, QualName};
use markup5ever_rcdom::RcDom;
use mime::HTML;
use crate::config::{reSsgConfig, ControlConfig, ServerConfig};
use crate::server::fileserver::files::ContentMapper;
use crate::util::html::{append_text, create_element, HTML};

static SCRIPT: &str = r#"
function ______setup_autoreload() {
    let port = window.location.port;
    let hostname = window.location.hostname;
    let prefix;

    let protocol;
    switch (window.location.protocol) {
        case "http:":
            protocol = "ws";
            break;
        case "https:":
            protocol = "wss";
            break;
        default:
            break;
    }
    function build_url(protocol, hostname, port, prefix) {
        if (prefix === undefined) {
            return `${protocol}://${hostname}:${port}/ws`
        } else {
            return `${protocol}://${hostname}:${port}/${prefix}/ws`
        }
    }
    let url = build_url(protocol, hostname, port, prefix);
    console.log(`Will connect to livereload socket at ${url}`);

    const socket = new WebSocket(url)
    socket.addEventListener('open', (event) => {
      console.log('Connected to the WebSocket server');
      socket.send('Hello Server!'); // You can send data once open
    });
    socket.addEventListener('message', (event) => {
      console.log('Message from server:', event.data);
      // If the server sends JSON, you can parse it:
      const data = JSON.parse(event.data);
      if (data.kind === "reload") {
        window.location.reload();
      }
    });
    socket.addEventListener('error', (event) => {
      console.error('WebSocket error observed:', event);
    });
}
______setup_autoreload();
"#;



pub struct LivereloadInjector {
    output: String,
    static_home: String,
    js: Option<String>,
}

impl LivereloadInjector {
    pub fn new(config: &reSsgConfig) -> Self {
        let js = match &config.server.control {
            ControlConfig::None => {None}
            ControlConfig::Endpoint(endpoint) => {
                let mut js = SCRIPT.to_string();
                if endpoint.interface != config.server.output.interface {
                    js = js.replace("let hostname = window.location.hostname;", format!(r#"let hostname = "{}";"#, endpoint.interface).as_str());
                }
                js = js.replace("let port = window.location.port;", format!(r#"let port = "{}";"#, endpoint.interface).as_str());
                Some(js)
            }
            ControlConfig::Prefix(prefix) => {
                Some(
                    SCRIPT.replace("let prefix;", format!(r#"let prefix = "{}";"#, prefix.trim_start_matches("/").trim_end_matches("/")).as_str())
                )
            }
        };
        Self {
            output: config.build.output.clone(),
            static_home: config.build.static_output.clone(),
            js,

        }
    }
}

impl ContentMapper for LivereloadInjector {
    fn map(&self, req: &HttpRequest, path: &PathBuf, content: HTML) -> HTML {
        match path.strip_prefix(format!("/{}", &self.output)) {
            Ok(path) => {
                if path.starts_with(&self.static_home) {
                    return content;
                }
            }
            Err(_) => {
                return content;
            }
        }
        let js = match &self.js {
            Some(js_src) => js_src,
            None => {return content}
        };
        let html = match content.select_first("html") {
            Ok(html) => html,
            Err(_) => return content,
        };
        let head = match html.as_node().select_first("head") {
            Ok(head) => head.as_node().clone(),
            Err(_) => {
                let new_head = create_element("head".to_string(), vec![]);
                html.as_node().prepend(new_head.clone());
                new_head
            },
        };
        let mut script = create_element("script".to_string(), vec![]);
        append_text(&mut script, js);
        head.prepend(script);

        content
    }
}