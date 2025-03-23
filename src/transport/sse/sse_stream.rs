use super::SessionManager;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::{broadcast, mpsc};

/// A Stream that produces SSE events for a session
pub struct SseEventStream {
    /// The session manager
    session_manager: SessionManager,

    /// The session ID
    session_id: String,

    /// Broadcast receiver
    rx: Option<broadcast::Receiver<String>>,

    /// Whether initial events have been sent
    init_sent: bool,
}

impl SseEventStream {
    pub(crate) fn new(session_manager: SessionManager, session_id: String) -> Self {
        // Register the session
        let (tx, _) = mpsc::channel(100);
        let session_manager_clone = session_manager.clone();
        let session_id_clone = session_id.clone();

        tokio::spawn(async move {
            let sessions = session_manager_clone.sessions();
            let mut sessions_guard = sessions.lock().await;
            sessions_guard.insert(session_id_clone, tx);
        });

        Self {
            session_manager: session_manager.clone(),
            session_id,
            rx: Some(session_manager.broadcaster().subscribe()),
            init_sent: false,
        }
    }
}

impl Stream for SseEventStream {
    type Item = Result<Bytes, warp::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Send initial events if not sent yet
        if !self.init_sent {
            self.init_sent = true;

            // Create endpoint URL event
            let host = "localhost:8000"; // Ideally from request
            let endpoint_url = format!("http://{}/messages?sessionId={}", host, self.session_id);
            let endpoint_event = format!("event: endpoint\ndata: {}\n\n", endpoint_url);

            // Create welcome message event
            let welcome_msg = serde_json::json!({
                "id": 0,
                "jsonrpc": "2.0",
                "method": "welcome",
                "params": {"message": "Connected to SSE stream", "session": self.session_id}
            });
            let welcome_event = format!("event: message\ndata: {}\n\n", welcome_msg);

            // Return combined message
            let data = format!("{}{}", endpoint_event, welcome_event);
            return Poll::Ready(Some(Ok(Bytes::from(data))));
        }

        // Poll the broadcast receiver for messages
        if let Some(rx) = &mut self.rx {
            // Try to receive a message without awaiting
            match rx.try_recv() {
                Ok(msg) => {
                    // Check if message should be sent to this session
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&msg) {
                        if value.get("session").is_none()
                            || value.get("session")
                                == Some(&serde_json::Value::String(self.session_id.clone()))
                        {
                            let event = format!("event: message\ndata: {}\n\n", msg);
                            return Poll::Ready(Some(Ok(Bytes::from(event))));
                        } else {
                            // Skip this message and try again
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    } else {
                        // Non-JSON messages are broadcast to everyone
                        let event = format!("event: message\ndata: {}\n\n", msg);
                        return Poll::Ready(Some(Ok(Bytes::from(event))));
                    }
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                    // No messages available, register waker for future notifications
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Err(_) => {
                    // Channel closed or lagged, end the stream
                    self.rx = None;
                    return Poll::Ready(None);
                }
            }
        } else {
            // No receiver available, end the stream
            return Poll::Ready(None);
        }
    }
}
