use crate::{ast::Node, domain::Context, hash::sha256};
use serde::Serialize;
use std::collections::BTreeMap;
pub type TraceFrame = std::sync::Arc<serde_json::Value>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Occurrence {
    pub context: Option<Context>,
    pub call_chain: Vec<TraceFrame>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_diagnostics_keep_bounded_distinct_provenance() {
        let mut diagnostics = Diagnostics::default();
        let node = Node {
            source: "script.txt".into(),
            line: 1,
            ..Node::default()
        };
        for caller in 0..260 {
            diagnostics.emit(
                &node,
                "scope",
                "invalid input".into(),
                ("error", "confirmed-rule-violation"),
                Some(&Context::new("planet")),
                &[serde_json::json!({"caller":caller}).into()],
            );
        }
        assert_eq!(diagnostics.counts(), (1, 0));
        assert_eq!(diagnostics.rows[0].occurrences.len(), 256);
        assert!(diagnostics.rows[0].occurrences_truncated);
    }
    #[test]
    fn sorting_does_not_break_diagnostic_identity() {
        let mut diagnostics = Diagnostics::default();
        for line in [2, 1] {
            let node = Node {
                source: "script.txt".into(),
                line,
                ..Node::default()
            };
            diagnostics.emit(
                &node,
                "scope",
                "invalid input".into(),
                ("error", "confirmed-rule-violation"),
                None,
                &[],
            );
        }
        diagnostics.sorted();
        let node = Node {
            source: "script.txt".into(),
            line: 2,
            ..Node::default()
        };
        diagnostics.emit(
            &node,
            "scope",
            "invalid input".into(),
            ("error", "confirmed-rule-violation"),
            None,
            &[serde_json::json!({"caller":"second"}).into()],
        );
        assert_eq!(diagnostics.rows.len(), 2);
        assert_eq!(diagnostics.rows[0].occurrences.len(), 1);
        assert_eq!(diagnostics.rows[1].occurrences.len(), 2);
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub key: String,
    pub severity: String,
    pub message: String,
    pub reason: serde_json::Value,
    pub context: Option<Context>,
    pub call_chain: Vec<TraceFrame>,
    pub diagnostic_id: String,
    pub occurrences: Vec<Occurrence>,
    pub occurrences_truncated: bool,
}
#[derive(Default)]
pub struct Diagnostics {
    pub rows: Vec<Diagnostic>,
    unique: BTreeMap<String, usize>,
}
impl Diagnostics {
    pub fn emit(
        &mut self,
        n: &Node,
        key: &str,
        message: String,
        classification: (&str, &str),
        ctx: Option<&Context>,
        chain: &[TraceFrame],
    ) {
        let (severity, reason) = classification;
        let identity = format!(
            "{}:{}:{}:{}:{}:{}",
            n.source, n.line, n.column, key, severity, message
        );
        let occurrence = Occurrence {
            context: ctx.cloned(),
            call_chain: chain.to_vec(),
        };
        let id = sha256(identity.as_bytes())[..20].to_string();
        if let Some(&index) = self.unique.get(&id) {
            let row = &mut self.rows[index];
            if !row.occurrences.contains(&occurrence) {
                if row.occurrences.len() < 256 {
                    row.occurrences.push(occurrence);
                } else {
                    row.occurrences_truncated = true;
                }
            }
            return;
        }
        self.unique.insert(id.clone(), self.rows.len());
        self.rows.push(Diagnostic{path:n.source.to_string(),line:n.line,column:n.column,key:key.into(),severity:severity.into(),message,
            reason:serde_json::json!({"code":reason,"classification_source":"Rust explicit contract","static_rule_violation":severity=="error","engine_fault_proven":false}),
            context:ctx.cloned(),call_chain:chain.to_vec(),diagnostic_id:id,
            occurrences:vec![occurrence],occurrences_truncated:false});
    }
    pub fn sorted(&mut self) {
        self.rows.sort_by(|a, b| {
            (&a.path, a.line, a.column, &a.key, &a.message)
                .cmp(&(&b.path, b.line, b.column, &b.key, &b.message))
        });
        self.unique = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, d)| (d.diagnostic_id.clone(), i))
            .collect();
    }
    pub fn counts(&self) -> (usize, usize) {
        (
            self.rows
                .iter()
                .filter(|d| d.severity == "error" || d.severity == "fatal")
                .count(),
            self.rows.iter().filter(|d| d.severity == "warning").count(),
        )
    }
    pub fn reasons(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for d in &self.rows {
            if d.severity == "warning" {
                *counts
                    .entry(d.reason["code"].as_str().unwrap_or("unknown").into())
                    .or_default() += 1;
            }
        }
        counts
    }
}
