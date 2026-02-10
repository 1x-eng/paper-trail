use axum::{http::HeaderMap, routing::post, Json, Router};
use opentelemetry::propagation::Extractor;
use rand::Rng;
use std::time::Duration;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use trace_first_demo::types::{WorkerRequest, WorkerResponse};

struct HeaderExtractor<'a>(&'a HeaderMap);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

#[tokio::main]
async fn main() {
    let provider = trace_first_demo::telemetry::init_telemetry("worker");

    let app = Router::new().route("/work", post(handle_work));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    tracing::info!("worker listening on 0.0.0.0:3001");
    axum::serve(listener, app).await.unwrap();

    provider.shutdown().ok();
}

async fn handle_work(
    headers: HeaderMap,
    Json(req): Json<WorkerRequest>,
) -> Json<WorkerResponse> {
    let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(&headers))
    });

    let span = tracing::info_span!(
        "process_payload",
        payload.id = %req.id,
        processing.duration_ms = tracing::field::Empty,
    );
    let _ = span.set_parent(parent_cx);

    async {
        let start = std::time::Instant::now();
        let result = simulate_work().await;
        let duration_ms = start.elapsed().as_millis() as i64;
        tracing::Span::current().record("processing.duration_ms", duration_ms);

        match result {
            Ok(()) => Json(WorkerResponse {
                success: true,
                message: format!("processed {}", req.id),
            }),
            Err(e) => Json(WorkerResponse {
                success: false,
                message: format!("failed to process {}: {e}", req.id),
            }),
        }
    }
    .instrument(span)
    .await
}

#[tracing::instrument(
    name = "simulate_work",
    fields(work_type = "computation", work.success, otel.status_code, otel.status_message)
)]
async fn simulate_work() -> Result<(), String> {
    // thread_rng() is !Send so we gotta do this before the await
    let (sleep_ms, should_fail, failure_kind) = {
        let mut rng = rand::thread_rng();
        let ms = if rng.gen::<f32>() < 0.05 {
            rng.gen_range(500..800) // slow path, pretend this is a slow DB query or something
        } else {
            rng.gen_range(50..150)
        };
        (ms, rng.gen::<f32>() < 0.10, rng.gen_range(0u8..3))
    };

    tracing::info!(sleep_ms, "starting computation");
    tokio::time::sleep(Duration::from_millis(sleep_ms)).await;

    if should_fail {
        let err = match failure_kind {
            0 => "connection timeout: database did not respond within 5000ms",
            1 => "deserialization error: unexpected null in response payload",
            _ => "rate limited: upstream returned 429",
        };
        tracing::error!(exception.message = err, "computation error");
        tracing::Span::current().record("work.success", false);
        tracing::Span::current().record("otel.status_code", "ERROR");
        tracing::Span::current().record("otel.status_message", err);
        return Err(err.to_string());
    }

    tracing::info!("computation completed");
    tracing::Span::current().record("work.success", true);
    Ok(())
}
