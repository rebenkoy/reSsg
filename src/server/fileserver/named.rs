use markup5ever::interface::tree_builder::TreeSink;
use std::{io, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use std::cell::RefCell;
use std::sync::Arc;
use actix_web::{
    body::{self, BoxBody},
    http::{
        header::{
            self, Charset, ContentDisposition, ContentEncoding, DispositionParam, DispositionType,
            ExtendedValue,
        },
        StatusCode,
    },
    HttpMessage, HttpRequest, HttpResponse,
};
use bitflags::bitflags;
use derive_more::{Deref, DerefMut};
use kuchikiki::traits::TendrilSink;
use mime::Mime;
use rsfs::{File, GenFS, Metadata};
use crate::server::fileserver::files::ContentMapper;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct Flags: u8 {
        const ETAG =                0b0000_0001;
        const LAST_MD =             0b0000_0010;
        const CONTENT_DISPOSITION = 0b0000_0100;
        const PREFER_UTF8 =         0b0000_1000;
    }
}

impl Default for Flags {
    fn default() -> Self {
        Flags::from_bits_truncate(0b0000_1111)
    }
}

fn equiv_utf8_text(ct: Mime) -> Mime {
    // use (roughly) order of file-type popularity for a web server

    if ct == mime::APPLICATION_JAVASCRIPT {
        return mime::APPLICATION_JAVASCRIPT_UTF_8;
    }

    if ct == mime::TEXT_HTML {
        return mime::TEXT_HTML_UTF_8;
    }

    if ct == mime::TEXT_CSS {
        return mime::TEXT_CSS_UTF_8;
    }

    if ct == mime::TEXT_PLAIN {
        return mime::TEXT_PLAIN_UTF_8;
    }

    if ct == mime::TEXT_CSV {
        return mime::TEXT_CSV_UTF_8;
    }

    if ct == mime::TEXT_TAB_SEPARATED_VALUES {
        return mime::TEXT_TAB_SEPARATED_VALUES_UTF_8;
    }

    ct
}

#[derive(Debug, Deref, DerefMut)]
pub struct NamedFile<F: File> {
    #[deref]
    #[deref_mut]
    file: F,
    path: PathBuf,
    modified: Option<SystemTime>,
    pub(crate) md: F::Metadata,
    pub(crate) flags: Flags,
    pub(crate) status_code: StatusCode,
    pub(crate) content_type: Mime,
    pub(crate) content_disposition: ContentDisposition,
    pub(crate) encoding: Option<ContentEncoding>,
}
impl<F: File> NamedFile<F> {
    pub fn from_file<P: AsRef<Path>>(file: F, path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Get the name of the file and use it to construct default Content-Type
        // and Content-Disposition values
        let (content_type, content_disposition) = {
            let filename = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Provided path has no filename",
                    ));
                }
            };

            let ct = mime_guess::from_path(&path).first_or_octet_stream();

            let disposition = match ct.type_() {
                mime::IMAGE | mime::TEXT | mime::AUDIO | mime::VIDEO => DispositionType::Inline,
                mime::APPLICATION => match ct.subtype() {
                    mime::JAVASCRIPT | mime::JSON => DispositionType::Inline,
                    name if name == "wasm" || name == "xhtml" => DispositionType::Inline,
                    _ => DispositionType::Attachment,
                },
                _ => DispositionType::Attachment,
            };

            // replace special characters in filenames which could occur on some filesystems
            let filename_s = filename
                .replace('\n', "%0A") // \n line break
                .replace('\x0B', "%0B") // \v vertical tab
                .replace('\x0C', "%0C") // \f form feed
                .replace('\r', "%0D"); // \r carriage return
            let mut parameters = vec![DispositionParam::Filename(filename_s)];

            if !filename.is_ascii() {
                parameters.push(DispositionParam::FilenameExt(ExtendedValue {
                    charset: Charset::Ext(String::from("UTF-8")),
                    language_tag: None,
                    value: filename.into_owned().into_bytes(),
                }))
            }

            let cd = ContentDisposition {
                disposition,
                parameters,
            };

            (ct, cd)
        };

        let md = {
            {
                file.metadata()?
            }
        };

        let modified = md.modified().ok();
        let encoding = None;

        Ok(NamedFile {
            path,
            file,
            content_type,
            content_disposition,
            md,
            modified,
            encoding,
            status_code: StatusCode::OK,
            flags: Flags::default(),
        })
    }

    pub fn open<P: AsRef<Path>>(fs: &impl GenFS<File = F>, path: P) -> io::Result<Self> {
        let file = fs.open_file(&path)?;
        Self::from_file(file, path)
    }

    pub async fn open_async<P: AsRef<Path>>(fs: &impl GenFS<File = F>, path: P) -> io::Result<Self> {
        let file = fs.open_file(&path)?;

        Self::from_file(file, path)
    }

    #[inline]
    pub fn file(&self) -> &F {
        &self.file
    }

    #[inline]
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    #[inline]
    pub fn modified(&self) -> Option<SystemTime> {
        self.modified
    }
    #[inline]
    pub fn metadata(&self) -> &F::Metadata {
        &self.md
    }
    #[inline]
    pub fn content_type(&self) -> &Mime {
        &self.content_type
    }
    #[inline]
    pub fn content_disposition(&self) -> &ContentDisposition {
        &self.content_disposition
    }
    #[inline]
    pub fn content_encoding(&self) -> Option<ContentEncoding> {
        self.encoding
    }
    #[deprecated(since = "0.7.0", note = "Prefer `Responder::customize()`.")]
    pub fn set_status_code(mut self, status: StatusCode) -> Self {
        self.status_code = status;
        self
    }
    #[inline]
    pub fn set_content_type(mut self, mime_type: Mime) -> Self {
        self.content_type = mime_type;
        self
    }
    #[inline]
    pub fn set_content_disposition(mut self, cd: ContentDisposition) -> Self {
        self.content_disposition = cd;
        self.flags.insert(Flags::CONTENT_DISPOSITION);
        self
    }
    #[inline]
    pub fn disable_content_disposition(mut self) -> Self {
        self.flags.remove(Flags::CONTENT_DISPOSITION);
        self
    }
    #[inline]
    pub fn set_content_encoding(mut self, enc: ContentEncoding) -> Self {
        self.encoding = Some(enc);
        self
    }
    #[inline]
    pub fn use_etag(mut self, value: bool) -> Self {
        self.flags.set(Flags::ETAG, value);
        self
    }
    #[inline]
    pub fn use_last_modified(mut self, value: bool) -> Self {
        self.flags.set(Flags::LAST_MD, value);
        self
    }
    #[inline]
    pub fn prefer_utf8(mut self, value: bool) -> Self {
        self.flags.set(Flags::PREFER_UTF8, value);
        self
    }
    pub(crate) fn etag(&self) -> Option<header::EntityTag> {
        self.modified.as_ref().map(|mtime| {
            let dur = mtime
                .duration_since(UNIX_EPOCH)
                .expect("modification time must be after epoch");

            header::EntityTag::new_strong(format!(
                "{:x}:{:x}:{:x}:{:x}",
                0,
                self.md.len(),
                dur.as_secs(),
                dur.subsec_nanos()
            ))
        })
    }

    pub(crate) fn last_modified(&self) -> Option<header::HttpDate> {
        self.modified.map(|mtime| mtime.into())
    }
    pub fn into_response(mut self, req: &HttpRequest, content_mappers: &Vec<Arc<dyn ContentMapper>>) -> HttpResponse<BoxBody> {
        if self.status_code != StatusCode::OK {
            let mut res = HttpResponse::build(self.status_code);

            let ct = if self.flags.contains(Flags::PREFER_UTF8) {
                equiv_utf8_text(self.content_type.clone())
            } else {
                self.content_type
            };

            res.insert_header((header::CONTENT_TYPE, ct.to_string()));

            if self.flags.contains(Flags::CONTENT_DISPOSITION) {
                res.insert_header((
                    header::CONTENT_DISPOSITION,
                    self.content_disposition.to_string(),
                ));
            }

            if let Some(current_encoding) = self.encoding {
                res.insert_header((header::CONTENT_ENCODING, current_encoding.as_str()));
            }

            let mut bytes= Vec::new();
            if self.file.read_to_end(&mut bytes).is_err() {
                return res.status(StatusCode::INTERNAL_SERVER_ERROR).finish()
            }

            return res.body(bytes);
        }

        let etag = if self.flags.contains(Flags::ETAG) {
            self.etag()
        } else {
            None
        };

        let last_modified = if self.flags.contains(Flags::LAST_MD) {
            self.last_modified()
        } else {
            None
        };

        // check preconditions
        let precondition_failed = if !any_match(etag.as_ref(), req) {
            true
        } else if let (Some(ref m), Some(header::IfUnmodifiedSince(ref since))) =
            (last_modified, req.get_header())
        {
            let t1: SystemTime = (*m).into();
            let t2: SystemTime = (*since).into();

            match (t1.duration_since(UNIX_EPOCH), t2.duration_since(UNIX_EPOCH)) {
                (Ok(t1), Ok(t2)) => t1.as_secs() > t2.as_secs(),
                _ => false,
            }
        } else {
            false
        };

        let mut res = HttpResponse::build(self.status_code);

        let ct = if self.flags.contains(Flags::PREFER_UTF8) {
            equiv_utf8_text(self.content_type.clone())
        } else {
            self.content_type
        };

        res.insert_header((header::CONTENT_TYPE, ct.to_string()));

        if self.flags.contains(Flags::CONTENT_DISPOSITION) {
            res.insert_header((
                header::CONTENT_DISPOSITION,
                self.content_disposition.to_string(),
            ));
        }

        if let Some(current_encoding) = self.encoding {
            res.insert_header((header::CONTENT_ENCODING, current_encoding.as_str()));
        }

        if let Some(lm) = last_modified {
            res.insert_header((header::LAST_MODIFIED, lm.to_string()));
        }

        if let Some(etag) = etag {
            res.insert_header((header::ETAG, etag.to_string()));
        }

        res.insert_header((header::ACCEPT_RANGES, "bytes"));

        if precondition_failed {
            return res.status(StatusCode::PRECONDITION_FAILED).finish();
        }
        let mut bytes = Vec::new();
        if let Err(e) = self.file.read_to_end(&mut bytes) {
            return res.status(StatusCode::INTERNAL_SERVER_ERROR).finish()
        }

        bytes = match String::try_from(bytes) {
            Ok(string) => {
                let mut html = kuchikiki::parse_html().one(string);
                for mapper in content_mappers {
                    html = mapper.map(html);
                }
                let mut buff = Vec::new();
                if html.serialize(&mut buff).is_err() {
                    return res.status(StatusCode::INTERNAL_SERVER_ERROR).finish();
                }
                buff
            }
            Err(e) => {
                e.into_bytes()
            }
        };

        res.body(bytes)
    }
}

fn any_match(etag: Option<&header::EntityTag>, req: &HttpRequest) -> bool {
    match req.get_header::<header::IfMatch>() {
        None | Some(header::IfMatch::Any) => true,

        Some(header::IfMatch::Items(ref items)) => {
            if let Some(some_etag) = etag {
                for item in items {
                    if item.strong_eq(some_etag) {
                        return true;
                    }
                }
            }

            false
        }
    }
}

fn none_match(etag: Option<&header::EntityTag>, req: &HttpRequest) -> bool {
    match req.get_header::<header::IfNoneMatch>() {
        Some(header::IfNoneMatch::Any) => false,

        Some(header::IfNoneMatch::Items(ref items)) => {
            if let Some(some_etag) = etag {
                for item in items {
                    if item.weak_eq(some_etag) {
                        return false;
                    }
                }
            }

            true
        }

        None => true,
    }
}

