//! Finite Boolean implication under hull potential_country assumptions.
//! This does not prove technology, resources, runtime or automatic design.
use crate::{ast::Node, engine::Engine};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Expr {
    Constant(bool),
    Atom(String),
    Not(Box<Expr>),
    All(Vec<Expr>),
    Any(Vec<Expr>),
}
fn not(e: Expr) -> Expr {
    match e {
        Expr::Constant(v) => Expr::Constant(!v),
        Expr::Not(x) => *x,
        other => Expr::Not(Box::new(other)),
    }
}
fn combine(any: bool, items: impl IntoIterator<Item = Expr>) -> Expr {
    let mut parts = BTreeSet::new();
    for e in items {
        match e {
            Expr::Constant(v) if v == any => return Expr::Constant(any),
            Expr::Constant(_) => {}
            Expr::Any(v) if any => parts.extend(v),
            Expr::All(v) if !any => parts.extend(v),
            other => {
                parts.insert(other);
            }
        }
    }
    if parts.iter().any(|e| parts.contains(&not(e.clone()))) {
        return Expr::Constant(any);
    }
    if parts.is_empty() {
        return Expr::Constant(!any);
    }
    if parts.len() == 1 {
        return parts.pop_first().unwrap();
    }
    if any {
        Expr::Any(parts.into_iter().collect())
    } else {
        Expr::All(parts.into_iter().collect())
    }
}
fn atoms(e: &Expr, out: &mut BTreeSet<String>) {
    match e {
        Expr::Atom(a) => {
            out.insert(a.clone());
        }
        Expr::Not(e) => atoms(e, out),
        Expr::All(v) | Expr::Any(v) => {
            for e in v {
                atoms(e, out);
            }
        }
        _ => {}
    }
}
fn evaluate(e: &Expr, values: &BTreeMap<String, bool>) -> bool {
    match e {
        Expr::Constant(v) => *v,
        Expr::Atom(a) => values[a],
        Expr::Not(e) => !evaluate(e, values),
        Expr::All(v) => v.iter().all(|e| evaluate(e, values)),
        Expr::Any(v) => v.iter().any(|e| evaluate(e, values)),
    }
}

#[derive(Debug, Serialize)]
pub struct RequiredSetProof {
    pub hull: String,
    pub component_set: String,
    pub status: &'static str,
    pub symbols: usize,
    pub reason: Option<&'static str>,
    pub counterexample: Option<BTreeMap<String, bool>>,
    pub assumption: &'static str,
    pub proof_domain: &'static str,
}
struct Solver<'a> {
    engine: &'a Engine,
    hull: &'a Node,
    serial: u64,
    fuel: usize,
    depth: usize,
    depth_limited: bool,
    truncated: bool,
    active: BTreeSet<String>,
}
impl Solver<'_> {
    fn opaque(&mut self, n: &Node, path: &[String]) -> Expr {
        self.serial += 1;
        Expr::Atom(
            serde_json::to_string(&(
                "unsupported-occurrence",
                self.serial,
                path,
                &n.key,
                &n.value,
            ))
            .unwrap(),
        )
    }
    fn body(&mut self, nodes: &[Node], path: &[String]) -> Expr {
        combine(
            false,
            nodes.iter().map(|n| self.node(n, path)).collect::<Vec<_>>(),
        )
    }
    fn node(&mut self, n: &Node, path: &[String]) -> Expr {
        if n.is_memory_directive() {
            return Expr::Constant(true);
        }
        if self.depth >= 64 {
            self.truncated = true;
            self.depth_limited = true;
            return self.opaque(n, path);
        }
        if self.fuel == 0 {
            self.truncated = true;
            return self.opaque(n, path);
        }
        self.fuel -= 1;
        self.depth += 1;
        let answer = self.node_inner(n, path);
        self.depth -= 1;
        answer
    }
    fn node_inner(&mut self, n: &Node, path: &[String]) -> Expr {
        let key = n.key.to_ascii_lowercase();
        if n.op == "=" && n.children.is_none() {
            if key == "always" && matches!(n.scalar(), "yes" | "no") {
                return Expr::Constant(n.scalar() == "yes");
            }
            if path.is_empty()
                && matches!(key.as_str(), "is_ship_size" | "is_ship_class")
                && !n.scalar().contains(['$', '@', ':', '['])
            {
                let known = if key == "is_ship_size" {
                    Some(self.hull.key.as_str())
                } else {
                    self.hull.val("class")
                };
                if let Some(known) = known {
                    return Expr::Constant(known == n.scalar());
                }
            }
        }
        if n.children.is_some()
            && matches!(
                key.as_str(),
                "and" | "or" | "not" | "nor" | "nand" | "hidden_trigger"
            )
        {
            let any = matches!(key.as_str(), "or" | "nor");
            let e = combine(
                any,
                n.body()
                    .iter()
                    .map(|n| self.node(n, path))
                    .collect::<Vec<_>>(),
            );
            return if matches!(key.as_str(), "not" | "nor" | "nand") {
                not(e)
            } else {
                e
            };
        }
        if n.op == "="
            && n.children.is_none()
            && matches!(n.scalar(), "yes" | "no")
            && let Some(def) = self
                .engine
                .program
                .scripts
                .get(&(false, n.key.clone()))
                .cloned()
            && def.macro_parts.is_none()
            && self.active.insert(n.key.clone())
        {
            let e = self.body(def.body(), path);
            self.active.remove(&n.key);
            return if n.scalar() == "no" { not(e) } else { e };
        }
        if n.children.is_some() && self.engine.rules.scopes.contains_key(&key) {
            let mut next = path.to_vec();
            next.push(key.clone());
            let exists = if path.is_empty() && key == "from" {
                Expr::Constant(true)
            } else {
                Expr::Atom(serde_json::to_string(&("scope-exists", &next)).unwrap())
            };
            let body = self.body(n.body(), &next);
            return combine(false, [exists, body]);
        }
        if n.children.is_none()
            && !key.contains("random")
            && let Some(rule) = self.engine.rules.triggers.get(&key)
        {
            // yes/no are complements only for a Boolean contract. For a flag
            // name, literal "yes" and literal "no" are different names.
            let inverse = rule.boolean && n.op == "=" && n.scalar() == "no";
            let value = if rule.boolean && n.op == "=" && matches!(n.scalar(), "yes" | "no") {
                "yes"
            } else {
                n.scalar()
            };
            let atom = Expr::Atom(serde_json::to_string(&(path, &key, &n.op, value)).unwrap());
            return if inverse { not(atom) } else { atom };
        }
        self.opaque(n, path)
    }
}
impl Engine {
    pub(crate) fn prove_required_set(
        &self,
        hull: &Node,
        set: &str,
        candidates: &[Arc<Node>],
    ) -> RequiredSetProof {
        let mut solver = Solver {
            engine: self,
            hull,
            serial: 0,
            fuel: 4096,
            depth: 0,
            depth_limited: false,
            truncated: false,
            active: BTreeSet::new(),
        };
        let expression = combine(
            true,
            candidates
                .iter()
                .map(|n| solver.body(n.field("potential").map(Node::body).unwrap_or(&[]), &[]))
                .collect::<Vec<_>>(),
        );
        let assumption = solver.body(
            hull.field("potential_country")
                .map(Node::body)
                .unwrap_or(&[]),
            &["from".into()],
        );
        let mut symbols = BTreeSet::new();
        atoms(&expression, &mut symbols);
        atoms(&assumption, &mut symbols);
        let mut result = RequiredSetProof {
            hull: hull.key.clone(),
            component_set: set.into(),
            status: "unknown",
            symbols: symbols.len(),
            reason: None,
            counterexample: None,
            assumption: "hull potential_country holds for component FROM country",
            proof_domain: "component potential only; technology, resources, runtime and automatic design not proved",
        };
        if solver.truncated {
            result.reason = Some(if solver.depth_limited {
                "depth budget"
            } else {
                "expression budget"
            });
            return result;
        }
        if symbols.len() > 12 {
            result.reason = Some("symbol budget");
            return result;
        }
        let symbols = symbols.into_iter().collect::<Vec<_>>();
        let (mut yes, mut no) = (false, false);
        for mask in 0..(1usize << symbols.len()) {
            let values = symbols
                .iter()
                .enumerate()
                .map(|(i, k)| (k.clone(), mask & (1 << i) != 0))
                .collect::<BTreeMap<_, _>>();
            if !evaluate(&assumption, &values) {
                continue;
            }
            if evaluate(&expression, &values) {
                yes = true;
            } else {
                no = true;
                if result.counterexample.is_none() {
                    result.counterexample = Some(values);
                }
            }
            if yes && no {
                break;
            }
        }
        result.status = match (yes, no) {
            (true, false) => "covered",
            (false, true) => "impossible",
            _ => "unknown",
        };
        if !yes && !no {
            result.reason = Some("no satisfiable hull-country assumption");
        }
        result
    }
}
