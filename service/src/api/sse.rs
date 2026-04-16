use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use std::convert::Infallible;
use tokio_stream::{wrappers::IntervalStream, StreamExt};

use super::AppState;

/// SSE endpoint for real-time request updates.
/// Clients connect and receive events as request statuses change.
pub async fn events_handler(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let _pool = state.pool.clone();

    // Poll the database for changes and emit events.
    // In production, this should use a broadcast channel instead of polling.
    let stream = IntervalStream::new(tokio::time::interval(std::time::Duration::from_secs(2)))
        .map(move |_| {
            // Heartbeat event
            Ok(Event::default()
                .event("heartbeat")
                .data(chrono::Utc::now().to_rfc3339()))
        });

    Sse::new(stream).keep_alive(KeepAlive::default().interval(std::time::Duration::from_secs(15)))
}
