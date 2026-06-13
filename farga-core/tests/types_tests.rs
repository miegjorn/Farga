use farga_core::types::{NodeKind, EdgeKind, Node};
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
