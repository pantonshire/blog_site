use std::{
    borrow::Cow,
    fmt::{self, Write},
};

use axum::{
    body::{Bytes, Full},
    http::{header::{self, HeaderValue}, StatusCode},
    response::{IntoResponse, Response},
};
use maud::{html, Markup, Render, Escaper, DOCTYPE};

#[derive(Debug)]
pub(super) enum Error {
    Internal,
    PostNotFound,
    StaticResourceNotFound,
    RouteNotFound,
}

impl Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Error::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            Error::PostNotFound => StatusCode::NOT_FOUND,
            Error::StaticResourceNotFound => StatusCode::NOT_FOUND,
            Error::RouteNotFound => StatusCode::NOT_FOUND,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status_code = self.status_code();
        
        // Create a string buffer containing the full error text, e.g. "404 Not Found".
        let status_text = {
            let status_code_str = status_code.as_str();
            let reason = status_code.canonical_reason();

            // Allocate a buffer with enough capacity to store the full error text.
            let mut buf = String::with_capacity(
                status_code_str.len() + reason.map(|reason| reason.len() + 1).unwrap_or(0));
            
            // Push the numerical code string first, then a space, then the error reason string.
            buf.push_str(status_code_str);
            if let Some(reason) = reason {
                buf.push(' ');
                buf.push_str(reason);
            }

            buf
        };

        Html::new()
            .with_status(status_code)
            .with_body(html! {
                p { (status_text) }
            })
            .with_title_owned(status_text)
            .into_response()
    }
}

pub(super) struct Html {
    status: StatusCode,
    title: Cow<'static, str>,
    head: Option<Markup>,
    body: Option<Markup>,
    crawler_hints: CrawlerHints,
}

impl Html {
    pub(super) fn new() -> Self {
        Self {
            status: StatusCode::OK,
            title: Cow::Borrowed("untitled"),
            head: None,
            body: None,
            crawler_hints: CrawlerHints::restrictive(),
        }
    }

    pub(super) fn with_status(self, status: StatusCode) -> Self {
        Self { status, ..self }
    }

    pub(super) fn with_title(self, title: Cow<'static, str>) -> Self {
        Self { title, ..self }
    }

    pub(super) fn with_title_static(self, title: &'static str) -> Self {
        self.with_title(Cow::Borrowed(title))
    }

    pub(super) fn with_title_owned(self, title: String) -> Self {
        self.with_title(Cow::Owned(title))
    }

    pub(super) fn with_head(self, head: Markup) -> Self {
        Self { head: Some(head), ..self }
    }

    pub(super) fn with_body(self, body: Markup) -> Self {
        Self { body: Some(body), ..self }
    }

    pub(super) fn with_crawler_hints(self, crawler_hints: CrawlerHints) -> Self {
        Self { crawler_hints, ..self }
    }

    pub(super) fn with_crawler_permissive(self) -> Self {
        self.with_crawler_hints(CrawlerHints::permissive())
    }
}

impl Default for Html {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoResponse for Html {
    fn into_response(self) -> Response {
        let html_doc = html! {
            (DOCTYPE)
            html {
                head {
                    meta charset="utf-8";
                    
                    meta name="robots" content=(self.crawler_hints);
                    meta name="viewport" content="width=device-width, initial-scale=1";

                    title { (self.title) }
                    
                    link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png";
                    link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png";
                    link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png";
                    link rel="manifest" href="/site.webmanifest";
                    
                    @if let Some(head) = self.head {
                        (head)
                    }
                }
                body {
                    @if let Some(body) = self.body {
                        (body)
                    }
                }
            }
        };

        (self.status, axum::response::Html(html_doc.into_string()))
            .into_response()
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CrawlerHints {
    index: bool,
    follow: bool,
    archive: bool,
    snippet: bool,
    image_index: bool,
}

impl CrawlerHints {
    pub(super) const fn restrictive() -> Self {
        Self {
            index: false,
            follow: false,
            archive: false,
            snippet: false,
            image_index: false,
        }
    }

    pub(super) const fn permissive() -> Self {
        Self {
            index: true,
            follow: true,
            archive: true,
            snippet: true,
            image_index: true,
        }
    }

    fn index_str(self) -> &'static str {
        if self.index {
            "index"
        } else {
            "noindex"
        }
    }

    fn follow_str(self) -> &'static str {
        if self.follow {
            "follow"
        } else {
            "nofollow"
        }
    }

    fn archive_strs(self) -> Option<[&'static str; 2]> {
        if self.archive {
            None
        } else {
            Some(["noarchive", "nocache"])
        }
    }

    fn snippet_str(self) -> Option<&'static str> {
        if self.snippet {
            None
        } else {
            Some("nosnippet")
        }
    }

    fn image_index_str(self) -> Option<&'static str> {
        if self.image_index {
            None
        } else {
            Some("noimageindex")
        }
    }

    fn write_meta_list_to<W: Write>(self, mut buf: W) -> fmt::Result {
        write!(buf, "{},{}", self.index_str(), self.follow_str())?;
        if let Some([archive_str, cache_str]) = self.archive_strs() {
            write!(buf, ",{},{}", archive_str, cache_str)?;
        }
        if let Some(snippet_str) = self.snippet_str() {
            write!(buf, ",{}", snippet_str)?;
        }
        if let Some(image_index_str) = self.image_index_str() {
            write!(buf, ",{}", image_index_str)?;
        }
        Ok(())
    }
}

impl Render for CrawlerHints {
    fn render_to(&self, buf: &mut String) {
        let escaper = Escaper::new(buf);
        let _result = self.write_meta_list_to(escaper);
    }
}

pub(super) struct Rss<T>(pub T);

impl<T: Into<Full<Bytes>>> IntoResponse for Rss<T> {
    fn into_response(self) -> Response {
        let headers = [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/rss+xml")),
        ];

        (headers, self.0.into())
            .into_response()
    }
}

pub(super) struct Atom<T>(pub T);

impl<T: Into<Full<Bytes>>> IntoResponse for Atom<T> {
    fn into_response(self) -> Response {
        let headers = [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/atom+xml")),
        ];

        (headers, self.0.into())
            .into_response()
    }
}
