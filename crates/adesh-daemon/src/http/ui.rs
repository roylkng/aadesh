use axum::{
    http::{HeaderValue, header},
    response::{Html, IntoResponse},
};

const INDEX_HTML: &str = include_str!("../../ui/index.html");

pub async fn index() -> impl IntoResponse {
    let mut response = Html(INDEX_HTML).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
}
