# paper-trail

Companion repo for my blog post on why I think we've been doing observability backwards.

The argument: instead of scattering `log::info!()` everywhere and then grepping through thousands of lines when something breaks, what if you just... instrumented first? Every unit of work is a span. Every span has attributes. When something goes wrong, you click a trace in Jaeger and see the full story — which service, which step, why it failed, how long it took. No grep. No correlating timestamps across files. Just click and see.

## what's in here

Two Rust services talking over HTTP, both exporting traces to Jaeger via OpenTelemetry:

- **gateway** accepts requests, validates them, forwards to the worker. Injects W3C `traceparent` headers so the trace crosses the network boundary.
- **worker** does "computation" (sleeps for a bit, occasionally fails). Extracts the trace context from headers so its spans show up in the same trace.

The worker has two bad-day modes baked in — ~10% of requests fail outright and ~5% are weirdly slow (500ms+ instead of the usual ~100ms). This is deliberate. The whole point is to show how you'd find and debug these in Jaeger vs. how painful it'd be with just logs.

## running it

```bash
docker compose up --build
```

Send some traffic:

```bash
for i in {1..20}; do
  curl -s -X POST http://localhost:3000/process \
    -H "Content-Type: application/json" \
    -d "{\"id\": \"req-$i\", \"payload\": \"test data $i\"}"
  echo
done
```

Then open http://localhost:16686, pick **gateway**, hit Find Traces. Look for the red ones (errors) and sort by duration to spot the slow outlier. Click into any trace and you'll see the full span tree across both services — plus the `tracing::info!()` calls show up as structured events on each span. That's the thing people miss: trace-first doesn't mean no logs. It means your logs know where they belong.
