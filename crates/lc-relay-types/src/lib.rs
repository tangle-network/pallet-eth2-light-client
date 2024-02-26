use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, typed_builder::TypedBuilder)]
pub struct WebbRetryClient {
	/// The inner client
	#[builder(setter(into))]
	inner: Client,
	/// How many connection `TimedOut` should be retried.
	#[builder(setter(into))]
	timeout_retries: u32,
	/// How long to wait initially
	#[builder(setter(into))]
	initial_backoff: u64,
}

impl WebbRetryClient {
	pub async fn get(&self, url: &str) -> anyhow::Result<String> {
		let mut timeout_retries: u32 = 0;
		loop {
			let err;

			{
				let resp = self.inner.get(url).send().await;
				match resp {
					Ok(val) => return Ok(val.text().await?),
					Err(err_) => err = err_,
				}
			}
			// initial backoff before retrying
			tokio::time::sleep(Duration::from_millis(self.initial_backoff)).await;

			if timeout_retries < self.timeout_retries && err.is_timeout() {
				timeout_retries += 1;
				tracing::error!(err = ?err, "retrying due to spurious network");
				continue
			} else {
				return Err(err.into())
			}
		}
	}

	pub async fn post(&self, url: &str, body: Value) -> anyhow::Result<String> {
		let mut timeout_retries: u32 = 0;
		loop {
			let err;

			{
				let resp = self.inner.post(url).json(&body).send().await;
				match resp {
					Ok(val) => return Ok(val.text().await?),
					Err(err_) => err = err_,
				}
			}
			// initial backoff before retrying
			tokio::time::sleep(Duration::from_millis(self.initial_backoff)).await;

			if timeout_retries < self.timeout_retries && err.is_timeout() {
				timeout_retries += 1;
				tracing::error!(err = ?err, "retrying due to spurious network");
				continue
			} else {
				return Err(err.into())
			}
		}
	}
}
