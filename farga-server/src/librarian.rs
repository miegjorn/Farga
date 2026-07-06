use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Local verdict type — carries the raw strings stored in the DB.
/// Distinct from farga_core::types::LibrarianAssessment which uses enums.
#[derive(Debug, Deserialize, Serialize)]
pub struct LibrarianVerdict {
    pub reversibility: String, // "FullyReversible"|"EffectsLinger"|"CostlyReversible"|"Irreversible"
    pub impact: String,        // "Contained"|"CrossProject"|"DomainWide"|"OrgWide"
    pub routing: String,       // "ProjectLevel"|"InitiativeLevel"|"OrgLevel"
    pub notes: String,
}

/// Minimal shape of GovernanceContribution stored in nodes.content.
/// Only the fields the librarian needs for its prompt.
#[derive(Debug, Deserialize)]
struct ContribSnapshot {
    pub title: String,
    pub narrative: String,
    pub lessons: Vec<String>,
    pub involved_projects: Vec<String>,
}

/// Entry point — spawn this with `tokio::spawn(run_librarian(pool))`.
pub async fn run_librarian(pool: SqlitePool) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        if let Err(e) = assess_pending(&pool).await {
            tracing::error!("librarian: assessment cycle failed: {}", e);
        }
    }
}

async fn assess_pending(pool: &SqlitePool) -> Result<()> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT ga.node_id, n.content
         FROM governance_assessments ga
         JOIN nodes n ON ga.node_id = n.id
         WHERE ga.status = 'pending'
         LIMIT 10",
    )
    .fetch_all(pool)
    .await?;

    for (node_id, content) in rows {
        match assess_contribution(&content).await {
            Ok(verdict) => {
                if let Err(e) = update_assessment(pool, &node_id, &verdict).await {
                    tracing::error!("librarian: failed to update assessment for {}: {}", node_id, e);
                } else {
                    tracing::info!("librarian: assessed node {} -> routing={}", node_id, verdict.routing);
                }
            }
            Err(e) => {
                tracing::error!("librarian: LLM call failed for node {}: {}", node_id, e);
            }
        }
    }

    Ok(())
}

async fn assess_contribution(content: &str) -> Result<LibrarianVerdict> {
    let snap: ContribSnapshot = serde_json::from_str(content)?;

    let lessons = snap.lessons.join("; ");
    let projects = snap.involved_projects.join(", ");

    let prompt = format!(
        r#"You are the Farga librarian for the Occitan multi-agent system. You assess governance contributions to determine their reversibility, impact scope, and routing level.

Contribution title: {title}
Narrative: {narrative}
Lessons learned: {lessons}
Involved projects: {projects}

Respond ONLY with a JSON object, no markdown:
{{
  "reversibility": "FullyReversible"|"EffectsLinger"|"CostlyReversible"|"Irreversible",
  "impact": "Contained"|"CrossProject"|"DomainWide"|"OrgWide",
  "routing": "ProjectLevel"|"InitiativeLevel"|"OrgLevel",
  "notes": "one sentence explaining your assessment"
}}

Definitions:
- FullyReversible: decision can be undone with no lasting effects
- EffectsLinger: reversible but some effects persist (e.g. learned behaviors, written docs)
- CostlyReversible: reversible but requires significant rework
- Irreversible: cannot be undone (e.g. data deletion, published decisions)
- Contained: affects only one project
- CrossProject: affects 2-3 specific projects
- DomainWide: affects an entire domain/team
- OrgWide: affects the entire organization"#,
        title = snap.title,
        narrative = snap.narrative,
        lessons = lessons,
        projects = projects,
    );

    let model = "claude-sonnet-4-6";

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 256,
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    let resp_json: serde_json::Value = resp.json().await?;

    let text = resp_json["content"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("unexpected Anthropic response shape: {:?}", resp_json))?;

    let verdict: LibrarianVerdict = serde_json::from_str(text.trim())?;
    Ok(verdict)
}

async fn update_assessment(
    pool: &SqlitePool,
    node_id: &str,
    verdict: &LibrarianVerdict,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE governance_assessments
         SET status = 'assessed',
             reversibility = ?,
             impact = ?,
             routing = ?,
             notes = ?,
             updated_at = ?
         WHERE node_id = ?",
    )
    .bind(&verdict.reversibility)
    .bind(&verdict.impact)
    .bind(&verdict.routing)
    .bind(&verdict.notes)
    .bind(&now)
    .bind(node_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn librarian_verdict_deserializes_correctly() {
        let json = r#"{
            "reversibility": "EffectsLinger",
            "impact": "CrossProject",
            "routing": "InitiativeLevel",
            "notes": "This decision affects multiple projects and its documentation effects will persist."
        }"#;
        let verdict: LibrarianVerdict = serde_json::from_str(json).unwrap();
        assert_eq!(verdict.reversibility, "EffectsLinger");
        assert_eq!(verdict.impact, "CrossProject");
        assert_eq!(verdict.routing, "InitiativeLevel");
        assert!(!verdict.notes.is_empty());
    }

    #[sqlx::test]
    async fn update_assessment_sql_is_correct(pool: SqlitePool) {
        // Create schema
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                address TEXT,
                project TEXT,
                component TEXT,
                title TEXT,
                content TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                stale INTEGER DEFAULT 0 NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS governance_assessments (
                id TEXT PRIMARY KEY,
                node_id TEXT NOT NULL REFERENCES nodes(id),
                status TEXT NOT NULL DEFAULT 'pending',
                reversibility TEXT,
                impact TEXT,
                routing TEXT,
                notes TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert seed data
        let node_id = "test-node-001";
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO nodes (id, kind, created_at, updated_at, stale) VALUES (?, 'GovernanceContribution', ?, ?, 0)",
        )
        .bind(node_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO governance_assessments (id, node_id, status, created_at, updated_at)
             VALUES ('assess-001', ?, 'pending', ?, ?)",
        )
        .bind(node_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        // Run the function under test
        let verdict = LibrarianVerdict {
            reversibility: "CostlyReversible".into(),
            impact: "DomainWide".into(),
            routing: "OrgLevel".into(),
            notes: "Test note.".into(),
        };
        update_assessment(&pool, node_id, &verdict).await.unwrap();

        // Verify the row was updated
        let row: (String, String, String, String, String) = sqlx::query_as(
            "SELECT status, reversibility, impact, routing, notes FROM governance_assessments WHERE node_id = ?",
        )
        .bind(node_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.0, "assessed");
        assert_eq!(row.1, "CostlyReversible");
        assert_eq!(row.2, "DomainWide");
        assert_eq!(row.3, "OrgLevel");
        assert_eq!(row.4, "Test note.");
    }
}
