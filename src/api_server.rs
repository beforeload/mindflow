//! Local HTTP API, the equivalent of flomo's "记录 API":
//!
//! `POST http://127.0.0.1:<port>/api/v1/memos` with `{"content": "..."}` and
//! `Authorization: Bearer <token>` creates a memo. `GET /api/v1/memos?q=&tag=&limit=`
//! searches and `GET /api/v1/tags` lists tags.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::Result;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, Server};

use crate::core::query::{MemoFilter, SortOrder, filter_memos};
use crate::core::store::{NewMemo, Store};
use crate::core::{tags, time};

pub struct ApiServer {
    server: Arc<Server>,
    thread: Option<JoinHandle<()>>,
}

impl ApiServer {
    pub fn start(data_dir: PathBuf, port: u16, token: String) -> Result<Self> {
        let server = Arc::new(
            Server::http(("127.0.0.1", port))
                .map_err(|e| anyhow::anyhow!("无法监听端口 {port}：{e}"))?,
        );
        let worker = server.clone();
        let thread = std::thread::spawn(move || {
            let Ok(mut store) = Store::open(&data_dir) else {
                return;
            };
            for mut request in worker.incoming_requests() {
                let _ = store.sync_external_changes();
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let (status, payload) = handle(
                    &mut store,
                    &token,
                    request.method(),
                    request.url(),
                    request.headers(),
                    body,
                );
                let response = Response::from_string(payload.to_string())
                    .with_status_code(status)
                    .with_header(
                        Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                            .unwrap(),
                    );
                let _ = request.respond(response);
            }
        });
        Ok(Self {
            server,
            thread: Some(thread),
        })
    }
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(v) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn authorized(token: &str, url: &str, headers: &[Header]) -> bool {
    if token.is_empty() {
        return true;
    }
    let bearer = headers
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .map(|h| h.value.as_str().trim_start_matches("Bearer ").trim().to_string());
    bearer.as_deref() == Some(token) || query_param(url, "token").as_deref() == Some(token)
}

fn memo_value(store: &Store, id: &str) -> Value {
    store
        .get(id)
        .map(|m| {
            json!({
                "id": m.id, "content": m.content, "tags": m.tags,
                "created_at": time::fmt_datetime(m.created_at),
            })
        })
        .unwrap_or(Value::Null)
}

fn handle(
    store: &mut Store,
    token: &str,
    method: &Method,
    url: &str,
    headers: &[Header],
    body: String,
) -> (u16, Value) {
    if !authorized(token, url, headers) {
        return (401, json!({"error": "unauthorized"}));
    }
    let path = url.split('?').next().unwrap_or("");
    match (method, path) {
        (&Method::Post, "/api/v1/memos") => {
            let content = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("content").and_then(|c| c.as_str()).map(|s| s.to_string()))
                .unwrap_or(body);
            if content.trim().is_empty() {
                return (400, json!({"error": "content 不能为空"}));
            }
            match store.create_memo(NewMemo {
                content: content.trim().to_string(),
                created_at: None,
                images: vec![],
                source: "api".into(),
            }) {
                Ok(id) => (201, json!({"code": 0, "memo": memo_value(store, &id)})),
                Err(e) => (500, json!({"error": e.to_string()})),
            }
        }
        (&Method::Get, "/api/v1/memos") => {
            let limit = query_param(url, "limit").and_then(|l| l.parse().ok()).unwrap_or(20usize);
            let filter = MemoFilter {
                keywords: query_param(url, "q").unwrap_or_default(),
                tag: query_param(url, "tag"),
                ..Default::default()
            };
            let memos: Vec<Value> = filter_memos(store, &filter, SortOrder::CreatedDesc)
                .iter()
                .take(limit)
                .map(|m| memo_value(store, &m.id))
                .collect();
            (200, json!({"code": 0, "memos": memos}))
        }
        (&Method::Get, "/api/v1/tags") => {
            let counts = tags::tag_counts(store.active_memos());
            (200, json!({"code": 0, "tags": counts}))
        }
        _ => (404, json!({"error": "not found"})),
    }
}
