use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    OrgLayer, InitiativeLayer, ProjectLayer, ComponentLayer,
    Artifact, Signal, Decision, Pattern, FondamentProposal, AuditEntry,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OrgLayer => "OrgLayer",
            Self::InitiativeLayer => "InitiativeLayer",
            Self::ProjectLayer => "ProjectLayer",
            Self::ComponentLayer => "ComponentLayer",
            Self::Artifact => "Artifact",
            Self::Signal => "Signal",
            Self::Decision => "Decision",
            Self::Pattern => "Pattern",
            Self::FondamentProposal => "FondamentProposal",
            Self::AuditEntry => "AuditEntry",
        }
    }
}

impl FromStr for NodeKind {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "OrgLayer" => Ok(Self::OrgLayer),
            "InitiativeLayer" => Ok(Self::InitiativeLayer),
            "ProjectLayer" => Ok(Self::ProjectLayer),
            "ComponentLayer" => Ok(Self::ComponentLayer),
            "Artifact" => Ok(Self::Artifact),
            "Signal" => Ok(Self::Signal),
            "Decision" => Ok(Self::Decision),
            "Pattern" => Ok(Self::Pattern),
            "FondamentProposal" => Ok(Self::FondamentProposal),
            "AuditEntry" => Ok(Self::AuditEntry),
            _ => Err(format!("unknown NodeKind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    ContributesTo, IsPartOf, SupersededBy, ConflictsWith,
    DerivedFrom, ReferencedBy, PromotesTo,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ContributesTo => "contributes_to",
            Self::IsPartOf => "is_part_of",
            Self::SupersededBy => "supersedes",
            Self::ConflictsWith => "conflicts_with",
            Self::DerivedFrom => "derived_from",
            Self::ReferencedBy => "referenced_by",
            Self::PromotesTo => "promotes_to",
        }
    }
}

impl FromStr for EdgeKind {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "contributes_to" => Ok(Self::ContributesTo),
            "is_part_of" => Ok(Self::IsPartOf),
            "supersedes" => Ok(Self::SupersededBy),
            "conflicts_with" => Ok(Self::ConflictsWith),
            "derived_from" => Ok(Self::DerivedFrom),
            "referenced_by" => Ok(Self::ReferencedBy),
            "promotes_to" => Ok(Self::PromotesTo),
            _ => Err(format!("unknown EdgeKind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub address: Option<String>,
    pub project: Option<String>,
    pub component: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub stale: bool,
}

impl Node {
    pub fn new(kind: NodeKind, project: Option<String>, content: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            kind,
            address: None,
            project,
            component: None,
            title: None,
            content,
            created_at: now,
            updated_at: now,
            stale: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_id: String,
    pub to_id: String,
    pub kind: EdgeKind,
    pub weight: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub project: String,
    pub content: String,
    pub source: String,         // "concierge" | "session" | "manual"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub project: String,
    pub title: String,
    pub content: String,
    pub session_id: Option<String>,
    pub kind: String,           // "adr" | "implementation-notes" | "test-plan" | ...
}
