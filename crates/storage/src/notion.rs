//! Read-heavy Notion connector. One tenant gets one short-lived connector
//! instance; the bearer token is never placed in payloads, logs, or hashes.
use engrave_contracts::TenantId;
use engrave_core::{ConnectorCursor, ExternalObject};
use reqwest::StatusCode;
use serde_json::Value;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

#[derive(Clone)]
pub struct NotionConnector {
    tenant_id: TenantId,
    token: String,
    endpoint: String,
    client: reqwest::Client,
    max_retries: u8,
    revoked: Arc<AtomicBool>,
}

impl NotionConnector {
    pub fn new(tenant_id: TenantId, token: String, endpoint: String) -> Result<Self, String> {
        Self::new_with_timeout(tenant_id, token, endpoint, Duration::from_secs(20))
    }

    pub fn new_with_timeout(
        tenant_id: TenantId,
        token: String,
        endpoint: String,
        timeout: Duration,
    ) -> Result<Self, String> {
        if token.trim().is_empty() {
            return Err("Notion credential is empty".into());
        }
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| "invalid connector HTTP client")?;
        Ok(Self {
            tenant_id,
            token,
            endpoint,
            client,
            max_retries: 3,
            revoked: Arc::new(AtomicBool::new(false)),
        })
    }
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }
    /// Fail closed for the next request after an operator or credential
    /// authority revokes this short-lived connector instance. The worker must
    /// discard the instance and resolve a fresh tenant-bound credential.
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }
    pub async fn list(
        &self,
        cursor: &ConnectorCursor,
    ) -> Result<(ConnectorCursor, Vec<ExternalObject>), String> {
        if self.revoked.load(Ordering::Acquire) {
            return Err("connector credential revoked".into());
        }
        let mut attempts = 0u8;
        loop {
            if self.revoked.load(Ordering::Acquire) {
                return Err("connector credential revoked".into());
            }
            let mut request = self
                .client
                .post(format!("{}/v1/search", self.endpoint.trim_end_matches('/')))
                .bearer_auth(&self.token)
                .header("Notion-Version", "2022-06-28")
                .json(&serde_json::json!({"page_size":100}));
            if let Some(cursor) = &cursor.0 {
                request = request.json(&serde_json::json!({"page_size":100,"start_cursor":cursor}));
            }
            let response = request.send().await.map_err(|e| {
                if e.is_timeout() {
                    "connector timeout"
                } else {
                    "connector unavailable"
                }
            })?;
            if response.status() == StatusCode::TOO_MANY_REQUESTS
                || response.status().is_server_error()
            {
                if attempts >= self.max_retries {
                    return Err(format!(
                        "connector retry budget exhausted: HTTP {}",
                        response.status()
                    ));
                }
                let delay = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1)
                    .min(8);
                tokio::time::sleep(Duration::from_secs(delay * (attempts as u64 + 1))).await;
                attempts += 1;
                continue;
            }
            if !response.status().is_success() {
                return Err(format!(
                    "connector request rejected: HTTP {}",
                    response.status()
                ));
            }
            let body: Value = response
                .json()
                .await
                .map_err(|_| "invalid connector response")?;
            let next = body
                .get("next_cursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let objects = body
                .get("results")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|object| {
                    let id = object.get("id")?.as_str()?.to_owned();
                    let title = object
                        .get("url")
                        .and_then(Value::as_str)
                        .unwrap_or(&id)
                        .to_owned();
                    let permissions = object
                        .get("permissions")
                        .and_then(Value::as_array)
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default();
                    Some(ExternalObject {
                        external_id: id,
                        title,
                        content: serde_json::to_string(&object).ok()?,
                        permissions,
                        deleted: false,
                    })
                })
                .collect();
            return Ok((ConnectorCursor(next), objects));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use uuid::Uuid;
    #[test]
    fn credentials_are_required_and_tenant_bound() {
        let tenant = TenantId::new(Uuid::from_u128(7));
        assert!(NotionConnector::new(tenant, "".into(), "http://localhost".into()).is_err());
        let connector =
            NotionConnector::new(tenant, "opaque-secret".into(), "http://localhost".into())
                .unwrap();
        assert_eq!(connector.tenant_id(), tenant);
    }

    async fn response_server(
        statuses: Vec<u16>,
        body: &'static str,
    ) -> (String, tokio::task::JoinHandle<usize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let observed_calls = Arc::clone(&calls);
        let handle = tokio::spawn(async move {
            for status in statuses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0_u8; 4096];
                let _ = socket.read(&mut request).await;
                observed_calls.fetch_add(1, Ordering::SeqCst);
                let retry = if status == 429 || status >= 500 {
                    "Retry-After: 0\r\n"
                } else {
                    ""
                };
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\n{retry}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            calls.load(Ordering::SeqCst)
        });
        (endpoint, handle)
    }

    #[tokio::test]
    async fn transient_connector_failures_retry_with_a_hard_cap() {
        let (endpoint, server) = response_server(vec![500, 429, 200], r#"{"results":[]}"#).await;
        let connector = NotionConnector::new(
            TenantId::new(Uuid::from_u128(8)),
            "secret-a".into(),
            endpoint,
        )
        .unwrap();
        let (_, objects) = connector.list(&ConnectorCursor(None)).await.unwrap();
        assert!(objects.is_empty());
        assert_eq!(server.await.unwrap(), 3);
    }

    #[tokio::test]
    async fn authorization_failure_is_not_retried() {
        let (endpoint, server) = response_server(vec![401], r#"{"error":"revoked"}"#).await;
        let connector = NotionConnector::new(
            TenantId::new(Uuid::from_u128(9)),
            "secret-b".into(),
            endpoint,
        )
        .unwrap();
        let error = connector.list(&ConnectorCursor(None)).await.unwrap_err();
        assert!(error.contains("HTTP 401"));
        assert_eq!(server.await.unwrap(), 1);
    }

    #[tokio::test]
    async fn local_revocation_fails_closed_before_network_access() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_millis(50), listener.accept())
                .await
                .is_err()
        });
        let connector = NotionConnector::new(
            TenantId::new(Uuid::from_u128(11)),
            "secret-revoked-locally".into(),
            endpoint,
        )
        .unwrap();
        connector.revoke();
        let error = connector.list(&ConnectorCursor(None)).await.unwrap_err();
        assert_eq!(error, "connector credential revoked");
        assert!(server.await.unwrap());
    }

    #[tokio::test]
    async fn connector_timeout_is_bounded() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        });
        let connector = NotionConnector::new_with_timeout(
            TenantId::new(Uuid::from_u128(10)),
            "secret-c".into(),
            endpoint,
            Duration::from_millis(10),
        )
        .unwrap();
        let error = connector.list(&ConnectorCursor(None)).await.unwrap_err();
        assert_eq!(error, "connector timeout");
        server.await.unwrap();
    }
}
