use farga_core::types::{
    GovernanceContribution, FargaLayer, ReversibilityLevel, ImpactScope,
    LibrarianAssessment, LibrarianRouting, GovernanceStatus, NodeKind, EdgeKind, Node,
};
use chrono::Utc;

#[test]
fn node_kind_roundtrips_str() {
    assert_eq!(NodeKind::Artifact.as_str(), "Artifact");
    assert_eq!("Signal".parse::<NodeKind>().unwrap(), NodeKind::Signal);
}

#[test]
fn edge_kind_roundtrips_str() {
    assert_eq!(EdgeKind::SupersededBy.as_str(), "supersedes");
    assert_eq!("conflicts_with".parse::<EdgeKind>().unwrap(), EdgeKind::ConflictsWith);
}

#[test]
fn governance_contribution_round_trips() {
    let contrib = GovernanceContribution {
        title: "JWT Signing Pattern".into(),
        narrative: "Two projects converged on RS256.".into(),
        lessons: vec!["Use RS256 org-wide".into()],
        open_questions: vec![],
        involved_projects: vec!["auth-service".into(), "api-gateway".into()],
        concurrence: vec![],
        target_layer: FargaLayer::ProjectLevel,
        first_observed_at: Utc::now(),
        last_observed_at: Utc::now(),
        event_count: 3,
        reversibility: None,
        impact: None,
    };
    let json = serde_json::to_string(&contrib).unwrap();
    let back: GovernanceContribution = serde_json::from_str(&json).unwrap();
    assert_eq!(back.title, "JWT Signing Pattern");
    assert_eq!(back.event_count, 3);
    assert_eq!(back.target_layer, FargaLayer::ProjectLevel);
    assert!(back.reversibility.is_none());
}

#[test]
fn librarian_assessment_round_trips() {
    let assessment = LibrarianAssessment {
        reversibility: ReversibilityLevel::CostlyReversible,
        impact: ImpactScope::DomainWide,
        routing: LibrarianRouting::OpenGovernance,
        notes: Some("Broad Fondament impact".into()),
    };
    let json = serde_json::to_string(&assessment).unwrap();
    let back: LibrarianAssessment = serde_json::from_str(&json).unwrap();
    assert_eq!(back.impact, ImpactScope::DomainWide);
    assert_eq!(back.routing, LibrarianRouting::OpenGovernance);
}

#[test]
fn governance_status_variants_round_trip() {
    for status in [
        GovernanceStatus::Pending,
        GovernanceStatus::DirectIntegrate,
        GovernanceStatus::OpenGovernance,
        GovernanceStatus::Rejected,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let back: GovernanceStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }
}

#[test]
fn node_kind_governance_contribution_round_trips() {
    let kind = NodeKind::GovernanceContribution;
    assert_eq!(kind.as_str(), "GovernanceContribution");
    let back: NodeKind = "GovernanceContribution".parse().unwrap();
    assert_eq!(back, NodeKind::GovernanceContribution);
}
