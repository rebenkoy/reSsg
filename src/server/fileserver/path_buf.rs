use std::{
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use actix_utils::future::{ready, Ready};
use actix_web::{dev::Payload, FromRequest, HttpRequest, ResponseError};
use actix_web::http::StatusCode;
use derive_more::Display;

#[derive(Debug, PartialEq, Eq, Display)]
#[non_exhaustive]
pub enum UriSegmentError {
    /// Segment started with the wrapped invalid character.
    #[display("segment started with invalid character: ('{_0}')")]
    BadStart(char),

    /// Segment contained the wrapped invalid character.
    #[display("segment contained invalid character ('{_0}')")]
    BadChar(char),

    /// Segment ended with the wrapped invalid character.
    #[display("segment ended with invalid character: ('{_0}')")]
    BadEnd(char),

    /// Path is not a valid UTF-8 string after percent-decoding.
    #[display("path is not a valid UTF-8 string after percent-decoding")]
    NotValidUtf8,
}

impl ResponseError for UriSegmentError {
    /// Returns `400 Bad Request`.
    fn status_code(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PathBufWrap(PathBuf);

impl FromStr for PathBufWrap {
    type Err = UriSegmentError;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        Self::parse_path(path, false)
    }
}

impl PathBufWrap {
    /// Parse a path, giving the choice of allowing hidden files to be considered valid segments.
    ///
    /// Path traversal is guarded by this method.
    pub fn parse_path(path: &str, hidden_files: bool) -> Result<Self, UriSegmentError> {
        let mut buf = PathBuf::new();

        // equivalent to `path.split('/').count()`
        let mut segment_count = path.matches('/').count() + 1;

        // we can decode the whole path here (instead of per-segment decoding)
        // because we will reject `%2F` in paths using `segment_count`.
        let path = percent_encoding::percent_decode_str(path)
            .decode_utf8()
            .map_err(|_| UriSegmentError::NotValidUtf8)?;

        // disallow decoding `%2F` into `/`
        if segment_count != path.matches('/').count() + 1 {
            return Err(UriSegmentError::BadChar('/'));
        }

        for segment in path.split('/') {
            if segment == ".." {
                segment_count -= 1;
                buf.pop();
            } else if !hidden_files && segment.starts_with('.') {
                return Err(UriSegmentError::BadStart('.'));
            } else if segment.starts_with('*') {
                return Err(UriSegmentError::BadStart('*'));
            } else if segment.ends_with(':') {
                return Err(UriSegmentError::BadEnd(':'));
            } else if segment.ends_with('>') {
                return Err(UriSegmentError::BadEnd('>'));
            } else if segment.ends_with('<') {
                return Err(UriSegmentError::BadEnd('<'));
            } else if segment.is_empty() {
                segment_count -= 1;
                continue;
            } else if cfg!(windows) && segment.contains('\\') {
                return Err(UriSegmentError::BadChar('\\'));
            } else if cfg!(windows) && segment.contains(':') {
                return Err(UriSegmentError::BadChar(':'));
            } else {
                buf.push(segment)
            }
        }

        // make sure we agree with stdlib parser
        for (i, component) in buf.components().enumerate() {
            assert!(
                matches!(component, Component::Normal(_)),
                "component `{:?}` is not normal",
                component
            );
            assert!(i < segment_count);
        }

        Ok(PathBufWrap(buf))
    }
}

impl AsRef<Path> for PathBufWrap {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl FromRequest for PathBufWrap {
    type Error = UriSegmentError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(req.match_info().unprocessed().parse())
    }
}
