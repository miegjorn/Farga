use async_trait::async_trait;
use crate::error::Result;
use crate::types::{Artifact, Signal};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub agent: String,
    pub capability: String,
    pub outcome: String,
    pub token_id: String,
}

#[async_trait]
pub trait FargaWriter: Send + Sync {
    async fn write_signals(&self, project: &str, signals: Vec<Signal>) -> Result<()>;
    async fn write_artifact(&self, artifact: Artifact) -> Result<()>;
    async fn write_audit(&self, entry: AuditEntry) -> Result<()>;
}

pub struct HttpFargaWriter {
    client: reqwest::Client,
    base_url: String,
}

impl HttpFargaWriter {
    pub fn new(base_url: String) -> Self {
        Self { client: reqwest::Client::new(), base_url }
    }
}

#[async_trait]
impl FargaWriter for HttpFargaWriter {
    async fn write_signals(&self, project: &str, signals: Vec<Signal>) -> Result<()> {
        let url = format!("{}/signals", self.base_url);
        self.client.post(&url)
            .json(&serde_json::json!({ "project": project, "signals": signals }))
            .send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(())
    }

    async fn write_artifact(&self, artifact: Artifact) -> Result<()> {
        let url = format!("{}/artifacts", self.base_url);
        self.client.post(&url).json(&artifact).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(())
    }

    async fn write_audit(&self, entry: AuditEntry) -> Result<()> {
        let url = format!("{}/audit", self.base_url);
        self.client.post(&url).json(&entry).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(())
    }
}
