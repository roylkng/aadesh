use axum::response::{Html, IntoResponse};

const INDEX_HTML: &str = include_str!("../../ui/index.html");

pub async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}
