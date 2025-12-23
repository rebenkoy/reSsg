use std::{
    cell::RefCell,
    fmt, io,
    path::{Path, PathBuf},
    rc::Rc,
};
use actix_service::{boxed, IntoServiceFactory, ServiceFactory, ServiceFactoryExt};
use actix_service::boxed::{BoxService, BoxServiceFactory};
use actix_web::{
    dev::{
        AppService, HttpServiceFactory, RequestHead, ResourceDef, ServiceRequest, ServiceResponse,
    },
    http::header::DispositionType,
    HttpRequest,
};
use futures_core::future::LocalBoxFuture;
use crate::{
    server::fileserver::named::{NamedFile, Flags},
};
use std::{ops::Deref};
use std::sync::{Arc, Mutex, RwLock};
use actix_web::{
    body::BoxBody,
    dev::{self, Service},
    error::Error,
    guard::Guard,
    http::{header, Method},
    HttpResponse,
};
use kuchikiki::NodeRef;
use rsfs::{File, FileType, GenFS, Metadata};
use crate::server::fileserver::path_buf::PathBufWrap;

pub trait ContentMapper {
    fn map(&self, req: &HttpRequest, path: &PathBuf, content: NodeRef) -> NodeRef;
}

pub struct Files<FS: GenFS> {
    directory: PathBuf,
    default: Rc<RefCell<Option<Rc<BoxServiceFactory<(), ServiceRequest, ServiceResponse, Error, ()>>>>>,
    file_flags: Flags,
    hidden_files: bool,
    fs: Arc<RwLock<FS>>,
    content_mappers: Vec<Arc<dyn ContentMapper>>
}

impl<FS: GenFS> fmt::Debug for Files<FS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Files")
    }
}

impl<FS: GenFS> Clone for Files<FS> {
    fn clone(&self) -> Self {
        Self {
            directory: self.directory.clone(),
            default: self.default.clone(),
            file_flags: self.file_flags,
            hidden_files: self.hidden_files,
            fs: self.fs.clone(),
            content_mappers: self.content_mappers.clone(),
        }
    }
}

impl<FS: GenFS> Files<FS> {
    pub fn new<T: Into<PathBuf>>(serve_from: T, fs: Arc<RwLock<FS>>) -> Self {
        let orig_dir = serve_from.into();
        let dir = match fs.read().unwrap().canonicalize(&orig_dir) {
            Ok(canon_dir) => canon_dir,
            Err(e) => {
                log::error!("Specified path is not a directory: {:?}, {e}", orig_dir);
                PathBuf::new()
            }
        };

        Self {
            directory: dir,
            default: Rc::new(RefCell::new(None)),
            file_flags: Flags::default(),
            hidden_files: false,
            fs,
            content_mappers: vec![],
        }
    }
    pub fn use_etag(mut self, value: bool) -> Self {
        self.file_flags.set(Flags::ETAG, value);
        self
    }

    pub fn use_last_modified(mut self, value: bool) -> Self {
        self.file_flags.set(Flags::LAST_MD, value);
        self
    }

    pub fn prefer_utf8(mut self, value: bool) -> Self {
        self.file_flags.set(Flags::PREFER_UTF8, value);
        self
    }

    pub fn disable_content_disposition(mut self) -> Self {
        self.file_flags.remove(Flags::CONTENT_DISPOSITION);
        self
    }

    pub fn content_mappers(mut self, mappers: Vec<Arc<dyn ContentMapper>>) -> Self {
        self.content_mappers = mappers;
        self
    }

    pub fn default_handler<F, U>(mut self, f: F) -> Self
    where
        F: IntoServiceFactory<U, ServiceRequest>,
        U: ServiceFactory<ServiceRequest, Config = (), Response = ServiceResponse, Error = Error>
        + 'static,
    {
        // create and configure default resource
        self.default = Rc::new(RefCell::new(Some(Rc::new(boxed::factory(
            f.into_factory().map_init_err(|_| ()),
        )))));

        self
    }

    pub fn use_hidden_files(mut self) -> Self {
        self.hidden_files = true;
        self
    }
}

impl<FS: GenFS + 'static> HttpServiceFactory for Files<FS> {
    fn register(mut self, config: &mut AppService) {
        if self.default.borrow().is_none() {
            *self.default.borrow_mut() = Some(config.default_service());
        }

        let rdef = if config.is_root() {
            ResourceDef::root_prefix("")
        } else {
            ResourceDef::prefix("")
        };

        config.register_service(rdef, None, self, None)
    }
}

impl<FS: GenFS + 'static> ServiceFactory<ServiceRequest> for Files<FS> {
    type Response = ServiceResponse;
    type Error = Error;
    type Config = ();
    type Service = FilesService<FS>;
    type InitError = ();
    type Future = LocalBoxFuture<'static, Result<Self::Service, Self::InitError>>;

    fn new_service(&self, _: ()) -> Self::Future {
        let mut inner = FilesServiceInner {
            directory: self.directory.clone(),
            default: None,
            file_flags: self.file_flags,
            hidden_files: self.hidden_files,
            content_mappers: self.content_mappers.clone(),
        };
        let fs = self.fs.clone();

        if let Some(ref default) = *self.default.borrow() {
            let fut = default.new_service(());
            Box::pin(async {
                match fut.await {
                    Ok(default) => {
                        inner.default = Some(default);
                        Ok(FilesService{
                            s: Rc::new(inner),
                            fs,
                        })
                    }
                    Err(_) => Err(()),
                }
            })
        } else {
            Box::pin(async move {
                Ok(FilesService {
                    s: Rc::new(inner),
                    fs,
                })
            })
        }
    }
}

#[derive(Clone)]
pub struct FilesService<FS: GenFS>{
    s: Rc<FilesServiceInner>,
    fs: Arc<RwLock<FS>>,
}

impl<FS: GenFS> Deref for FilesService<FS> {
    type Target = FilesServiceInner;

    fn deref(&self) -> &Self::Target {
        &self.s
    }
}

pub struct FilesServiceInner {
    pub(crate) directory: PathBuf,
    pub(crate) default: Option<BoxService<ServiceRequest, ServiceResponse, Error>>,
    pub(crate) file_flags: Flags,
    pub(crate) hidden_files: bool,
    pub(crate) content_mappers: Vec<Arc<dyn ContentMapper>>,
}

impl fmt::Debug for FilesServiceInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FilesServiceInner")
    }
}

impl<FS: GenFS> FilesService<FS> {}

impl<FS: GenFS> fmt::Debug for FilesService<FS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FilesService")
    }
}

impl<FS: GenFS + 'static> Service<ServiceRequest> for FilesService<FS> {
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    dev::always_ready!();

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let is_method_valid = matches!(*req.method(), Method::HEAD | Method::GET);

        let Self { s, fs } = self;
        let s = s.clone();
        let fs = fs.clone();

        Box::pin(async move {
            if !is_method_valid {
                return Ok(req.into_response(
                    HttpResponse::MethodNotAllowed()
                        .insert_header(header::ContentType(mime::TEXT_PLAIN_UTF_8))
                        .body("Request did not meet this resource's requirements."),
                ));
            }

            let path_on_disk =
                match PathBufWrap::parse_path(req.match_info().unprocessed(), s.hidden_files) {
                    Ok(item) => item,
                    Err(err) => return Ok(req.error_response(err)),
                };

            let guard = fs.read().expect("Failed to lock");

            // full file path
            let mut path = s.directory.join(&path_on_disk);
            if let Err(err) = guard.canonicalize(&path) {
                return s.handle_err(err, req).await;
            }
            let meta = match guard.metadata(&path) {
                Ok(meta) => meta.file_type(),
                Err(err) => {
                    return s.handle_err(err, req).await;
                }
            };

            if meta.is_dir() {
                path.push("index.html");
            }
            match NamedFile::open_async(&*guard, &path).await {
                Ok(named_file) => Ok(s.serve_named_file(req, named_file)),
                Err(err) => s.handle_err(err, req).await,
            }
        })
    }
}

impl FilesServiceInner {

    async fn handle_err(
        &self,
        err: io::Error,
        req: ServiceRequest,
    ) -> Result<ServiceResponse, Error> {
        log::debug!("error handling {}: {}", req.path(), err);

        if let Some(ref default) = self.default {
            default.call(req).await
        } else {
            Ok(req.error_response(err))
        }
    }

    fn serve_named_file<F: File>(&self, req: ServiceRequest, mut named_file: NamedFile<F>) -> ServiceResponse {
        named_file.flags = self.file_flags;

        let (req, _) = req.into_parts();
        let res = named_file
            .into_response(&req, &self.content_mappers);
        ServiceResponse::new(req, res)
    }
}
