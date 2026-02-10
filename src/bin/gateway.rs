use axum::{routing::post, Json, Router};
use opentelemetry::propagation::Injector;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use trace_first_demo::types::{ProcessRequest, ProcessResponse, WorkerRequest, WorkerResponse};

struct HeaderInjector<'a>(&'a mut reqwest::header::HeaderMap);

impl<'a> Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        let name = reqwest::header::HeaderName::from_bytes(key.as_bytes()).unwrap();
        let val = reqwest::header::HeaderValue::from_str(&value).unwrap();
        self.0.insert(name, val);
    }
}

#[tokio::main]
async fn main() {
    let provider = trace_first_demo::telemetry::init_telemetry("gateway");

    let app = Router::new().route("/process", post(process_request));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("gateway listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();

    provider.shutdown().ok();
}

async fn process_request(
    Json(req): Json<ProcessRequest>,
) -> Json<ProcessResponse> {
    let span = tracing::info_span!(
        "http_request",
        http.method = "POST",
        http.route = "/process",
        http.status_code = tracing::field::Empty,
        otel.status_code = tracing::field::Empty,
        otel.status_message = tracing::field::Empty,
        request_id = %req.id,
    );

    async move {
        if !validate_input(&req) {
            tracing::Span::current().record("http.status_code", 400);
            tracing::Span::current().record("otel.status_code", "ERROR");
            tracing::Span::current().record("otel.status_message", "invalid input");
            return Json(ProcessResponse {
                id: req.id,
                status: String::from("error"),
                message: "invalid input".into(),
            });
        }

        let worker_resp = dispatch_to_worker(&req).await;

        if worker_resp.success {
            tracing::Span::current().record("http.status_code", 200);
            Json(ProcessResponse {
                id: req.id,
                status: "success".to_string(),
                message: worker_resp.message,
            })
        } else {
            tracing::error!(error = %worker_resp.message, "worker reported failure");
            tracing::Span::current().record("http.status_code", 500);
            tracing::Span::current().record("otel.status_code", "ERROR");
            tracing::Span::current().record("otel.status_message", worker_resp.message.as_str());
            Json(ProcessResponse {
                id: req.id,
                status: "error".into(),
                message: worker_resp.message,
            })
        }
    }
    .instrument(span)
    .await
}

#[tracing::instrument(
    name = "validate_input",
    skip(req),
    fields(validation.result, validation.errors)
)]
fn validate_input(req: &ProcessRequest) -> bool {
    tracing::info!(payload_size = req.payload.len(), "validating request");

    if req.id.is_empty() || req.payload.is_empty() {
        tracing::Span::current().record("validation.result", "failed");
        tracing::Span::current().record("validation.errors", "empty id or payload");
        return false;
    }

    tracing::Span::current().record("validation.result", "ok");
    true
}

#[tracing::instrument(
    name = "dispatch_to_worker",
    skip(req),
)]
async fn dispatch_to_worker(req: &ProcessRequest) -> WorkerResponse {
    let worker_url = std::env::var("WORKER_URL")
        .unwrap_or_else(|_| "http://localhost:3001".into());

    tracing::info!("forwarding to worker");

    let mut headers = reqwest::header::HeaderMap::new();
    let cx = tracing::Span::current().context();
    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, &mut HeaderInjector(&mut headers));
    });

    // TODO: reuse client across requests
    let client = reqwest::Client::new();
    let worker_req = WorkerRequest {
        id: req.id.clone(),
        payload: req.payload.clone(),
    };

    match client
        .post(format!("{}/work", worker_url))
        .headers(headers)
        .json(&worker_req)
        .send()
        .await
    {
        Ok(resp) => resp.json::<WorkerResponse>().await.unwrap_or(WorkerResponse {
            success: false,
            message: "failed to parse worker response".into(),
        }),
        Err(e) => WorkerResponse {
            success: false,
            message: format!("worker unavailable: {}", e),
        },
    }
}
