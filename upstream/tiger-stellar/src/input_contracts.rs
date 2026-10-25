//! Necessary THIS-type constraints for uninvoked scripts. This is a contract
//! check, not discovery of an engine entry, caller, ROOT or runtime target.
use crate::{
    ast::Node,
    domain::{Context, Types},
    engine::Engine,
    rules::Rule,
};
use std::collections::BTreeSet;

struct Constraints {
    candidates: BTreeSet<String>,
    evidence: Vec<serde_json::Value>,
    active: BTreeSet<(bool, String)>,
    remaining: usize,
    exhausted: bool,
}
enum GuardScan {
    Absent,
    Present,
    Limit,
}
impl Constraints {
    fn accept(&mut self, node: &Node, label: &str, rule: &Rule) {
        if rule.input.is_empty()
            || rule
                .input
                .iter()
                .any(|s| matches!(s.as_str(), "all" | "various" | "?"))
        {
            return;
        }
        self.candidates.retain(|s| rule.input.contains(s));
        self.evidence.push(serde_json::json!({"path":node.source,"line":node.line,"rule":label,"accepted_input_types":rule.input}));
    }
}
impl Engine {
    fn collect_input_constraints(
        &self,
        nodes: &[Node],
        effect: bool,
        constraints: &mut Constraints,
        depth: usize,
    ) {
        if depth >= 64 {
            constraints.exhausted = true;
            return;
        }
        for node in nodes {
            if constraints.remaining == 0 {
                constraints.exhausted = true;
                return;
            }
            constraints.remaining -= 1;
            if node.is_memory_directive() || node.key.starts_with('@') {
                continue;
            }
            let lower = node.key.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "and"
                    | "or"
                    | "not"
                    | "nor"
                    | "nand"
                    | "hidden_trigger"
                    | "hidden_effect"
                    | "limit"
            ) {
                // Scope-type guarded alternatives can intentionally support
                // different receivers. Do not intersect their branch contracts.
                match scan_scope_guard(node.body()) {
                    GuardScan::Absent => {
                        self.collect_input_constraints(node.body(), effect, constraints, depth + 1)
                    }
                    GuardScan::Present => {}
                    GuardScan::Limit => constraints.exhausted = true,
                }
                continue;
            }
            let scope = node
                .key
                .trim_end_matches('?')
                .split('.')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            if scope == "this" && !node.key.contains('.') {
                self.collect_input_constraints(node.body(), effect, constraints, depth + 1);
                continue;
            }
            if let Some(rule) = self.rules.scopes.get(&scope) {
                // Only the initial receiver's contract applies to THIS. The
                // nested body is validated later in the scope link's output.
                constraints.accept(node, &scope, rule);
                continue;
            }
            if node.key == "exists" {
                let head = node
                    .scalar()
                    .split('.')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if let Some(rule) = self.rules.scopes.get(&head) {
                    constraints.accept(node, &head, rule);
                }
                continue;
            }
            let rules = if effect {
                &self.rules.effects
            } else {
                &self.rules.triggers
            };
            if let Some(rule) = rules.get(&node.key) {
                constraints.accept(node, &node.key, rule);
                // Parameter blocks, selectors and callbacks change receivers;
                // they are not unconditional constraints on the outer THIS.
                continue;
            }
            let key = (effect, node.key.clone());
            if let Some(definition) = self.program.scripts.get(&key)
                && definition.macro_parts.is_none()
                && constraints.active.insert(key.clone())
            {
                match scan_scope_guard(definition.body()) {
                    GuardScan::Absent => self.collect_input_constraints(
                        definition.body(),
                        effect,
                        constraints,
                        depth + 1,
                    ),
                    GuardScan::Present => {}
                    GuardScan::Limit => constraints.exhausted = true,
                }
                constraints.active.remove(&key);
            }
        }
    }
    pub(crate) fn inferred_script_context(
        &mut self,
        effect: bool,
        name: &str,
        node: &Node,
    ) -> Context {
        let mut context = Context::unknown_caller();
        if node.macro_parts.is_some() {
            return context;
        }
        let mut constraints = Constraints {
            candidates: crate::rules::SCOPES
                .split_whitespace()
                .filter(|s| !matches!(*s, "all" | "various" | "?"))
                .map(String::from)
                .collect(),
            evidence: vec![],
            active: BTreeSet::from([(effect, name.into())]),
            remaining: 4096,
            exhausted: false,
        };
        match scan_scope_guard(node.body()) {
            GuardScan::Absent => {
                self.collect_input_constraints(node.body(), effect, &mut constraints, 0)
            }
            GuardScan::Present => return context,
            GuardScan::Limit => constraints.exhausted = true,
        }
        if constraints.evidence.is_empty() && !constraints.exhausted {
            return context;
        }
        let status = if constraints.exhausted {
            "analysis-limit"
        } else if constraints.candidates.is_empty() {
            "conflicting-input-constraints"
        } else {
            "inferred-necessary-input"
        };
        if status == "inferred-necessary-input" {
            context.this = Types {
                known: constraints.candidates,
                unknown: false,
            };
            context.object = format!("inferred-script:{name}.this");
            context.root_object = "unknown-caller.root".into();
            // In a nested caller THIS and ROOT need not be the same object.
            // No FROM/PREV facts or target availability are inferred here.
            self.chain.push(serde_json::json!({"kind":"inferred-script-input","definition":name,"path":node.source,"line":node.line,"input_types":context.this,"caller_proven":false}).into());
        } else {
            self.gap(
                node,
                format!("script input contract remains unknown: {name} ({status})"),
                if constraints.exhausted {
                    "analysis-limit"
                } else {
                    "script-input-contract-conflict"
                },
                None,
            );
            if constraints.exhausted {
                self.complete = false;
            }
        }
        self.inferred_script_inputs.push(serde_json::json!({
            "name":name,"effect":effect,"path":node.source,"line":node.line,
            "status":status,"input_types":context.this,"root_types":context.root,
            "evidence":constraints.evidence,"complete_contract":false,
            "caller_proven":false,"engine_entry_registered":false,"runtime_availability_proven":false
        }));
        context
    }
}
fn scan_scope_guard(nodes: &[Node]) -> GuardScan {
    let mut pending = nodes.iter().collect::<Vec<_>>();
    let mut remaining = 4096;
    while let Some(node) = pending.pop() {
        if remaining == 0 {
            return GuardScan::Limit;
        }
        if node.key == "is_scope_type" {
            return GuardScan::Present;
        }
        remaining -= 1;
        pending.extend(node.body());
    }
    GuardScan::Absent
}
