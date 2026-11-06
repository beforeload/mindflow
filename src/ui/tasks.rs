//! Bridges blocking network calls (run on their own threads) into GPUI tasks.

use gpui_kit::{Context, Task};

use crate::core::ai::{self, ChatMessage};
use crate::core::model::AiConfig;

/// Runs `f` on a dedicated thread; resolves to `None` only if the thread panicked.
pub async fn run_blocking<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> Option<R> {
    let (tx, rx) = smol::channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(f());
    });
    rx.recv().await.ok()
}

pub enum StreamEvent {
    Delta(String),
    Done(String),
    Failed(String),
}

/// Streams a chat completion into `on_event`. Dropping the returned task stops the stream.
pub fn stream_chat<T: 'static>(
    cx: &mut Context<T>,
    cfg: AiConfig,
    messages: Vec<ChatMessage>,
    mut on_event: impl FnMut(&mut T, StreamEvent, &mut Context<T>) + 'static,
) -> Task<()> {
    let (tx, rx) = smol::channel::unbounded::<StreamEvent>();
    std::thread::spawn(move || {
        let result = ai::chat_stream(&cfg, &messages, |delta| {
            tx.send_blocking(StreamEvent::Delta(delta.to_string())).is_ok()
        });
        let _ = tx.send_blocking(match result {
            Ok(full) => StreamEvent::Done(full),
            Err(err) => StreamEvent::Failed(err.to_string()),
        });
    });
    cx.spawn(async move |this, cx| {
        while let Ok(event) = rx.recv().await {
            let finished = !matches!(event, StreamEvent::Delta(_));
            if this.update(cx, |this, cx| on_event(this, event, cx)).is_err() || finished {
                break;
            }
        }
    })
}
