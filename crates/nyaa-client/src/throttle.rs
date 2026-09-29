//! One shared gate in front of every nyaa.si request: a small concurrency
//! cap, a minimum gap between request starts, and - when nyaa.si answers 429
//! or 503 - a client-wide cooldown (honouring `Retry-After`) followed by a
//! retry, so concurrent searches/detail scrapes slow down together instead of
//! each hammering a server that already said no.

use std::time::Duration;

use tokio::sync::{Mutex, Semaphore};
use tokio::time::Instant;

/// Requests in flight at once.
const MAX_CONCURRENT: usize = 2;
/// Minimum gap between the start of two requests.
const MIN_GAP: Duration = Duration::from_millis(250);
/// Attempts per request (first try plus retries after a 429/503).
const MAX_ATTEMPTS: u32 = 4;
/// Longest cooldown we'll sit out, whatever `Retry-After` asks for.
const MAX_COOLDOWN: Duration = Duration::from_secs(30);

pub(crate) struct Throttle {
    permits: Semaphore,
    /// Earliest instant the next request may start (pacing + cooldown).
    next_start: Mutex<Instant>,
}

impl Throttle {
    pub(crate) fn new() -> Self {
        Self { permits: Semaphore::new(MAX_CONCURRENT), next_start: Mutex::new(Instant::now()) }
    }

    /// GETs `url` through the gate, retrying rate-limit responses with
    /// backoff. Any other HTTP error status is returned as an error.
    pub(crate) async fn get(&self, http: &reqwest::Client, url: &str) -> anyhow::Result<reqwest::Response> {
        let mut attempt = 1;
        loop {
            let _permit = self.permits.acquire().await.expect("semaphore never closed");
            self.wait_turn().await;

            let response = http.get(url).send().await?;
            let status = response.status();
            if (status == reqwest::StatusCode::TOO_MANY_REQUESTS || status == reqwest::StatusCode::SERVICE_UNAVAILABLE) && attempt < MAX_ATTEMPTS {
                let cooldown = retry_after(&response).unwrap_or_else(|| Duration::from_secs(1 << attempt)).min(MAX_COOLDOWN);
                tracing::warn!(url, %status, attempt, cooldown_ms = cooldown.as_millis() as u64, "nyaa.si is rate limiting, backing off");
                self.cool_down(cooldown).await;
                attempt += 1;
                continue;
            }
            return Ok(response.error_for_status()?);
        }
    }

    /// Blocks until this request's slot, and reserves the next one.
    async fn wait_turn(&self) {
        let start = {
            let mut next = self.next_start.lock().await;
            let start = (*next).max(Instant::now());
            *next = start + MIN_GAP;
            start
        };
        tokio::time::sleep_until(start).await;
    }

    /// Pushes every request's next slot out by `cooldown`.
    async fn cool_down(&self, cooldown: Duration) {
        let mut next = self.next_start.lock().await;
        *next = (*next).max(Instant::now() + cooldown);
    }
}

fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    let seconds: u64 = response.headers().get(reqwest::header::RETRY_AFTER)?.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(seconds))
}
