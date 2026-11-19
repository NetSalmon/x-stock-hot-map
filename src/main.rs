// Copyright (C) 2026 NetSalmon
// SPDX-License-Identifier: AGPL-3.0-only

mod demo;
mod model;
mod score;
mod store;
mod yahoo;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tower_http::services::ServeDir;

use crate::model::{Circle, CreateReq, FeedbackReq};
use crate::score::{build_users, market_of};
use crate::store::{Store, new_id, valid_id};

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    store: Store,
}

fn project_dir() -> PathBuf {
    fn has_page(dir: &std::path::Path) -> bool {
        dir.join("static").join("index.html").is_file()
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent().filter(|dir| has_page(dir)) {
            return dir.to_path_buf();
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if has_page(&cwd) {
            return cwd;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[tokio::main]
async fn main() {
    let root = project_dir();
    let store = Store::new(root.join("data")).expect("create data directory");
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
        .build()
        .expect("http client");
    let state = AppState { client, store };
    let assets = root.join("static");
    let app = Router::new()
        .route("/", get(index))
        .route("/c/{id}", get(index))
        .route("/api/circles", post(create_circle))
        .route("/api/circles/{id}", get(get_circle))
        .route("/api/demo", get(get_demo))
        .route("/api/leaderboard", get(leaderboard))
        .route("/api/stats", get(stats))
        .route("/api/feedback", post(feedback))
        .route("/api/avatar", get(avatar))
        .nest_service("/assets", ServeDir::new(assets))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8787".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|err| panic!("bind {addr}: {err}"));
    println!("X星图 http://127.0.0.1:{port}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .unwrap();
}

async fn index() -> Response {
    let path = project_dir().join("static/index.html");
    match std::fs::read(path) {
        Ok(body) => (
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            body,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("页面缺失：{err}"),
        )
            .into_response(),
    }
}

async fn get_demo() -> Json<Circle> {
    Json(demo::demo_circle(now_secs()))
}

async fn get_circle(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    if id == "demo" {
        return Json(demo::demo_circle(now_secs())).into_response();
    }
    if !valid_id(&id) {
        return ApiError::new(StatusCode::BAD_REQUEST, "圈子 ID 应为 8 位").into_response();
    }
    match state.store.load(&id) {
        Ok(Some(circle)) => Json(circle).into_response(),
        Ok(None) => {
            ApiError::new(StatusCode::NOT_FOUND, "没有这张星图，或临时星图已过期").into_response()
        }
        Err(err) => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
        }
    }
}

async fn create_circle(State(state): State<AppState>, Json(req): Json<CreateReq>) -> Response {
    let username = normalize_username(&req.username);
    if username.is_empty() {
        return ApiError::new(StatusCode::BAD_REQUEST, "请输入 X 用户名").into_response();
    }
    let (events, tweet_count, display_name, avatar) =
        match yahoo::collect(&state.client, &username).await {
            Ok(v) => v,
            Err(err) => return ApiError::new(StatusCode::BAD_GATEWAY, err).into_response(),
        };
    if events.is_empty() {
        return ApiError::new(
            StatusCode::NOT_FOUND,
            "没有采到公开的回复、引用、提及或转发。换一个近期有公开互动的用户名再试。",
        )
        .into_response();
    }
    let now = now_secs();
    let users = build_users(&events, now);
    if users.is_empty() {
        return ApiError::new(StatusCode::NOT_FOUND, "公开互动里没有可排名的账号").into_response();
    }
    let market = market_of(&users);
    let mut id = new_id();
    for _ in 0..5 {
        if !matches!(state.store.load(&id), Ok(Some(_))) {
            break;
        }
        id = new_id();
    }
    let circle = Circle {
        id,
        username,
        display_name,
        avatar,
        created_at: now,
        persist: req.persist,
        source: "yahoo".to_string(),
        tweet_count,
        users,
        market,
    };
    if let Err(err) = state.store.save(&circle) {
        return ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
    }
    Json(circle).into_response()
}

async fn leaderboard(State(state): State<AppState>) -> Response {
    match state.store.leaderboard() {
        Ok(items) => Json(items).into_response(),
        Err(err) => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
        }
    }
}

async fn stats(State(state): State<AppState>) -> Response {
    match state.store.stats() {
        Ok(stats) => Json(stats).into_response(),
        Err(err) => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
        }
    }
}

#[derive(Deserialize)]
struct AvatarQuery {
    u: String,
}

async fn avatar(State(state): State<AppState>, Query(query): Query<AvatarQuery>) -> Response {
    if !allowed_avatar(&query.u) {
        return ApiError::new(StatusCode::BAD_REQUEST, "头像地址不在允许范围内").into_response();
    }
    let response = match state
        .client
        .get(&query.u)
        .header(
            "accept",
            "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
        )
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };
    if !response.status().is_success() {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    if !content_type.starts_with("image/") {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let bytes = match response.bytes().await {
        Ok(bytes) if bytes.len() <= 1_500_000 => bytes,
        _ => return StatusCode::BAD_GATEWAY.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=86400".to_string()),
        ],
        bytes,
    )
        .into_response()
}

fn allowed_avatar(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("rts-pctr.c.yimg.jp" | "pbs.twimg.com" | "abs.twimg.com")
        )
}

async fn feedback(State(state): State<AppState>, Json(req): Json<FeedbackReq>) -> Response {
    let text = req.text.trim();
    if text.chars().count() < 2 {
        return ApiError::new(StatusCode::BAD_REQUEST, "反馈内容太短").into_response();
    }
    if text.chars().count() > 2000 {
        return ApiError::new(StatusCode::BAD_REQUEST, "反馈内容过长").into_response();
    }
    let contact = req.contact.trim();
    if contact.chars().count() > 200 {
        return ApiError::new(StatusCode::BAD_REQUEST, "联系方式过长").into_response();
    }
    match state.store.feedback(text, contact) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
        }
    }
}

fn normalize_username(raw: &str) -> String {
    let s = raw.trim().trim_start_matches('@');
    if (1..=15).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        s.to_ascii_lowercase()
    } else {
        String::new()
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
