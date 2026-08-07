# Scalar Galaxy

This library provides convenient access to the Scalar Galaxy REST API from asynchronous Rust.

The full API of this library can be found in [api.md](./api.md).

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
scalar-galaxy = "0.1.0" # x-release-please-version
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Or install via cargo:

```sh
cargo add scalar-galaxy
cargo add tokio --features rt-multi-thread,macros
```

## Usage

The client is asynchronous, with `reqwest` as the default HTTP backend
(swappable — see "Bring your own HTTP client" below):

```rust,ignore
use scalar_galaxy::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Galaxy::builder()
        .bearer_token(std::env::var("SCALAR_BEARER_TOKEN")?)
        .build()?;

    let response = client.planets().list_all_data().send().await?;

    println!("{:?}", response);

    Ok(())
}
```

Every operation returns a request builder; set optional parameters fluently
and finish with `.send().await`.

The builder accepts every credential this API takes, and `from_env` reads
them from the environment instead:

```rust,no_run
use scalar_galaxy::Galaxy;

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let client = Galaxy::builder()
        .bearer_token("…")
        .username("…")
        .password("…")
        .api_key_header("…")
        .api_key_query("…")
        .api_key_cookie("…")
        .access_token("…")
        .build()?;

    // Or, reading credentials from the environment:
    let client = Galaxy::from_env()?;
    let _ = client;
    Ok(())
}
```

## Authentication

Credentials can be set on the builder or read from the environment by
`from_env`:

- `bearer_token` — environment variable `SCALAR_BEARER_TOKEN`
- `username` — environment variable `SCALAR_USERNAME`
- `password` — environment variable `SCALAR_PASSWORD`
- `api_key_header` — environment variable `SCALAR_API_KEY_HEADER`
- `api_key_query` — environment variable `SCALAR_API_KEY_QUERY`
- `api_key_cookie` — environment variable `SCALAR_API_KEY_COOKIE`
- `access_token` — environment variable `SCALAR_ACCESS_TOKEN`
- `client_id` — environment variable `SCALAR_CLIENT_ID`
- `client_secret` — environment variable `SCALAR_CLIENT_SECRET`

With the client id and secret set, the client performs the OAuth 2.0
client-credentials exchange itself: it fetches an access token on the first
request, caches it until shortly before it expires, refreshes it as needed,
and never lets concurrent requests stampede the token endpoint. Token
requests go through the configured transport, so a custom transport (or
`MockTransport` in tests) sees them like any other request. Setting
`access_token` explicitly skips the exchange entirely.

The exchange requests every scope this API documents
(`read:account write:planets read:planets`) by default. A client granted only a subset
should narrow the request with `oauth_scopes` (or the
`SCALAR_OAUTH_SCOPES` environment variable, whitespace-separated) —
asking for a scope that was not granted is rejected as `invalid_scope`:

```rust,ignore
let client = Galaxy::builder().oauth_scopes(["read:account"]).build()?;
```

This API accepts alternative credentials, and the client applies the first
set it holds in full. A token is only fetched when that set is the one
backed by the OAuth flow, so requests another configured credential
authenticates never reach the token endpoint — and if an exchange fails
while another alternative is configured, the request is sent with that
alternative instead of failing.

## Error handling

Fallible operations return [`Error`]. Match on the
result of `send().await` to distinguish API errors (with status and decoded
body) from transport and decoding failures:

```rust,no_run
use scalar_galaxy::Error;

fn report<T>(result: Result<T, Error>) {
    match result {
        Ok(value) => { /* … */ }
        Err(Error::Api(api)) => eprintln!("status {}: {:?}", api.status, api.body),
        Err(other) => eprintln!("request failed: {other}"),
    }
}
```

`Error::Api` carries the response `status`, `headers`, the server-assigned
`request_id` when there was one, the decoded `body`, and the raw text of a
body that was not JSON — nothing about a failed response is discarded.

## Bring your own HTTP client

Every request flows through the `transport::Transport` trait — one
`execute` method over `http::Request<SdkBody>`. The default backend is
`reqwest`; to tune it (proxies, connection pools, TLS settings), pass a
configured client:

```rust,no_run
use scalar_galaxy::Galaxy;
use scalar_galaxy::transport::ReqwestTransport;

fn configure() -> Result<(), Box<dyn std::error::Error>> {
    let http_client = reqwest::Client::builder().build()?;
    let client = Galaxy::builder()
        .transport(ReqwestTransport::from_client(http_client))
        .build()?;
    Ok(())
}
```

To replace reqwest entirely, implement `Transport` for your backend and
drop the default features:

```sh
cargo add scalar-galaxy --no-default-features --features tokio
```

Re-enabling the `reqwest` feature on top of `--no-default-features` also
requires a TLS backend — add `rustls` or `native-tls` (the default feature
set includes one), or the crate fails to compile with a clear error.

`SdkBody` implements `http_body::Body`, so hyper and tower stacks consume
requests without conversion glue.
A complete hyper-backed transport ships
in [`examples/byo_transport_hyper.rs`](./examples/byo_transport_hyper.rs).
Retries, deadlines, auth, and decoding stay in the SDK runtime, so a custom
transport only moves one request and classifies its failures.

## Testing with MockTransport

The `mock` cargo feature ships an in-process transport with an ordered
response queue and full request capture, so tests drive the real client
without a network:

```sh
cargo add --dev scalar-galaxy --features mock
```

```rust,ignore
use scalar_galaxy::transport::{InstantSleep, MockTransport};

let mock = MockTransport::new();
mock.enqueue(200, r#"{"id":"example"}"#);

let client = Galaxy::builder()
    .transport(mock.clone()) // clone into the builder, assert on the original
    .sleeper(InstantSleep) // retry backoff resolves instantly in tests
    .build()?;

// ... call the client, then assert on what went over the wire:
let requests = mock.requests();
assert_eq!(requests[0].method, http::Method::GET);
```

Responses are returned strictly in the order enqueued; executing a request
with an empty queue panics, so an unexpected extra request fails loudly.

A complete, network-free round-trip ships in [`examples/mock_transport.rs`](./examples/mock_transport.rs).

## Tracing

The `tracing` feature (off by default — enabling it is the only way this
crate pulls in the `tracing` dependency) instruments the request runtime
with `debug`-level events: each request attempt, each retry decision and the
delay it chose, the terminal outcome, and response decode failures. A
credential the client could not obtain — a failed OAuth token exchange that
another configured credential covered for — is reported at `warn`.

```sh
cargo add scalar-galaxy --features tracing
```

This crate emits events only and never installs a subscriber — your binary
does that, e.g. with `tracing-subscriber`:

```sh
RUST_LOG=scalar_galaxy=debug cargo run
```

Credentials never reach an event: URLs are logged with their query string
redacted, header values and names are never logged, and request/response
bodies are never logged (a failed decode reports the status and an error
class only). Event targets, messages, and field names are diagnostics rather
than API, and may change in any release.

## Cargo features

| Feature | Default | Enables |
| --- | --- | --- |
| `reqwest` | yes | The bundled `reqwest` HTTP backend. Implies `tokio`. |
| `tokio` | yes | The tokio-backed timer used for retry backoff and deadlines. |
| `rustls` | yes | TLS for the bundled backend via `rustls`. |
| `native-tls` | no | TLS for the bundled backend via the platform's own stack. |
| `multipart` | yes | The vendored `multipart/form-data` encoder for file uploads. |
| `mock` | no | `MockTransport` and `InstantSleep` for network-free tests. |
| `tracing` | no | `debug`-level request instrumentation via the `tracing` facade. |

Features are additive: no combination is rejected, and none changes the
behavior of another. Dropping the defaults yields a `reqwest`-free crate for
a bring-your-own-transport build.

## Minimum supported Rust version

The MSRV is Rust 1.85. Raising it is a minor version change, never a
patch: a patch release will always build on the MSRV declared by the minor
series it belongs to. `rust-version` in `Cargo.toml` is the machine-readable
form of the same promise, so Cargo's resolver honors it automatically.

## Platform notes

The default `rustls` TLS backend builds `aws-lc-rs`, which needs CMake and
NASM on `PATH` when no prebuilt artifact matches the host — most often hit
on Windows. To use the platform TLS stack (Schannel, Secure Transport)
instead:

```sh
cargo add scalar-galaxy --no-default-features --features reqwest,native-tls
```

## Reusing the client

Construct one `Galaxy` and share it for the lifetime of your
process. All of its state sits behind an `Arc`, so `Clone` is cheap and every
clone shares one transport — and therefore one connection pool, one cached
access token, and one set of default headers. Building a client per request
silently gives up connection reuse and re-runs any token exchange.

## Untrusted input

Response bodies are untrusted, so the runtime bounds them by default:

- Buffered response bodies are capped at 64 MiB (`max_response_size`), which
  bounds memory against a hostile or misconfigured server. Streaming responses
  are consumed incrementally and are exempt.
- Each attempt carries a 60-second deadline unless one is set explicitly; it
  covers request-body upload through response headers, and every retry gets a
  fresh one.
- The bundled backend follows **no** redirects: a followed redirect can
  re-send credentials to a host the caller never chose. A next-page link that
  points at another origin is refused for the same reason.
- Header values are never logged, URLs are logged with their query string
  redacted, and request/response bodies are never logged.

## API reference

See [`api.md`](./api.md) for the full list of resources and operations.
