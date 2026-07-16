use std::collections::HashMap;

use hiddensteps_domain::EventSummary;

use crate::detector::action_key;

/// A weighted transition between two action keys — mirrors one row of the
/// `workflow_edges` table in `docs/design/07-database-schema.md` (minus the
/// surrogate integer node/pattern ids, which are assigned at the storage layer,
/// not here).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    pub weight: u32,
}

/// The observed graph of "what tends to follow what" across an event history:
/// nodes are distinct action keys (`{source_id}:{signal_type}`, same identity
/// `PatternDetector` uses), edges are observed direct transitions weighted by how
/// often they occurred. This is deliberately a *simple* directed graph (not a
/// Markov model with probabilities, not a hypergraph) — the Recommendation Engine
/// only needs "what usually follows this" to reason about a detected pattern's
/// surrounding context, not a predictive model.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowGraph {
    pub nodes: Vec<String>,
    pub edges: Vec<WorkflowEdge>,
}

impl WorkflowGraph {
    pub fn edges_from<'a>(&'a self, node: &str) -> impl Iterator<Item = &'a WorkflowEdge> + 'a {
        let node = node.to_string();
        self.edges.iter().filter(move |e| e.from == node)
    }
}

/// Builds the graph from a chronologically ordered event history — every
/// consecutive pair of events becomes (or reinforces) one edge.
pub fn build_workflow_graph(events: &[EventSummary]) -> WorkflowGraph {
    let mut node_set: Vec<String> = Vec::new();
    let mut seen_nodes: HashMap<String, ()> = HashMap::new();
    let mut edge_weights: HashMap<(String, String), u32> = HashMap::new();

    for pair in events.windows(2) {
        let from = action_key(&pair[0]);
        let to = action_key(&pair[1]);
        for key in [&from, &to] {
            if !seen_nodes.contains_key(key) {
                seen_nodes.insert(key.clone(), ());
                node_set.push(key.clone());
            }
        }
        *edge_weights.entry((from, to)).or_insert(0) += 1;
    }

    // A single-event history has one node and no edges; make sure that node is
    // still recorded even though the `windows(2)` loop above never runs for it.
    if events.len() == 1 {
        let key = action_key(&events[0]);
        node_set.push(key);
    }

    let edges = edge_weights
        .into_iter()
        .map(|((from, to), weight)| WorkflowEdge { from, to, weight })
        .collect();

    WorkflowGraph {
        nodes: node_set,
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiddensteps_domain::{PrivacyLevel, SignalType};
    use time::OffsetDateTime;

    fn event(source_id: &str, signal_type: SignalType) -> EventSummary {
        EventSummary::new(
            OffsetDateTime::now_utc(),
            source_id,
            signal_type,
            PrivacyLevel::WorkflowMetadata,
            serde_json::json!({}),
            None,
        )
    }

    #[test]
    fn builds_nodes_for_every_distinct_action_key() {
        let events = vec![
            event("jira", SignalType::AppActionEvent),
            event("excel", SignalType::AppActionEvent),
            event("slack", SignalType::AppFocusChange),
        ];
        let graph = build_workflow_graph(&events);
        assert_eq!(graph.nodes.len(), 3);
    }

    #[test]
    fn repeated_transitions_accumulate_edge_weight_instead_of_duplicating() {
        // jira->excel occurs twice (non-adjacently); excel->slack and slack->jira
        // each occur once — three distinct directed edges in total, one of them
        // weighted 2.
        let events = vec![
            event("jira", SignalType::AppActionEvent),
            event("excel", SignalType::AppActionEvent),
            event("slack", SignalType::AppFocusChange),
            event("jira", SignalType::AppActionEvent),
            event("excel", SignalType::AppActionEvent),
        ];
        let graph = build_workflow_graph(&events);
        assert_eq!(graph.edges.len(), 3);
        let jira_to_excel = graph
            .edges
            .iter()
            .find(|e| e.from == "jira:app_action_event" && e.to == "excel:app_action_event")
            .expect("edge should exist");
        assert_eq!(jira_to_excel.weight, 2);
    }

    #[test]
    fn edges_from_returns_only_outgoing_transitions() {
        let events = vec![
            event("jira", SignalType::AppActionEvent),
            event("excel", SignalType::AppActionEvent),
            event("slack", SignalType::AppFocusChange),
        ];
        let graph = build_workflow_graph(&events);
        let outgoing: Vec<&str> = graph
            .edges_from("excel:app_action_event")
            .map(|e| e.to.as_str())
            .collect();
        assert_eq!(outgoing, vec!["slack:app_focus_change"]);
    }

    #[test]
    fn single_event_history_yields_one_node_and_no_edges() {
        let events = vec![event("jira", SignalType::AppActionEvent)];
        let graph = build_workflow_graph(&events);
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn empty_history_yields_an_empty_graph() {
        let graph = build_workflow_graph(&[]);
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }
}
