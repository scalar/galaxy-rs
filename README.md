# Scalar Galaxy

This library provides convenient access to the Scalar Galaxy REST API from asynchronous Rust.

The full API of this library can be found in [api.md](./api.md).

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
scalar-galaxy = "0.5.0" # x-release-please-version
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
    let client = Galaxy::from_env()?;

    let response = client.planets().list().send().await?;

    println!("{:?}", response);

    Ok(())
}
```

Every operation returns a request builder; set optional parameters fluently
and finish with `.send().await`.

`from_env` reads the environment variables listed under Authentication below. A
variable that is not set is skipped rather than reported, so export them before
running the example, or authenticated requests will be rejected. The builder
takes every credential explicitly instead:

```rust,no_run
use scalar_galaxy::Galaxy;

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let client = Galaxy::builder()
        .bearer_auth("…")
        .basic_auth_username("…")
        .basic_auth_password("…")
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

- `bearer_auth` — environment variable `BEARER_AUTH`
- `basic_auth_username` — environment variable `BASIC_AUTH_USERNAME`
- `basic_auth_password` — environment variable `BASIC_AUTH_PASSWORD`
- `api_key_header` — environment variable `API_KEY_HEADER`
- `api_key_query` — environment variable `API_KEY_QUERY`
- `api_key_cookie` — environment variable `API_KEY_COOKIE`
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
use scalar_galaxy::{ApiErrorKind, Error};

fn report<T>(result: Result<T, Error>) {
    match result {
        Ok(value) => { /* … */ }
        Err(Error::Api(api)) => match api.kind() {
            ApiErrorKind::NotFound => eprintln!("not found"),
            ApiErrorKind::RateLimit => eprintln!("rate limited; retry later"),
            ApiErrorKind::InternalServer => eprintln!("server error {}", api.status),
            _ => eprintln!("status {}: {:?}", api.status, api.body),
        },
        Err(other) => eprintln!("request failed: {other}"),
    }
}
```

`Error::Api` carries the response `status`, `headers`, the server-assigned
`request_id` when there was one, the decoded `body`, and the raw text of a
body that was not JSON — nothing about a failed response is discarded.

[`ApiErrorKind`] classifies the status: `BadRequest` (400), `Authentication`
(401), `PermissionDenied` (403), `NotFound` (404), `Conflict` (409),
`UnprocessableEntity` (422), `RateLimit` (429), `InternalServer` (500 and
above), and `Other` for any other status. It is non-exhaustive, so a match
on it needs a `_` arm. `Error::status()` returns the status of an API failure
and `None` for every other kind of error.

## Bring your own HTTP client

Every request flows through the `transport::Transport` trait — one
`execute` method over `http::Request<SdkBody>`. The default backend is
`reqwest`; to tune it (proxies, connection pools, TLS settings), pass a
configured client builder:

```rust,no_run
use scalar_galaxy::Galaxy;
use scalar_galaxy::transport::ReqwestTransport;

fn configure() -> Result<(), Box<dyn std::error::Error>> {
    let http_client = reqwest::Client::builder().pool_max_idle_per_host(8);
    let client = Galaxy::builder()
        .transport(ReqwestTransport::from_builder(http_client)?)
        .build()?;
    Ok(())
}
```

`from_builder` turns reqwest's own redirect following off; the SDK client
follows redirects itself (see [Untrusted input](#untrusted-input)).
`ReqwestTransport::from_client` takes an already-built `reqwest::Client` and
uses it as is, so build that client with
`.redirect(reqwest::redirect::Policy::none())`. reqwest follows redirects by
default, and on a hop to another host it drops `Authorization` and cookies
but keeps other headers, so a header-placed API key goes wherever the
redirect names.

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
| `websocket` | no | Nothing: this API has no WebSocket operations. |
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
  fresh one. Reading a *buffered* response body is bounded the same way, so a
  server that sends headers and then stalls cannot hang the call. The default
  skips streamed and multipart uploads and the body of a binary download, and
  streaming responses are always exempt.
- Redirects (301, 302, 303, 307, 308) are followed by the client, not the
  HTTP backend, at most 20 per request. A redirect to another origin (scheme,
  host, or port) drops `Authorization`, `Cookie`, `Proxy-Authorization`, and
  every credential header for the rest of the chain, and the client adds no
  query credential to it (a `Location` the server wrote is followed as given);
  an https→http downgrade or a non-HTTP target is refused. A 303, or a 301/302
  answering a POST, continues as a GET without the body; other redirects
  resend the body and are refused when it was a one-shot stream. A next-page
  link that points at another origin is refused outright.
- A path parameter that is empty, or fills a whole segment and is `.` or
  `..`, is refused with `Error::Config` before anything is sent, and so is a
  `.` or `..` segment that parameters sharing it form: each would address a
  different endpoint on the same host.
- Header values are never logged, URLs are logged with their query string
  redacted, and request/response bodies are never logged.

## API reference

See [`api.md`](./api.md) for the full list of resources and operations.
