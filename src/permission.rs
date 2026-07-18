use crate::intent::IntentResolution;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorRisk {
    PublicQuery,
    PublicIdentifier,
    SensitiveContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorDisclosure {
    pub id: String,
    pub service: String,
    pub destinations: Vec<String>,
    pub outbound_data: String,
    pub purpose: String,
    pub risk: ConnectorRisk,
    pub automatic_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub schema_version: String,
    pub plan_id: String,
    pub query_preview: String,
    pub intent: IntentResolution,
    pub connectors: Vec<ConnectorDisclosure>,
    pub permission_required: bool,
    pub automatic_eligible: bool,
    pub disclosure: String,
}

pub fn build_execution_plan(
    query: &str,
    intent: IntentResolution,
    mut connectors: Vec<ConnectorDisclosure>,
) -> ExecutionPlan {
    connectors.sort_by(|left, right| left.id.cmp(&right.id));
    connectors.dedup_by(|left, right| left.id == right.id);
    let permission_required = !connectors.is_empty();
    let automatic_eligible = permission_required
        && intent.clarification.is_none()
        && connectors
            .iter()
            .all(|connector| connector.automatic_eligible)
        && connectors
            .iter()
            .all(|connector| connector.risk == ConnectorRisk::PublicQuery);
    let canonical = serde_json::to_vec(&("inquiry.execution-plan/v1", query, &intent, &connectors))
        .expect("execution plan components serialize");
    let plan_id = format!("sha256:{:x}", Sha256::digest(canonical));
    let disclosure = if connectors.is_empty() {
        "No public connector request is planned.".into()
    } else {
        format!(
            "Approval permits this run to send the displayed query or minimized parameters to {} reviewed public service(s). It does not grant background access, account access, or future-run permission.",
            connectors.len()
        )
    };
    ExecutionPlan {
        schema_version: "inquiry.execution-plan/v1".into(),
        plan_id,
        query_preview: query.into(),
        intent,
        connectors,
        permission_required,
        automatic_eligible,
        disclosure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent;

    #[test]
    fn plan_ids_are_deterministic_and_connector_specific() {
        let connector = ConnectorDisclosure {
            id: "wikipedia".into(),
            service: "Wikipedia".into(),
            destinations: vec!["en.wikipedia.org".into()],
            outbound_data: "public query".into(),
            purpose: "encyclopedia discovery".into(),
            risk: ConnectorRisk::PublicQuery,
            automatic_eligible: true,
        };
        let one = build_execution_plan(
            "public question",
            intent::resolve("public question"),
            vec![connector.clone()],
        );
        let two = build_execution_plan(
            "public question",
            intent::resolve("public question"),
            vec![connector],
        );
        assert_eq!(one.plan_id, two.plan_id);
        assert!(one.permission_required);
        assert!(one.automatic_eligible);
    }
}
