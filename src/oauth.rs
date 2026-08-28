//! OAuth 2.0 client-credentials token provider.
//!
//! Emitted only for SDKs whose spec declares an `oauth2` scheme with a
//! `clientCredentials` flow carrying a `tokenUrl`. The provider exchanges the
//! client id/secret for an access token (RFC 6749 §4.4), caches it until it is
//! close to expiring, and refreshes it through the *client's own* `Transport`
//! and `Sleep` — never a private HTTP client — so a bring-your-own transport
//! (or a `MockTransport` in tests) gets OAuth for free and sees every token
//! request on the same wire as the API requests.
//!
//! Concurrency: a caller that finds no usable cached token takes an async
//! fetch lock, and re-checks the cache after acquiring it. Concurrent callers
//! therefore issue exactly **one** token request between them — the losers wake
//! up, see the token the winner stored, and return it without touching the
//! network.
//!
//! Nothing here panics: every mutex acquisition recovers from poisoning
//! instead of unwrapping, so a panic in an unrelated task can never take the
//! whole client down.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::error::Error;
use crate::transport::{SdkBody, Sleep, Transport};

/// How long before expiry a cached token is treated as stale.
///
/// A token that expires in the next two minutes is refreshed eagerly rather
/// than raced against the clock: the request it would authenticate still has
/// to travel, and a token that expires in flight fails the call for no reason.
const EXPIRY_BUFFER: Duration = Duration::from_secs(120);

/// Deadline applied to a token request when the client sets none.
///
/// Mirrors the client's own default for buffered bodies: the form body is fully
/// buffered, so bounding the exchange cannot kill a streaming upload. Without
/// it a hung token endpoint would stall every request in the process.
const DEFAULT_TOKEN_TIMEOUT: Duration = Duration::from_secs(60);

/// A cached access token and the instant it stops being usable.
///
/// Deliberately does **not** derive `Debug`: the token is a credential and must
/// never reach log or panic output (the same rule the client's redacting
/// `Debug` and the sensitive-marked credential headers follow).
struct CachedToken {
    access_token: String,
    /// `None` when the token endpoint returned no `expires_in`, which per RFC
    /// 6749 §5.1 leaves the lifetime unspecified — the token is then cached for
    /// the life of the process and only replaced if the server rejects it.
    expires_at: Option<Instant>,
}

impl CachedToken {
    /// Whether this token is still usable, accounting for [`EXPIRY_BUFFER`].
    fn is_fresh(&self) -> bool {
        self.expires_at
            .is_none_or(|at| at.saturating_duration_since(Instant::now()) > EXPIRY_BUFFER)
    }
}

/// The token endpoint's success payload (RFC 6749 §5.1). Extra members are
/// ignored, and `Debug` is deliberately not derived (it carries the token).
#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    /// Lifetime in seconds. Optional per the RFC; absent means "unspecified".
    #[serde(default)]
    expires_in: Option<u64>,
}

/// Fetches, caches, and refreshes client-credentials access tokens.
pub(crate) struct TokenProvider {
    token_url: String,
    client_id: String,
    client_secret: String,
    /// Space-joined `scope` parameter; empty when the flow declares no scopes.
    scope: String,
    /// The cached token. A plain `std::sync::Mutex` because every critical
    /// section is a field read/write with no await inside it.
    cache: Mutex<Option<CachedToken>>,
    /// Single-flight gate. Held across the token request (an await point), so
    /// it has to be an async mutex; `futures_util` already ships one and is
    /// already a dependency, so this adds no crate to the tree.
    fetch_lock: futures_util::lock::Mutex<()>,
}

// A manual `Debug` that names no credential: the client id, secret, and cached
// token are all secrets, and `Inner`/`Client` may be formatted by callers.
impl std::fmt::Debug for TokenProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenProvider")
            .field("token_url", &self.token_url)
            .finish_non_exhaustive()
    }
}

impl TokenProvider {
    /// Creates a provider for one client-credentials flow.
    /// `token_url` is resolved against `base_url` with RFC 3986 semantics before
    /// it is stored: OpenAPI permits a relative `tokenUrl` (multi-environment
    /// specs commonly declare `/oauth/token`), and parsing that as a `Uri`
    /// yields an authority-less target the transport cannot dispatch. An
    /// absolute token URL is unaffected — `join` returns it unchanged.
    pub(crate) fn new(
        base_url: &url::Url,
        token_url: &str,
        client_id: String,
        client_secret: String,
        scope: &str,
    ) -> Self {
        Self {
            token_url: base_url
                .join(token_url)
                .map_or_else(|_| token_url.to_string(), String::from),
            client_id,
            client_secret,
            scope: scope.to_string(),
            cache: Mutex::new(None),
            fetch_lock: futures_util::lock::Mutex::new(()),
        }
    }

    /// Returns a usable access token, fetching or refreshing it if needed.
    ///
    /// The fast path is a cache read under a sync lock — no allocation beyond
    /// cloning the token, no await. Only a missing or nearly-expired token
    /// takes the async fetch lock, and the cache is re-checked after acquiring
    /// it so concurrent callers share the winner's token instead of each
    /// hitting the token endpoint.
    ///
    /// # Errors
    ///
    /// Returns whatever the token exchange failed with — [`Error::Transport`]
    /// for a network failure, [`Error::Api`] for a rejected exchange (bad
    /// credentials surface as the endpoint's own 400/401). Failures are never
    /// cached, so the next call retries.
    pub(crate) async fn access_token(
        &self,
        transport: &dyn Transport,
        sleep: &dyn Sleep,
        max_retries: u32,
        deadline: Option<Duration>,
        max_response_size: usize,
    ) -> Result<String, Error> {
        if let Some(token) = self.cached() {
            return Ok(token);
        }
        let _guard = self.fetch_lock.lock().await;
        // Another caller may have refreshed while this one waited for the lock;
        // taking their token is what makes concurrent callers single-flight.
        if let Some(token) = self.cached() {
            return Ok(token);
        }
        let fetched = self
            .fetch(transport, sleep, max_retries, deadline, max_response_size)
            .await?;
        let token = fetched.access_token.clone();
        self.store(fetched);
        Ok(token)
    }

    /// The cached token when one is present and still fresh.
    fn cached(&self) -> Option<String> {
        // Poison recovery instead of `unwrap`: a panic in an unrelated task
        // must not turn every later request into a panic.
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let token = cache.as_ref()?;
        token.is_fresh().then(|| token.access_token.clone())
    }

    /// Replaces the cached token.
    fn store(&self, token: CachedToken) {
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        *cache = Some(token);
    }

    /// Performs one `grant_type=client_credentials` exchange.
    ///
    /// The request rides the client's transport, sleeper, and retry count, so
    /// it inherits the same retry taxonomy, deadline enforcement, and response
    /// size cap as every API call.
    async fn fetch(
        &self,
        transport: &dyn Transport,
        sleep: &dyn Sleep,
        max_retries: u32,
        deadline: Option<Duration>,
        max_response_size: usize,
    ) -> Result<CachedToken, Error> {
        // Scoped so the serializer is dropped before the `.await` below.
        // `url::form_urlencoded::Serializer` is not `Send`, and a binding that
        // merely stops being *used* is still held across an await point — which
        // would make this future non-`Send`. Pagination requires exactly that
        // bound (`PageFuture` is `Pin<Box<dyn Future + Send>>`), so without the
        // block an SDK with both OAuth client-credentials and a paginated
        // operation fails to compile.
        let body = {
            let mut form = url::form_urlencoded::Serializer::new(String::new());
            form.append_pair("grant_type", "client_credentials");
            if !self.scope.is_empty() {
                form.append_pair("scope", &self.scope);
            }
            form.finish()
        };

        let mut request = http::Request::new(SdkBody::from_bytes(body.into_bytes()));
        *request.method_mut() = http::Method::POST;
        *request.uri_mut() = self
            .token_url
            .parse::<http::Uri>()
            .map_err(|error| Error::Config(format!("invalid OAuth token URL: {error}")))?;
        request.headers_mut().insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        request
            .headers_mut()
            .insert(http::header::ACCEPT, http::HeaderValue::from_static("application/json"));
        request
            .headers_mut()
            .insert(http::header::AUTHORIZATION, self.basic_authorization()?);

        // A token response is small by construction, so it always gets a
        // deadline — the caller's if they set one, otherwise the token default.
        let deadline = deadline.or(Some(DEFAULT_TOKEN_TIMEOUT));
        // A token request is a POST, so `send_with_retries` will only replay it
        // for statuses the server could not have processed (408/429/503) and
        // for failed connections — exactly the right taxonomy here.
        let response = crate::http::send_with_retries(transport, sleep, request, max_retries, deadline, false).await?;
        // Reuses the shared decode path, so a non-success token response
        // surfaces as the same `Error::Api` (status, request id, summarized
        // body) callers already handle, under the configured size cap. The read
        // is deadline-bounded like every other buffered decode: without it a
        // token endpoint that sends headers and then stalls would hang every
        // request waiting on the credential.
        let payload: TokenResponse = crate::http::read_with_deadline(
            sleep,
            deadline,
            crate::http::decode_json::<TokenResponse>(response, max_response_size),
        )
        .await?;
        Ok(CachedToken {
            access_token: payload.access_token,
            // `checked_add` rather than `+`: `expires_in` is untrusted server
            // input, and a hostile value near u64::MAX would overflow the
            // instant arithmetic. An unrepresentable expiry means "no known
            // expiry", which is the same conservative handling as a missing
            // `expires_in`.
            expires_at: payload
                .expires_in
                .and_then(|seconds| Instant::now().checked_add(Duration::from_secs(seconds))),
        })
    }

    /// Builds the `Authorization: Basic` header carrying the client credentials.
    ///
    /// RFC 6749 §2.3.1 requires the client id and secret to be
    /// `application/x-www-form-urlencoded`-encoded *before* being base64'd, so
    /// a credential containing `:` or other reserved characters round-trips.
    /// The value is marked sensitive, so a `Debug` of the request never prints
    /// it.
    fn basic_authorization(&self) -> Result<http::HeaderValue, Error> {
        let id = form_encode(&self.client_id);
        let secret = form_encode(&self.client_secret);
        let token = base64_encode(format!("{id}:{secret}").as_bytes());
        let mut header = http::HeaderValue::from_str(&format!("Basic {token}")).map_err(|_| {
            Error::Config("OAuth client credentials contain characters not allowed in an HTTP header".to_string())
        })?;
        header.set_sensitive(true);
        Ok(header)
    }
}

/// Percent-encodes one credential the way `application/x-www-form-urlencoded`
/// does, for the RFC 6749 §2.3.1 Basic header.
fn form_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// Encodes bytes as standard (padded) base64.
///
/// Vendored here (like the client's own copy for `Basic` auth) rather than
/// pulling in a base64 dependency for one header.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::transport::{BoxFuture, TransportError};

    /// A transport that counts requests and replies from a scripted queue,
    /// so the tests can assert exactly how many token exchanges happened.
    #[derive(Debug)]
    struct ScriptedTransport {
        responses: Mutex<std::collections::VecDeque<(u16, String)>>,
        calls: AtomicUsize,
    }

    impl ScriptedTransport {
        fn new(responses: Vec<(u16, &str)>) -> Self {
            Self {
                responses: Mutex::new(
                    responses
                        .into_iter()
                        .map(|(status, body)| (status, body.to_string()))
                        .collect(),
                ),
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl Transport for ScriptedTransport {
        fn execute(
            &self,
            _request: http::Request<SdkBody>,
        ) -> BoxFuture<'_, Result<http::Response<SdkBody>, TransportError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let (status, body) = self
                .responses
                .lock()
                .expect("scripted responses lock")
                .pop_front()
                .expect("a scripted response for every token request");
            let response = http::Response::builder()
                .status(status)
                .body(SdkBody::from_bytes(body))
                .expect("statically valid response parts must build");
            Box::pin(PendingPolls::new(TRANSPORT_POLLS, Ok(response)))
        }
    }

    /// How many times the scripted transport yields before answering. Anything
    /// above zero forces the executor to poll the other pending callers, which
    /// is what makes the single-flight test exercise the fetch lock rather than
    /// letting the first caller run to completion uninterrupted.
    const TRANSPORT_POLLS: u32 = 1;

    /// How many times the test sleeper yields before resolving. Strictly more
    /// than the transport, so the deadline race in `send_with_retries` is always
    /// won by the transport and a test never trips the timeout.
    const SLEEP_POLLS: u32 = TRANSPORT_POLLS + 1;

    /// A future that returns `Pending` a fixed number of times before resolving.
    struct PendingPolls<T> {
        remaining: u32,
        value: Option<T>,
    }

    impl<T> PendingPolls<T> {
        fn new(remaining: u32, value: T) -> Self {
            Self {
                remaining,
                value: Some(value),
            }
        }
    }

    impl<T: Unpin> std::future::Future for PendingPolls<T> {
        type Output = T;

        fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
            let this = self.get_mut();
            if this.remaining > 0 {
                this.remaining -= 1;
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
            std::task::Poll::Ready(this.value.take().expect("polled after completion"))
        }
    }

    /// A sleeper that costs no wall time, but yields often enough to always
    /// lose the deadline race against the scripted transport.
    #[derive(Debug)]
    struct NoSleep;

    impl Sleep for NoSleep {
        fn sleep(&self, _duration: Duration) -> BoxFuture<'static, ()> {
            Box::pin(PendingPolls::new(SLEEP_POLLS, ()))
        }
    }

    fn provider() -> TokenProvider {
        let base = url::Url::parse("https://api.example.com/v1/").expect("a static base parses");
        TokenProvider::new(
            &base,
            "https://auth.example.com/oauth/token",
            "id".to_string(),
            "secret".to_string(),
            "read write",
        )
    }

    #[test]
    fn a_relative_token_url_resolves_against_the_base() {
        let base = url::Url::parse("https://api.example.com/v1/").expect("a static base parses");
        // Path-absolute replaces the base path; an absolute token URL is untouched.
        let relative = TokenProvider::new(&base, "/oauth/token", "id".into(), "secret".into(), "");
        assert_eq!(relative.token_url, "https://api.example.com/oauth/token");
        let absolute = TokenProvider::new(&base, "https://auth.example.com/t", "id".into(), "secret".into(), "");
        assert_eq!(absolute.token_url, "https://auth.example.com/t");
    }

    async fn token(provider: &TokenProvider, transport: &ScriptedTransport) -> Result<String, Error> {
        provider.access_token(transport, &NoSleep, 0, None, 1024 * 1024).await
    }

    /// A fresh token is fetched once and then served from the cache.
    #[tokio::test]
    async fn caches_a_fresh_token() {
        let transport = ScriptedTransport::new(vec![(200, r#"{"access_token":"first","expires_in":3600}"#)]);
        let provider = provider();
        assert_eq!(token(&provider, &transport).await.expect("first token"), "first");
        assert_eq!(token(&provider, &transport).await.expect("cached token"), "first");
        assert_eq!(transport.calls(), 1, "the cached token must not re-hit the endpoint");
    }

    /// A token inside the expiry buffer is refreshed rather than reused.
    #[tokio::test]
    async fn refreshes_inside_the_expiry_buffer() {
        let transport = ScriptedTransport::new(vec![
            (200, r#"{"access_token":"first","expires_in":30}"#),
            (200, r#"{"access_token":"second","expires_in":3600}"#),
        ]);
        let provider = provider();
        assert_eq!(token(&provider, &transport).await.expect("first token"), "first");
        // 30 s is inside the 120 s buffer, so the first token is already stale.
        assert_eq!(token(&provider, &transport).await.expect("refreshed token"), "second");
        assert_eq!(transport.calls(), 2);
    }

    /// A response without `expires_in` is cached for the life of the process.
    #[tokio::test]
    async fn caches_a_token_without_an_expiry() {
        let transport = ScriptedTransport::new(vec![(200, r#"{"access_token":"forever"}"#)]);
        let provider = provider();
        assert_eq!(token(&provider, &transport).await.expect("first token"), "forever");
        assert_eq!(token(&provider, &transport).await.expect("cached token"), "forever");
        assert_eq!(transport.calls(), 1);
    }

    /// Concurrent callers share one token request (single flight).
    #[tokio::test]
    async fn concurrent_callers_share_one_fetch() {
        let transport = ScriptedTransport::new(vec![(200, r#"{"access_token":"shared","expires_in":3600}"#)]);
        let provider = provider();
        let (a, b, c) = futures_util::future::join3(
            token(&provider, &transport),
            token(&provider, &transport),
            token(&provider, &transport),
        )
        .await;
        assert_eq!(a.expect("token a"), "shared");
        assert_eq!(b.expect("token b"), "shared");
        assert_eq!(c.expect("token c"), "shared");
        assert_eq!(
            transport.calls(),
            1,
            "concurrent callers must not stampede the endpoint"
        );
    }

    /// A rejected exchange surfaces as an API error and is not cached.
    #[tokio::test]
    async fn rejects_without_caching_the_failure() {
        let transport = ScriptedTransport::new(vec![
            (401, r#"{"error":"invalid_client"}"#),
            (200, r#"{"access_token":"recovered","expires_in":3600}"#),
        ]);
        let provider = provider();
        let error = token(&provider, &transport).await.expect_err("a 401 must fail");
        assert!(matches!(error, Error::Api(_)), "expected an API error, got {error:?}");
        assert_eq!(token(&provider, &transport).await.expect("retry"), "recovered");
        assert_eq!(transport.calls(), 2);
    }

    /// A token endpoint that sends headers and then stalls its body is cut off
    /// rather than hanging every request that waits on the credential.
    #[tokio::test]
    async fn a_stalled_token_body_times_out() {
        /// Answers with headers and a body that never produces a byte.
        #[derive(Debug)]
        struct StalledBodyTransport;

        impl Transport for StalledBodyTransport {
            fn execute(
                &self,
                _request: http::Request<SdkBody>,
            ) -> BoxFuture<'_, Result<http::Response<SdkBody>, TransportError>> {
                let stalled = futures_util::stream::pending::<Result<bytes::Bytes, TransportError>>();
                let response = http::Response::builder()
                    .status(200)
                    .body(SdkBody::from_stream(stalled))
                    .expect("statically valid response parts must build");
                Box::pin(std::future::ready(Ok(response)))
            }
        }

        let error = provider()
            .access_token(&StalledBodyTransport, &NoSleep, 0, None, 1024 * 1024)
            .await
            .expect_err("a token body that never arrives must not resolve");
        assert!(
            matches!(error, Error::Transport(_)),
            "expected a transport timeout, got {error:?}"
        );
    }

    /// A poisoned cache mutex is recovered from, never unwrapped.
    #[tokio::test]
    async fn survives_a_poisoned_cache() {
        let provider = std::sync::Arc::new(provider());
        let poisoner = std::sync::Arc::clone(&provider);
        let panicked = std::thread::spawn(move || {
            let _guard = poisoner.cache.lock().expect("uncontended lock");
            panic!("poison the cache mutex");
        })
        .join();
        assert!(panicked.is_err(), "the helper thread must have panicked");
        let transport = ScriptedTransport::new(vec![(200, r#"{"access_token":"after-poison","expires_in":3600}"#)]);
        assert_eq!(token(&provider, &transport).await.expect("token"), "after-poison");
    }
}
