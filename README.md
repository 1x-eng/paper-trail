# paper-trail

companion repo for my blog post on trace-first instrumentation.

the premise is simple and well-established: "metrics for detection, traces for location, logs for explanation" (Google SRE / Cindy Sridharan's *Distributed Systems Observability*). this demo focuses on the middle piece - using traces as the primary debugging tool, with logs naturally falling out of span events rather than being a separate concern.

## what's here

two rust services (gateway -> worker) talking over HTTP, both exporting traces via OpenTelemetry to any OTEL-compliant backend. the docker-compose ships Jaeger but the code is backend-agnostic - swap in Tempo, Honeycomb, Datadog, whatever.

the worker has two failure modes baked in:
- ~10% of requests error (`otel.status_code = ERROR`, spans go red in any compliant UI)
- ~5% hit a slow path (500ms+ vs ~100ms normal)

this is deliberate. the demo is meant to show what you'd actually do on a bad day:
1. filter by error or sort by duration
2. click a trace
3. see the full span tree across both services, with structured events on each span
4. know exactly where it broke, why, and how long each step took

no separate log correlation step. the `tracing::info!()` and `tracing::warn!()` calls are span events - they're already attached to the right trace context.

### trace context propagation

gateway injects W3C `traceparent` into outgoing HTTP headers. worker extracts it. same trace ID, parent-child relationships preserved across the network boundary.

### span attributes

every span carries queryable attributes: `request_id`, `http.method`, `http.status_code`, `processing.duration_ms`, `validation.result`, `work.success`, `otel.status_code`. these aren't log fields you grep for - they're indexed, structured, and on the right span.

## running it

```bash
docker compose up --build
```

```bash
for i in {1..20}; do
  curl -s -X POST http://localhost:3000/process \
    -H "Content-Type: application/json" \
    -d "{\"id\": \"req-$i\", \"payload\": \"test data $i\"}"
  echo
done
```

open http://localhost:16686 (Jaeger) or point any OTEL backend at port 4317.
