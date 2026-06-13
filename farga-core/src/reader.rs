use async_trait::async_trait;
use crate::error::Result;
use crate::types::Signal;

#[derive(Debug, Clone)]
pub struct OrgContext { pub content: String }

#[derive(Debug, Clone)]
pub struct InitiativeContext { pub content: String }

#[derive(Debug, Clone)]
pub struct ProjectContext { pub content: String }

#[async_trait]
pub trait FargaReader: Send + Sync {
    async fn org_layer(&self, org: &str) -> Result<OrgContext>;
    async fn initiative_layer(&self, org: &str) -> Result<Vec<InitiativeContext>>;
    async fn project_layer(&self, project: &str) -> Result<ProjectContext>;
    async fn component_layer(&self, project: &str, path: &str) -> Result<ProjectContext>;
    async fn recent_signals(&self, project: &str, since_hours: u64) -> Result<Vec<Signal>>;
}

/// HTTP client implementation — connects to farga-server
pub struct HttpFargaReader {
    client: reqwest::Client,
    base_url: String,
}

impl HttpFargaReader {
    pub fn new(base_url: String) -> Self {
        Self { client: reqwest::Client::new(), base_url }
    }
}

#[async_trait]
impl FargaReader for HttpFargaReader {
    async fn org_layer(&self, org: &str) -> Result<OrgContext> {
        let url = format!("{}/context/org/{}", self.base_url, org);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        let content = resp.text().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(OrgContext { content })
    }

    async fn initiative_layer(&self, org: &str) -> Result<Vec<InitiativeContext>> {
        let url = format!("{}/context/initiatives/{}", self.base_url, org);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        let items: Vec<String> = resp.json().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(items.into_iter().map(|content| InitiativeContext { content }).collect())
    }

    async fn project_layer(&self, project: &str) -> Result<ProjectContext> {
        let url = format!("{}/context/project/{}", self.base_url, project);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        let content = resp.text().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(ProjectContext { content })
    }

    async fn component_layer(&self, project: &str, path: &str) -> Result<ProjectContext> {
        let url = format!("{}/context/component/{}/{}", self.base_url, project, path);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        let content = resp.text().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        Ok(ProjectContext { content })
    }

    async fn recent_signals(&self, project: &str, since_hours: u64) -> Result<Vec<Signal>> {
        let url = format!("{}/signals/recent?project={}&since={}h", self.base_url, project, since_hours);
        let resp = self.client.get(&url).send().await
            .map_err(|e| crate::error::FargaError::Http(e.to_string()))?;
        resp.json().await.map_err(|e| crate::error::FargaError::Http(e.to_string()))
    }
}
