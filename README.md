# Rwanda HTMX Performance Lab

A standalone performance comparison between **Rust + Actix Web + Askama** full-page rendering and **HTMX** fragment updates. Both modes use the same in-memory Rwanda location dataset and the same filtering function, making response/navigation strategy the primary variable.

The real `locations.json` hierarchy is loaded and deserialized exactly once when the server starts:

```text
Province → District → Sector → Cell → Village
```

No database, SPA framework, or client-side rendering framework is used. Bun is only responsible for frontend dependency management and CSS compilation; Actix Web is the application server.

## Stack

- Rust and Actix Web
- Askama server-rendered templates
- HTMX for targeted HTML swaps
- Tailwind CSS 4 and DaisyUI 5, installed with Bun using the [official DaisyUI HTMX setup](https://daisyui.com/docs/install/htmx/)
- `serde` and `serde_json` for the startup-only dataset load
- `tracing` and `tracing-subscriber` for JSON performance logs
- In-memory request records and aggregates

`bun.lock` pins the exact frontend dependency graph used by this project. The manifest requests the current releases of Tailwind CSS, the Tailwind CLI, and DaisyUI.

## Quick start

Requirements: a current Rust toolchain and [Bun](https://bun.sh/).

```bash
bun install
bun run css:build
cargo run
```

Open:

```text
http://127.0.0.1:4562
```

The default port is `4562`. If it is already occupied, override it without changing the benchmark application:

```bash
APP_PORT=4563 cargo run
```

PowerShell:

```powershell
$env:APP_PORT = "4563"
cargo run
```

## Development workflow

Start the complete development application with one command:

```bash
bun run dev
```

This runs the Tailwind CSS watcher and the Actix Web server together. Open `http://127.0.0.1:4562`, and press `Ctrl+C` to stop both processes.

If you only want to rebuild CSS while running Actix separately, use:

```bash
bun run css:watch
```

Then, in another terminal:

```bash
cargo run
```

Bun only orchestrates development tooling; Actix remains the actual HTTP application server.

## What to compare

### HTMX mode

Open `/`. Search uses `hx-trigger="keyup changed delay:300ms"`; filter changes request only option or result fragments; and location inspection loads only the modal content. HTMX requests send `HX-Request: true`, which the middleware records.

### Traditional mode

Open `/traditional`. Search, cascade selections, pagination, and detail inspection use normal GET navigation. State is retained in the URL, and every interaction returns a complete HTML document through `/locations`.

Both modes provide:

- search across province, district, sector, cell, and village names;
- Province → District → Sector → Cell → Village cascading filters;
- paginated breadcrumb results;
- a DaisyUI modal with the selected hierarchy and sibling villages;
- the same optional `delay_ms` test control;
- the same server-side filtering and Askama rendering path.

## Routes

| Route | Response |
| --- | --- |
| `GET /` | Complete HTMX dashboard document |
| `GET /traditional` | Complete traditional dashboard document |
| `GET /locations` | Complete filtered traditional document |
| `GET /fragments/locations` | Results and pagination fragment |
| `GET /fragments/districts` | District `<option>` elements |
| `GET /fragments/sectors` | Sector `<option>` elements |
| `GET /fragments/cells` | Cell `<option>` elements |
| `GET /fragments/villages` | Village `<option>` elements |
| `GET /fragments/location/{id}` | Location modal fragment |
| `GET /health` | Health and startup dataset counts as JSON |
| `GET /metrics` | Recent request records and aggregates as JSON |
| `GET /static/app.css` | Compiled Tailwind + DaisyUI stylesheet |

Filtering parameters are `query`, `province`, `district`, `sector`, `cell`, `village`, `page`, and `per_page`. `delay_ms` adds a testing-only server delay, defaults to `0`, and is capped at 10 seconds.

## Instrumentation

The custom Actix middleware records:

- UTC timestamp;
- HTTP method and request path;
- response status;
- whether `HX-Request: true` was present;
- server processing time in milliseconds;
- response body size when Actix can determine it.

Every response includes a header such as:

```text
Server-Timing: app;dur=3.742
```

Performance events are emitted as structured JSON through `tracing`. The most recent 200 records and running aggregates are held in memory; they reset whenever the process restarts. `/metrics` exposes the snapshot as JSON, while both dashboards show a readable summary.

For a controlled latency comparison:

```text
http://127.0.0.1:4562/locations?query=kigali&delay_ms=100
http://127.0.0.1:4562/fragments/locations?query=kigali&delay_ms=100
```

## Load testing

Build optimized Rust code before recording serious results:

```bash
cargo build --release
./target/release/rwanda-htmx-performance
```

### wrk

Compare a full document with the equivalent result fragment:

```bash
wrk -t4 -c50 -d30s "http://127.0.0.1:4562/locations?query=kigali"

wrk -t4 -c50 -d30s -H "HX-Request: true" "http://127.0.0.1:4562/fragments/locations?query=kigali"
```

Repeat with simulated application work:

```bash
wrk -t4 -c50 -d30s "http://127.0.0.1:4562/locations?query=kigali&delay_ms=25"
wrk -t4 -c50 -d30s -H "HX-Request: true" "http://127.0.0.1:4562/fragments/locations?query=kigali&delay_ms=25"
```

### hey

```bash
hey -z 30s -c 50 "http://127.0.0.1:4562/locations?query=kigali"
hey -z 30s -c 50 -H "HX-Request: true" "http://127.0.0.1:4562/fragments/locations?query=kigali"
```

### k6

Save as `benchmark.js` outside the project or in a temporary test directory:

```javascript
import http from "k6/http";
import { check } from "k6";

export const options = {
  vus: 50,
  duration: "30s",
};

export default function () {
  const mode = __ENV.MODE || "htmx";
  const isHtmx = mode === "htmx";
  const path = isHtmx
    ? "/fragments/locations?query=kigali"
    : "/locations?query=kigali";
  const response = http.get(`http://127.0.0.1:4562${path}`, {
    headers: isHtmx ? { "HX-Request": "true" } : {},
  });
  check(response, { "status is 200": (r) => r.status === 200 });
}
```

Run each mode:

```bash
k6 run -e MODE=traditional benchmark.js
k6 run -e MODE=htmx benchmark.js
```

Keep hardware, release/debug build, concurrency, query, delay, and test duration identical. Warm the server before collecting results, run each case several times, and compare both the load-test output and `/metrics`.

## Results worksheet

| Endpoint | Mode | Requests/sec | Avg latency | p95 latency | Response size | Server time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `/locations?query=kigali` | Traditional |  |  |  |  |  |
| `/fragments/locations?query=kigali` | HTMX |  |  |  |  |  |

Browser DevTools is useful for inspecting transferred bytes and navigation behavior. The `Server-Timing` response header separates application processing time from network and browser costs.

## Interpreting results

HTMX does **not** automatically make Actix, Rust code, dataset filtering, or Askama rendering faster. Both modes execute nearly the same Rust work in this project. The expected practical HTMX advantage is:

- sending a smaller HTML response;
- parsing and replacing less DOM;
- preserving the current document and browser state;
- avoiding a complete browser navigation and full-page repaint.

Under server-heavy simulated delay, the modes may have similar server times because the shared delay dominates. The browser can still transfer and replace less content in HTMX mode. Measure throughput, latency percentiles, bytes, server time, and browser behavior rather than relying on one number.

## Project layout

```text
rwanda-htmx-performance/
├── Cargo.toml
├── bun.lock
├── package.json
├── locations.json
├── tailwind.config.js
├── .gitignore
├── README.md
├── src/
│   ├── main.rs
│   ├── models.rs
│   ├── data.rs
│   ├── routes.rs
│   ├── metrics.rs
│   └── middleware.rs
├── templates/
│   ├── base.html
│   ├── index.html
│   ├── traditional.html
│   └── fragments/
│       ├── location_results.html
│       ├── location_details.html
│       ├── district_options.html
│       ├── sector_options.html
│       ├── cell_options.html
│       └── village_options.html
├── assets/
│   └── input.css
└── static/
    └── app.css
```
