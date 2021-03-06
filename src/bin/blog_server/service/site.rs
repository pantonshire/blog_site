use std::sync::Arc;

use axum::{
    handler::Handler,
    http::Uri,
    extract::Extension,
    Router,
    routing::get,
};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::Context;

use super::{
    atom,
    contact,
    index,
    post,
    posts_list,
    response::Error,
    rss,
    static_content,
};

pub(crate) fn service(context: Arc<Context>) -> Router {
    Router::new()
        .route("/", get(index::handle))
        .route("/contact", get(contact::handle))
        .route("/articles", get(posts_list::handle))
        .route("/rss.xml", get(rss::handle))
        .route("/atom.xml", get(atom::handle))
        .route("/articles/:post_id", get(post::handle))
        .route("/robots.txt", static_content::file_service(&context.config().content.robots_path, None))
        .route("/favicon.ico", static_content::file_service(&context.config().content.favicon_dir.join("favicon.ico"), None))
        .route("/favicon-16x16.png", static_content::file_service(&context.config().content.favicon_dir.join("favicon-16x16.png"), None))
        .route("/favicon-32x32.png", static_content::file_service(&context.config().content.favicon_dir.join("favicon-32x32.png"), None))
        .route("/apple-touch-icon.png", static_content::file_service(&context.config().content.favicon_dir.join("apple-touch-icon.png"), None))
        .route("/android-chrome-192x192.png", static_content::file_service(&context.config().content.favicon_dir.join("android-chrome-192x192.png"), None))
        .route("/android-chrome-512x512.png", static_content::file_service(&context.config().content.favicon_dir.join("android-chrome-512x512.png"), None))
        .route("/site.webmanifest", static_content::file_service(&context.config().content.favicon_dir.join("site.webmanifest"), None))
        .nest("/static", static_content::dir_service(&context.config().content.static_dir))
        .nest("/article_media", static_content::dir_service(&context.config().content.post_media_dir))
        .fallback(handle_fallback.into_service())
        .layer(ConcurrencyLimitLayer::new(context.config().concurrency_limit))
        .layer(TraceLayer::new_for_http())
        .layer(Extension(context))
}

async fn handle_fallback(uri: Uri) -> Error {
    info!(path = %uri.path(), "Requested resource not found");
    Error::RouteNotFound
}
