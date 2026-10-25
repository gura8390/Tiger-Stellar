use crate::{
    ast::Node,
    domain::{Availability, Binding, Context, State, Types, alias},
    loader::{Program, event_scope},
    parser::ParserClient,
    report::Diagnostics,
    rules::{Declarations, Rules},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type ScriptBodyCacheKey = (u64, String, u32, String, String);

#[derive(Default, Serialize)]
pub struct Metrics {
    pub structure_nodes: u64,
    pub transfer_nodes: u64,
    pub trigger_nodes: u64,
    pub summary_evaluations: u64,
    pub summary_hits: u64,
    pub script_body_compilations: u64,
    pub script_body_cache_hits: u64,
    pub loop_iterations: u64,
    pub max_binding_facts: usize,
    pub enumerated_binding_paths: u64,
    pub event_evaluations: u64,
    pub explicit_ship_designs: u64,
}
#[derive(Clone)]
pub(crate) struct Pending {
    pub ctx: Context,
    pub state: State,
    pub chain: Vec<crate::report::TraceFrame>,
}
pub struct Engine {
    pub rules: Rules,
    pub declarations: Declarations,
    pub program: Program,
    pub parser: ParserClient,
    pub diagnostics: Diagnostics,
    pub metrics: Metrics,
    pub language: Option<String>,
    pub complete: bool,
    pub(crate) error_epoch: u64,
    pub migration_gaps: BTreeSet<String>,
    pub phase_seconds: BTreeMap<String, f64>,
    pub ship_feasibility: Vec<crate::ship_design::Feasibility>,
    pub potential_proofs: Vec<crate::potential_proof::RequiredSetProof>,
    pub inferred_script_inputs: Vec<serde_json::Value>,
    pub engine_entries: Vec<serde_json::Value>,
    pub(crate) documented_action_entries:
        BTreeMap<String, crate::documented_on_actions::DocumentedAction>,
    pub(crate) saved_target_types: crate::saved_target_types::SavedTargetTypes,
    pub(crate) active: Vec<String>,
    pub(crate) script_bodies: BTreeMap<ScriptBodyCacheKey, std::sync::Arc<Vec<Node>>>,
    pub(crate) chain: Vec<crate::report::TraceFrame>,
    pub(crate) structure_seen: BTreeSet<(u64, bool)>,
    pub(crate) description_seen: BTreeSet<u64>,
    pub(crate) reads: BTreeMap<String, BTreeSet<String>>,
    pub(crate) invoked: BTreeSet<(bool, String)>,
    pub(crate) invoked_values: BTreeSet<String>,
    pub(crate) invoked_actions: BTreeSet<String>,
    pub(crate) value_work: usize,
    pub(crate) event_inputs: BTreeMap<String, Pending>,
    pub(crate) queue: VecDeque<String>,
    pub(crate) queued: BTreeSet<String>,
    pub(crate) visited_events: BTreeSet<String>,
    pub(crate) project_inputs: BTreeMap<String, crate::project_contracts::ProjectInput>,
    pub(crate) project_queue: VecDeque<String>,
}
impl Engine {
    pub fn new(
        rules: Rules,
        declarations: Declarations,
        program: Program,
        parser: ParserClient,
        diagnostics: Diagnostics,
    ) -> Self {
        Self {
            rules,
            declarations,
            program,
            parser,
            diagnostics,
            metrics: Metrics::default(),
            language: None,
            complete: true,
            error_epoch: 0,
            migration_gaps: BTreeSet::new(),
            phase_seconds: BTreeMap::new(),
            ship_feasibility: vec![],
            potential_proofs: vec![],
            inferred_script_inputs: vec![],
            engine_entries: vec![],
            documented_action_entries: BTreeMap::new(),
            saved_target_types: crate::saved_target_types::SavedTargetTypes::default(),
            active: vec![],
            script_bodies: BTreeMap::new(),
            chain: vec![],
            structure_seen: BTreeSet::new(),
            description_seen: BTreeSet::new(),
            reads: BTreeMap::new(),
            invoked: BTreeSet::new(),
            invoked_values: BTreeSet::new(),
            invoked_actions: BTreeSet::new(),
            value_work: 0,
            event_inputs: BTreeMap::new(),
            queue: VecDeque::new(),
            queued: BTreeSet::new(),
            visited_events: BTreeSet::new(),
            project_inputs: BTreeMap::new(),
            project_queue: VecDeque::new(),
        }
    }
    pub fn error(
        &mut self,
        n: &Node,
        key: &str,
        message: impl Into<String>,
        ctx: Option<&Context>,
    ) {
        self.error_epoch += 1;
        self.diagnostics.emit(
            n,
            key,
            message.into(),
            ("error", "confirmed-rule-violation"),
            ctx,
            &self.chain,
        );
    }
    pub fn gap(
        &mut self,
        n: &Node,
        message: impl Into<String>,
        reason: &str,
        ctx: Option<&Context>,
    ) {
        self.diagnostics.emit(
            n,
            "validation-coverage",
            message.into(),
            ("warning", reason),
            ctx,
            &self.chain,
        );
    }
    pub fn block(&mut self, n: &Node) -> bool {
        if n.children.is_none() {
            self.error(n, "structure", format!("{} requires a block", n.key), None);
            false
        } else {
            true
        }
    }
    pub fn numeric(&mut self, n: &Node, ctx: Option<&Context>) {
        if n.children.is_some() {
            self.error(n, "structure", "expected a numeric scalar", ctx);
            return;
        }
        if let Some(ctx) = ctx
            && self
                .script_value_reference(n, ctx, &State::default())
                .is_some()
        {
            return;
        }
        match n.scalar().parse::<f64>() {
            Ok(v) => {
                if !v.is_finite() {
                    self.error(n, "value", "numeric value must be finite", ctx);
                }
            }
            Err(_) => {
                self.gap(
                    n,
                    format!(
                        "numeric expression not yet resolved by Rust core: {}",
                        n.scalar()
                    ),
                    "numeric-information-unresolved",
                    ctx,
                );
            }
        }
    }
    pub fn boolean(&mut self, n: &Node) {
        if !matches!(n.value.as_deref(), Some("yes" | "no")) {
            self.error(n, "value", format!("{} expects yes/no", n.key), None);
        }
    }
    pub fn localisation(&mut self, n: &Node) {
        let value = n.scalar();
        if value.is_empty()
            || matches!(value, "none" | "default" | "yes" | "no")
            || value.contains([' ', '[', ']', '$'])
        {
            return;
        }
        let exists = if let Some(language) = &self.language {
            self.program
                .loc
                .get(language)
                .is_some_and(|s| s.contains(value))
        } else {
            self.program.loc.values().any(|s| s.contains(value))
        };
        if !exists {
            self.diagnostics.emit(
                n,
                "localization",
                format!(
                    "localization key `{value}` was not found in {}",
                    self.language.as_deref().unwrap_or("any loaded language")
                ),
                ("warning", "localisation-review"),
                None,
                &self.chain,
            );
        }
    }
    pub fn reference(&mut self, cat: &str, value: &str, n: &Node) {
        if value.is_empty() || matches!(value, "yes" | "no" | "none" | "random") {
            return;
        }
        self.symbol_reference(cat, value, n);
    }
    pub(crate) fn symbol_reference(&mut self, cat: &str, value: &str, n: &Node) {
        if value.is_empty() {
            self.error(n, "missing-field", format!("empty {cat} reference"), None);
            return;
        }
        if value.contains(['$', '@', ':', '|', '[', ']']) {
            self.gap(
                n,
                format!("dynamic reference: {cat}/{value}"),
                "dynamic-reference",
                None,
            );
            return;
        }
        if !self
            .program
            .entities
            .get(cat)
            .is_some_and(|s| s.contains(value))
        {
            self.error(
                n,
                "missing-item",
                format!("unknown {cat} reference `{value}`"),
                None,
            );
        }
    }
    pub fn compatible(
        &mut self,
        t: &Types,
        accepted: &[String],
        n: &Node,
        label: &str,
        ctx: &Context,
    ) {
        if accepted.iter().any(|s| s == "all") {
            return;
        }
        let good = t.known.iter().filter(|k| accepted.contains(k)).count();
        if !t.unknown && !t.known.is_empty() && good == 0 {
            self.error(
                n,
                "scope",
                format!(
                    "{label} accepts {}; current scope is {}",
                    accepted.join(", "),
                    t.label()
                ),
                Some(ctx),
            );
        } else if t.unknown || t.known.is_empty() {
            self.gap(
                n,
                format!(
                    "scope cannot be inferred for {label}; known candidates: {}",
                    t.label()
                ),
                "scope-type-unknown",
                Some(ctx),
            );
        } else if good < t.known.len() {
            self.gap(
                n,
                format!("polymorphic scope {} for {label}", t.label()),
                "scope-type-ambiguous",
                Some(ctx),
            );
        }
    }
    pub fn resolve(
        &mut self,
        value: &str,
        ctx: &Context,
        state: &State,
        n: &Node,
        probe: bool,
    ) -> Context {
        // Optional receiver syntax applies to the scope block being selected;
        // it is not a presence guarantee for scalar parameter references.
        let optional = value.ends_with('?') && n.key == value && n.children.is_some();
        let value = value.trim_end_matches('?');
        if value.starts_with("event_target:") || value.starts_with("global_event_target:") {
            if let Some((base, selector)) = value.split_once('@') {
                self.resolve(selector, ctx, state, n, false);
                self.gap(
                    n,
                    format!("dynamic event target identity: {base}"),
                    "dynamic-target-identity",
                    Some(ctx),
                );
                return ctx.push(Types::new("?"), "dynamic-target");
            }
            let (head, tail) = value
                .split_once('.')
                .map_or((value, None), |(a, b)| (a, Some(b)));
            let b = state.target_binding(head);
            if b.availability != Availability::Definite && !probe && !optional {
                self.gap(
                    n,
                    format!(
                        "event target binding is not established on every analyzed path: {head}"
                    ),
                    "target-binding-unproven",
                    Some(ctx),
                );
            }
            let mut types = b.types;
            if (types.known.is_empty() || types.unknown)
                && let Some(candidates) = self.saved_target_type_candidates(head)
            {
                let existing_unknown = types.unknown;
                types.known.extend(candidates.known);
                // Explicit unknown state can encode future rebinding or an
                // unmodelled producer. A structural catalog must not erase it.
                types.unknown = existing_unknown || candidates.unknown;
            }
            if types.known.is_empty() {
                types.unknown = true;
            }
            let result = ctx.push(types, head);
            return tail.map_or(result.clone(), |tail| {
                self.resolve(tail, &result, state, n, probe)
            });
        }
        let mut result = ctx.clone();
        for part in value.trim_end_matches('?').to_ascii_lowercase().split('.') {
            let previous_object = result.object.clone();
            let types = if part == "root" {
                ctx.root.clone()
            } else if part == "this" {
                result.this.clone()
            } else if let Some(depth) = repeated(part, "from") {
                ctx.froms
                    .get(depth - 1)
                    .cloned()
                    .unwrap_or_else(|| Types::new("?"))
            } else if let Some(depth) = repeated(part, "prev") {
                result.prevs.get(depth - 1).cloned().unwrap_or_else(|| {
                    if result.prev_tail_unknown {
                        self.gap(
                            n,
                            format!("previous scope stack is not known for this caller: {part}"),
                            "caller-context-unknown",
                            Some(ctx),
                        );
                    } else {
                        self.error(
                            n,
                            "scope",
                            format!("previous scope does not exist: {part}"),
                            Some(ctx),
                        );
                    }
                    Types::new("?")
                })
            } else if let Some(rule) = self.rules.scopes.get(part).cloned() {
                self.compatible(&result.this, &rule.input, n, part, &result);
                let mut out = Types::default();
                for source in &result.this.known {
                    out = out.join(&Types::new(
                        rule.output_by_input
                            .get(source)
                            .map(String::as_str)
                            .or(rule.output.as_deref())
                            .unwrap_or("?"),
                    ));
                }
                if result.this.unknown {
                    out = out.join(&Types::new(rule.output.as_deref().unwrap_or("?")));
                }
                out
            } else {
                self.error(
                    n,
                    "unknown-scope",
                    format!("unknown scope link {part}"),
                    Some(ctx),
                );
                Types::new("?")
            };
            result = result.push(types, part);
            if part == "root" {
                result.object = ctx.root_object.clone();
            } else if part == "this" {
                result.object = previous_object;
            } else if let Some(depth) = repeated(part, "from") {
                result.object = ctx
                    .from_objects
                    .get(depth - 1)
                    .cloned()
                    .unwrap_or_else(|| format!("unknown-from:{depth}"));
            } else if let Some(depth) = repeated(part, "prev") {
                result.object = result
                    .prev_objects
                    .get(depth)
                    .cloned()
                    .unwrap_or_else(|| format!("unknown-prev:{depth}"));
            }
        }
        result
    }
    pub(crate) fn is_scope(&self, key: &str) -> bool {
        let first = key
            .trim_end_matches('?')
            .split('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        key.starts_with("event_target:")
            || key.starts_with("global_event_target:")
            || matches!(first.as_str(), "root" | "this")
            || repeated(&first, "from").is_some()
            || repeated(&first, "prev").is_some()
            || self.rules.scopes.contains_key(&first)
    }
    pub(crate) fn guard_key(
        &self,
        limit: &[Node],
        ctx: &Context,
        positive: bool,
    ) -> Option<String> {
        // Only stable local identity/type and explicit target existence are
        // correlated. Unknown side effects invalidate these remembered deltas.
        if limit.len() != 1 {
            return None;
        }
        let n = &limit[0];
        let v = n.scalar();
        let (predicate, sign) = match n.key.as_str() {
            "is_ai" if matches!(v, "yes" | "no") => {
                (format!("{}:is_ai", ctx.object), (v == "yes") == positive)
            }
            "is_scope_type"
                if n.op.as_str() == "="
                    && crate::rules::SCOPES.split_whitespace().any(|s| s == v) =>
            {
                (format!("{}:type:{v}", ctx.object), positive)
            }
            "exists" if v.starts_with("event_target:") || v.starts_with("global_event_target:") => {
                (format!("{}:exists:{v}", ctx.object), positive)
            }
            _ => return None,
        };
        Some(format!("{predicate}={sign}"))
    }
    pub fn triggers(&mut self, nodes: &[Node], ctx: &Context, state: &State) {
        self.structure_script(nodes, false);
        let mut local = state.clone();
        let mut frame = ctx.clone();
        for n in nodes {
            // Structure and context-free references are checked above even
            // when a type guard proves this conjunction infeasible.
            frame = match self.refine(std::slice::from_ref(n), &frame, &mut local, true) {
                Some(narrow) => narrow,
                None => return,
            };
        }
        // Check predicate reads using entry/existence facts. The facts produced
        // by successful named predicates belong to their subsequent branch,
        // not to a nested AND, tooltip, alias or inline call checking itself.
        let read_state = if local
            .bindings
            .iter()
            .any(|(k, b)| b.availability != state.get(k).availability)
        {
            let mut entry = state.clone();
            self.refine_guard_entry(nodes, ctx, &mut entry);
            let mut read = local.clone();
            for (key, binding) in &mut read.bindings {
                binding.availability = entry.get(key).availability;
            }
            Some(read)
        } else {
            None
        };
        let checked = read_state.as_ref().unwrap_or(&local);
        let mut pending_false = Some((frame.clone(), checked.clone()));
        for n in nodes {
            self.metrics.trigger_nodes += 1;
            let key = n.key.to_ascii_lowercase();
            if n.key.starts_with('@') || n.is_memory_directive() {
                continue;
            }
            if matches!(key.as_str(), "if" | "else_if" | "else") {
                let base = if key == "if" {
                    Some((frame.clone(), checked.clone()))
                } else {
                    pending_false.take()
                };
                if let Some((before, mut input)) = base {
                    let limit = n.field("limit").map(Node::body).unwrap_or(&[]);
                    if key != "else" {
                        self.triggers(limit, &before, &input);
                    }
                    let yes = if key == "else" {
                        Some(before.clone())
                    } else {
                        self.refine(limit, &before, &mut input, true)
                    };
                    if let Some(yes) = yes {
                        let body = n
                            .body()
                            .iter()
                            .filter(|c| c.key != "limit")
                            .cloned()
                            .collect::<Vec<_>>();
                        self.triggers(&body, &yes, &input);
                    }
                    let mut no = checked.clone();
                    pending_false = if key == "else" {
                        None
                    } else {
                        self.refine(limit, &before, &mut no, false).map(|c| (c, no))
                    };
                }
                continue;
            }
            if self.is_scope(&n.key) {
                if self.block(n) {
                    let key = n.key.trim_end_matches('?');
                    let head = key.split('.').next().unwrap_or(key);
                    let next = self.resolve(&n.key, &frame, checked, n, false);
                    let mut selected = checked.clone();
                    if (head.starts_with("event_target:")
                        || head.starts_with("global_event_target:"))
                        && !head.contains(['@', '$', '[', ']'])
                    {
                        let actual = if selected.bindings.contains_key(head) {
                            head.to_string()
                        } else if head.starts_with("event_target:")
                            && selected.bindings.contains_key(&alias(head))
                        {
                            alias(head)
                        } else {
                            head.to_string()
                        };
                        let mut binding = selected.target_binding(head);
                        binding.availability = Availability::Definite;
                        if binding.types.known.is_empty() {
                            binding.types.unknown = true;
                        }
                        selected.bindings.insert(actual, binding);
                    }
                    self.triggers(n.body(), &next, &selected);
                }
                continue;
            }
            if matches!(
                key.as_str(),
                "and"
                    | "limit"
                    | "hidden_trigger"
                    | "or"
                    | "nor"
                    | "not"
                    | "nand"
                    | "custom_tooltip"
                    | "custom_tooltip_fail"
                    | "success_text"
                    | "custom_tooltip_with_modifiers"
            ) {
                let nested = n
                    .body()
                    .iter()
                    .filter(|n| !Self::tooltip_metadata(&key, n))
                    .cloned()
                    .collect::<Vec<_>>();
                // The refined conjunction is common to every alternative.
                // Joined facts do not select one arm; entry availability still
                // prevents a predicate from proving its own receiver read.
                if matches!(key.as_str(), "or" | "nor") {
                    self.trigger_alternatives(&nested, &frame, checked);
                } else {
                    self.triggers(&nested, &frame, checked);
                }
                continue;
            }
            if key == "inline_script" {
                if let Some(body) = self.expand_inline(n, &frame) {
                    self.triggers(&body, &frame, checked);
                }
                continue;
            }
            if self.program.scripts.contains_key(&(false, n.key.clone()))
                && !self.rules.triggers.contains_key(&n.key)
            {
                self.call(n, false, &frame, &mut checked.clone());
                continue;
            }
            if let Some(rule) = self.rules.triggers.get(&n.key).cloned() {
                self.compatible(&frame.this, &rule.input, n, &n.key, &frame);
                if n.key == "is_scope_type"
                    && !n.scalar().contains(['$', '@', '[', ']'])
                    && !crate::rules::SCOPES
                        .split_whitespace()
                        .any(|s| s == n.scalar())
                {
                    self.error(
                        n,
                        "value",
                        format!("unknown Stellaris scope type {}", n.scalar()),
                        Some(&frame),
                    );
                }
                if n.key == "exists" {
                    self.resolve(n.scalar(), &frame, checked, n, true);
                }
                if rule.iterator_output.is_some() {
                    let next = frame.push(
                        Types::new(rule.iterator_output.as_deref().unwrap()),
                        &format!("iterator:{}", n.id),
                    );
                    let mut iterator_input = checked.clone();
                    iterator_input.forget_number_object(&next.object);
                    let body = n
                        .body()
                        .iter()
                        .filter(|c| {
                            !matches!(
                                c.key.as_str(),
                                "count"
                                    | "percent"
                                    | "position"
                                    | "order_by"
                                    | "max_steps"
                                    | "min_steps"
                                    | "weights"
                                    | "inverse"
                                    | "use_bypasses"
                            )
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    for meta in n.body().iter().filter(|c| {
                        matches!(
                            c.key.as_str(),
                            "count" | "percent" | "position" | "max_steps" | "min_steps"
                        )
                    }) {
                        if meta.scalar() != "all" {
                            self.numeric_in_state(meta, &next, &iterator_input);
                        }
                    }
                    if let Some(weights) = n.field("weights") {
                        self.weighted(weights, &next, &iterator_input);
                    }
                    self.triggers(&body, &next, &iterator_input);
                } else if n.children.is_some() {
                    self.parameters(n, false, &frame, &mut checked.clone(), &rule);
                } else {
                    if rule.boolean
                        && reference_category(&n.key).is_none()
                        && !matches!(
                            n.key.as_str(),
                            "has_resource" | "has_deposit" | "has_building"
                        )
                    {
                        self.boolean(n);
                    }
                    if rule.numeric {
                        self.numeric_in_state(n, &frame, checked);
                    }
                    if let Some(cat) = reference_category(&n.key) {
                        self.reference(cat, n.scalar(), n);
                    }
                }
            } else {
                self.error(
                    n,
                    "unknown-trigger",
                    format!("unknown triggers token `{}`", n.key),
                    Some(&frame),
                );
            }
        }
    }
    pub fn effects(&mut self, nodes: &[Node], ctx: &Context, state: &mut State) {
        let material = self.materialize(nodes, ctx);
        let nodes = material.as_deref().unwrap_or(nodes);
        let mut i = 0;
        self.structure_script(nodes, true);
        while i < nodes.len() {
            let n = &nodes[i];
            i += 1;
            self.metrics.transfer_nodes += 1;
            self.metrics.max_binding_facts =
                self.metrics.max_binding_facts.max(state.bindings.len());
            if n.key.starts_with('@') || n.is_memory_directive() {
                continue;
            }
            match n.key.to_ascii_lowercase().as_str() {
                "if" => {
                    let mut chain = vec![n];
                    while i < nodes.len()
                        && matches!(
                            nodes[i].key.to_ascii_lowercase().as_str(),
                            "else_if" | "else"
                        )
                    {
                        let b = &nodes[i];
                        chain.push(b);
                        i += 1;
                        if b.key.eq_ignore_ascii_case("else") {
                            break;
                        }
                    }
                    let first_limit = n.field("limit").map(Node::body).unwrap_or(&[]).to_vec();
                    let simple_if = chain.len() == 1;
                    let simple_else = chain.len() == 2 && chain[1].key.eq_ignore_ascii_case("else");
                    let base = state.clone();
                    let mut pending = Some((base.clone(), ctx.clone()));
                    let mut results = Vec::new();
                    let mut branches = Vec::new();
                    for branch in chain {
                        let branch_key = branch.key.to_ascii_lowercase();
                        if !self.block(branch) {
                            continue;
                        }
                        let Some((before, frame)) = pending.take() else {
                            break;
                        };
                        let limit = branch.field("limit").map(Node::body).unwrap_or(&[]);
                        if branch_key != "else" {
                            if branch.field("limit").is_none() {
                                self.error(
                                    branch,
                                    "missing-field",
                                    "if/else_if requires limit",
                                    Some(ctx),
                                );
                            }
                            self.triggers(limit, &frame, &before);
                        }
                        let mut yes = before.clone();
                        let true_ctx = if branch_key == "else" {
                            Some(frame.clone())
                        } else {
                            self.refine(limit, &frame, &mut yes, true)
                        };
                        if let Some(frame) = true_ctx {
                            let body = branch
                                .body()
                                .iter()
                                .filter(|c| c.key != "limit")
                                .cloned()
                                .collect::<Vec<_>>();
                            self.effects(&body, &frame, &mut yes);
                            if branch_key == "if" {
                                branches.push((limit.to_vec(), yes.clone(), true));
                            } else if branch_key == "else" && simple_else {
                                branches.push((first_limit.clone(), yes.clone(), false));
                            }
                            results.push(yes);
                        }
                        if branch_key != "else" {
                            let mut no = before.clone();
                            if let Some(frame) = self.refine(limit, &frame, &mut no, false) {
                                pending = Some((no, frame));
                            }
                        }
                    }
                    if let Some((s, _)) = pending {
                        if simple_if {
                            branches.push((first_limit.clone(), s.clone(), false));
                        }
                        results.push(s);
                    }
                    if let Some(first) = results.first() {
                        *state = results[1..].iter().fold(first.clone(), |a, b| a.join(b));
                        let merged = state.clone();
                        for (limit, s, sign) in branches {
                            if let Some(k) = self.guard_key(&limit, ctx, sign) {
                                state.remember(k, &s, &merged);
                            }
                        }
                    }
                }
                "else" | "else_if" => {
                    self.error(
                        n,
                        "structure",
                        format!("{} has no preceding if", n.key),
                        Some(ctx),
                    );
                    self.effects(n.body(), ctx, state);
                }
                "random" => {
                    let chance = n.val("chance").and_then(|s| s.parse::<f64>().ok());
                    if let Some(c) = n.field("chance") {
                        self.numeric_in_state(c, ctx, state);
                    } else {
                        self.error(n, "missing-field", "random requires chance", Some(ctx));
                    }
                    let body = n
                        .body()
                        .iter()
                        .filter(|c| c.key != "chance")
                        .cloned()
                        .collect::<Vec<_>>();
                    let mut selected = state.clone();
                    if chance.is_none_or(|v| v > 0.) {
                        self.effects(&body, ctx, &mut selected);
                    }
                    if chance.is_none_or(|v| v > 0.) {
                        *state = if chance.is_some_and(|v| v >= 100.) {
                            selected
                        } else {
                            state.join(&selected)
                        };
                    }
                }
                "random_list" | "locked_random_list" | "switch" => {
                    self.alternatives(n, ctx, state);
                }
                "while" => {
                    self.repeat(n, ctx, state, None);
                }
                "hidden_effect" | "hidden_tooltip" | "tooltip" => {
                    if self.block(n) {
                        self.effects(n.body(), ctx, state);
                    }
                }
                "custom_tooltip" | "custom_description" => {
                    if n.children.is_some() {
                        let body = n
                            .body()
                            .iter()
                            .filter(|c| c.key != "text")
                            .cloned()
                            .collect::<Vec<_>>();
                        self.effects(&body, ctx, state);
                    }
                }
                _ => self.atom(n, ctx, state),
            }
        }
    }
    fn alternatives(&mut self, n: &Node, ctx: &Context, state: &mut State) {
        if !self.block(n) {
            return;
        }
        let initial = state.clone();
        let mut outputs = Vec::new();
        let mut can_skip = n.key.eq_ignore_ascii_case("switch") && n.field("default").is_none();
        if n.key.eq_ignore_ascii_case("switch") {
            if let Some(t) = n.field("trigger") {
                if !self.rules.triggers.contains_key(t.scalar())
                    && !self
                        .program
                        .scripts
                        .contains_key(&(false, t.scalar().into()))
                {
                    self.error(
                        t,
                        "unknown-trigger",
                        format!("unknown switch trigger {}", t.scalar()),
                        Some(ctx),
                    );
                }
            } else {
                self.error(n, "missing-field", "switch requires trigger", Some(ctx));
            }
        }
        for b in n.body() {
            if matches!(b.key.as_str(), "trigger" | "on_trigger")
                && n.key.eq_ignore_ascii_case("switch")
            {
                continue;
            }
            if !n.key.eq_ignore_ascii_case("switch") {
                match b.key.parse::<f64>() {
                    Ok(v) if v.is_finite() && v >= 0. => {
                        if v == 0. {
                            continue;
                        }
                    }
                    _ => {
                        self.error(
                            b,
                            "value",
                            "random_list weight must be finite and non-negative",
                            Some(ctx),
                        );
                        continue;
                    }
                }
            }
            let mut s = initial.clone();
            let effects = b
                .body()
                .iter()
                .filter(|c| !matches!(c.key.as_str(), "modifier" | "trigger" | "limit"))
                .cloned()
                .collect::<Vec<_>>();
            for meta in b
                .body()
                .iter()
                .filter(|c| matches!(c.key.as_str(), "modifier" | "trigger" | "limit"))
            {
                can_skip = true;
                if meta.key == "modifier" {
                    self.weighted(meta, ctx, &s);
                } else {
                    self.triggers(meta.body(), ctx, &s);
                }
            }
            self.effects(&effects, ctx, &mut s);
            outputs.push(s);
        }
        if can_skip || outputs.is_empty() {
            outputs.push(initial);
        }
        *state = outputs[1..]
            .iter()
            .fold(outputs[0].clone(), |a, b| a.join(b));
    }
    fn repeat(&mut self, n: &Node, ctx: &Context, state: &mut State, output: Option<&str>) {
        let mut base = state.clone();
        let mut current = base.clone();
        let body = n
            .body()
            .iter()
            .filter(|c| {
                !matches!(
                    c.key.as_str(),
                    "limit"
                        | "count"
                        | "position"
                        | "order_by"
                        | "percent"
                        | "max_steps"
                        | "min_steps"
                        | "use_bypasses"
                        | "inverse"
                        | "weights"
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let frame = output.map_or(ctx.clone(), |s| {
            ctx.push(Types::new(s), &format!("iterator:{}", n.id))
        });
        if output.is_some() {
            base.forget_number_object(&frame.object);
            current.forget_number_object(&frame.object);
        }
        if let Some(limit) = n.field("limit") {
            self.triggers(limit.body(), &frame, &current);
        }
        if let Some(weights) = n.field("weights") {
            self.weighted(weights, &frame, &current);
        }
        for field in n
            .body()
            .iter()
            .filter(|c| matches!(c.key.as_str(), "count" | "position" | "percent"))
        {
            if field.scalar() != "all" {
                self.numeric_in_state(field, ctx, state);
            }
        }
        let count = n.val("count").and_then(|s| s.parse::<u64>().ok());
        if count == Some(0) {
            return;
        }
        if n.key.eq_ignore_ascii_case("while")
            && let Some(limit) = n.field("limit")
            && self
                .refine(limit.body(), &frame, &mut current, true)
                .is_none()
        {
            return;
        }
        if let Some(count) = count {
            let mut accumulated = base.clone();
            let iterations = count.min(64);
            for _ in 0..iterations {
                self.metrics.loop_iterations += 1;
                if output.is_some() {
                    current.forget_number_object(&frame.object);
                }
                self.effects(&body, &frame, &mut current);
                accumulated = accumulated.join(&current);
            }
            *state = accumulated;
            if count > 64 {
                self.complete = false;
                self.gap(
                    n,
                    "finite loop exceeds transfer budget",
                    "analysis-limit",
                    Some(ctx),
                );
            }
            return;
        }
        for _ in 0..64 {
            self.metrics.loop_iterations += 1;
            let mut next = current.clone();
            if output.is_some() {
                next.forget_number_object(&frame.object);
            }
            self.effects(&body, &frame, &mut next);
            let joined = base.join(&next);
            if joined == current {
                *state = joined;
                return;
            }
            current = joined;
        }
        *state = current;
        self.complete = false;
        self.gap(
            n,
            "loop fact transfer did not converge within budget",
            "analysis-limit",
            Some(ctx),
        );
    }
    fn atom(&mut self, n: &Node, ctx: &Context, state: &mut State) {
        if self.is_scope(&n.key) {
            if self.block(n) {
                let key = n.key.trim_end_matches('?');
                let (head, tail) = key
                    .split_once('.')
                    .map_or((key, None), |(a, b)| (a, Some(b)));
                let named =
                    head.starts_with("event_target:") || head.starts_with("global_event_target:");
                let receiver = state.target_binding(head);
                // Known type does not guarantee presence. A named receiver
                // and nullable/undocumented links can skip the entire body.
                let root_present = !ctx.root.unknown
                    && !ctx.root.known.is_empty()
                    && !ctx
                        .root
                        .known
                        .iter()
                        .any(|s| matches!(s.as_str(), "none" | "no_scope"));
                let guaranteed_tail = tail.is_none_or(|s| {
                    s.split('.').all(|link| {
                        link.eq_ignore_ascii_case("this")
                            || (link.eq_ignore_ascii_case("root") && root_present)
                    })
                });
                let conditional =
                    named && (receiver.availability != Availability::Definite || !guaranteed_tail);
                let frame = self.resolve(&n.key, ctx, state, n, false);
                if conditional {
                    let mut selected = state.clone();
                    let mut assumption = None;
                    if !head.contains(['@', '$', '[', ']']) {
                        let actual = if selected.bindings.contains_key(head) {
                            head.to_string()
                        } else if head.starts_with("event_target:")
                            && selected.bindings.contains_key(&alias(head))
                        {
                            alias(head)
                        } else {
                            head.to_string()
                        };
                        let mut exists = receiver;
                        exists.availability = Availability::Definite;
                        if exists.types.known.is_empty() {
                            exists.types.unknown = true;
                        }
                        assumption = Some((
                            actual.clone(),
                            exists.clone(),
                            state.bindings.get(&actual).cloned(),
                        ));
                        selected.set(actual, exists);
                    }
                    self.effects(n.body(), &frame, &mut selected);
                    // Entry existence is a branch assumption, not an effect
                    // that created this binding. Retract it if unchanged.
                    if let Some((key, proof, original)) = assumption
                        && selected.bindings.get(&key) == Some(&proof)
                    {
                        if let Some(binding) = original {
                            selected.bindings.insert(key, binding);
                        } else {
                            selected.bindings.remove(&key);
                        }
                    }
                    *state = state.join(&selected);
                } else {
                    let before_numbers = state.number_snapshot();
                    self.effects(n.body(), &frame, state);
                    let direct = key.eq_ignore_ascii_case("this")
                        && !ctx.this.unknown
                        && !ctx.this.known.is_empty()
                        || key.eq_ignore_ascii_case("root") && root_present;
                    if !direct {
                        let conditional_numbers = before_numbers.join(&state.number_snapshot());
                        state.numbers = conditional_numbers.numbers;
                        state.guarded_numbers = conditional_numbers.guarded_numbers;
                    }
                }
            }
            return;
        }
        if self.program.scripts.contains_key(&(true, n.key.clone()))
            && !self.rules.effects.contains_key(&n.key)
        {
            self.call(n, true, ctx, state);
            return;
        }
        let Some(rule) = self.rules.effects.get(&n.key).cloned() else {
            self.error(
                n,
                "unknown-effect",
                format!("unknown effects token `{}`", n.key),
                Some(ctx),
            );
            return;
        };
        self.compatible(&ctx.this, &rule.input, n, &n.key, ctx);
        if self.variable_operation(n, true, ctx, state) {
            return;
        }
        if let Some(output) = rule.iterator_output.as_deref() {
            if n.key.starts_with("random_") || n.key.starts_with("ordered_") {
                let mut selected = state.clone();
                let frame = ctx.push(Types::new(output), &format!("selector:{}", n.id));
                selected.forget_number_object(&frame.object);
                if let Some(limit) = n.field("limit") {
                    self.triggers(limit.body(), &frame, &selected);
                }
                if let Some(weights) = n.field("weights") {
                    self.weighted(weights, &frame, &selected);
                }
                let body = n
                    .body()
                    .iter()
                    .filter(|c| {
                        !matches!(
                            c.key.as_str(),
                            "limit" | "count" | "position" | "order_by" | "inverse" | "weights"
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                self.effects(&body, &frame, &mut selected);
                *state = state.join(&selected);
            } else {
                self.repeat(n, ctx, state, Some(output));
            }
            return;
        }
        if matches!(
            n.key.as_str(),
            "save_event_target_as" | "save_global_event_target_as"
        ) {
            let prefix = if n.key == "save_event_target_as" {
                "event_target:"
            } else {
                "global_event_target:"
            };
            if n.children.is_some() || n.scalar().is_empty() {
                self.error(
                    n,
                    "structure",
                    "target save requires a scalar name",
                    Some(ctx),
                );
                return;
            }
            state.forget_number_alias(&format!("{prefix}{}", n.scalar()));
            state.set(
                format!("{prefix}{}", n.scalar()),
                Binding::assigned(ctx.this.clone()),
            );
            return;
        }
        if n.key == "clear_global_event_target" {
            state.forget_number_alias(&format!("global_event_target:{}", n.scalar()));
            state.set(
                format!("global_event_target:{}", n.scalar()),
                Binding::default(),
            );
            return;
        }
        if n.key == "clear_global_event_targets" {
            state.forget_number_targets();
            let keys = state
                .bindings
                .keys()
                .filter(|k| k.starts_with("global_event_target:"))
                .cloned()
                .collect::<Vec<_>>();
            for k in keys {
                state.forget_number_alias(&k);
                state.set(k, Binding::default());
            }
            return;
        }
        // Unknown state mutation cannot preserve a correlation merely because
        // the predicate has the same spelling later in the source.
        if !matches!(
            n.key.as_str(),
            "set_country_flag"
                | "remove_country_flag"
                | "set_planet_flag"
                | "remove_planet_flag"
                | "set_ship_flag"
                | "remove_ship_flag"
                | "set_fleet_flag"
                | "remove_fleet_flag"
                | "log"
        ) {
            state.clear_correlations();
        }
        if n.children.is_some() {
            self.parameters(n, true, ctx, state, &rule);
        } else {
            if event_scope(&n.key).is_some() {
                self.error(
                    n,
                    "structure",
                    "event call requires parameter block",
                    Some(ctx),
                );
            }
            if rule.boolean {
                self.boolean(n);
            }
            if rule.numeric {
                self.numeric_in_state(n, ctx, state);
            }
            if let Some(cat) = reference_category(&n.key) {
                self.reference(cat, n.scalar(), n);
            }
            if !crate::variables::preserves_numbers(&n.key) {
                state.forget_numbers();
            }
        }
    }
    pub(crate) fn structure_visit(&mut self, n: &Node, effect: bool) {
        if self.structure_seen.insert((n.id, effect)) {
            self.metrics.structure_nodes += 1;
        }
    }
    pub(crate) fn structure_script(&mut self, nodes: &[Node], effect: bool) {
        let mut preceding_if = false;
        for n in nodes {
            if n.is_memory_directive() {
                continue;
            }
            let key = n.key.to_ascii_lowercase();
            let chain = matches!(key.as_str(), "else" | "else_if");
            if chain && !preceding_if {
                self.error(
                    n,
                    "structure",
                    format!("{} has no preceding if", n.key),
                    None,
                );
            }
            preceding_if = key == "if" || (key == "else_if" && preceding_if);
            if !self.structure_seen.insert((n.id, effect)) {
                continue;
            }
            self.metrics.structure_nodes += 1;
            if n.key.starts_with('@') || n.key == "inline_script" {
                continue;
            }
            let control = matches!(
                key.as_str(),
                "if" | "else_if"
                    | "else"
                    | "while"
                    | "random"
                    | "random_list"
                    | "locked_random_list"
                    | "switch"
                    | "and"
                    | "or"
                    | "not"
                    | "nor"
                    | "nand"
                    | "hidden_trigger"
                    | "limit"
                    | "hidden_effect"
                    | "hidden_tooltip"
                    | "tooltip"
                    | "custom_tooltip"
                    | "custom_description"
                    | "custom_tooltip_with_modifiers"
            ) || (!effect
                && matches!(key.as_str(), "custom_tooltip_fail" | "success_text"));
            if self.is_scope(&n.key) {
                if self.block(n) {
                    self.structure_script(n.body(), effect);
                }
                continue;
            }
            if control {
                if n.value.is_some()
                    && matches!(key.as_str(), "custom_tooltip" | "custom_description")
                {
                    continue;
                }
                if !self.block(n) {
                    continue;
                }
                if matches!(
                    key.as_str(),
                    "custom_tooltip"
                        | "custom_description"
                        | "custom_tooltip_with_modifiers"
                        | "custom_tooltip_fail"
                        | "success_text"
                ) {
                    for text in n.body().iter().filter(|c| Self::tooltip_metadata(&key, c)) {
                        if self.scalar_field(text) {
                            self.localisation(text);
                        }
                    }
                }
                if matches!(key.as_str(), "if" | "else_if") && n.field("limit").is_none() {
                    self.error(n, "missing-field", "if/else_if requires limit", None);
                }
                if key == "random" && n.field("chance").is_none() {
                    self.error(n, "missing-field", "random requires chance", None);
                }
                if matches!(
                    key.as_str(),
                    "random_list" | "locked_random_list" | "switch"
                ) {
                    for b in n.body() {
                        if b.key == "trigger" || b.key == "on_trigger" {
                            continue;
                        }
                        let effects = b
                            .body()
                            .iter()
                            .filter(|c| !matches!(c.key.as_str(), "modifier" | "trigger" | "limit"))
                            .cloned()
                            .collect::<Vec<_>>();
                        self.structure_script(&effects, effect);
                    }
                    continue;
                }
                let body = n
                    .body()
                    .iter()
                    .filter(|c| {
                        !matches!(c.key.as_str(), "limit" | "count" | "chance")
                            && !Self::tooltip_metadata(&key, c)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if let Some(limit) = n.field("limit") {
                    self.structure_script(limit.body(), false);
                }
                self.structure_script(&body, effect);
                continue;
            }
            if self.scripted_call_shape(n, effect) {
                continue;
            }
            let rule = if effect {
                self.rules.effects.get(&n.key)
            } else {
                self.rules.triggers.get(&n.key)
            }
            .cloned();
            let Some(rule) = rule else {
                self.error(
                    n,
                    if effect {
                        "unknown-effect"
                    } else {
                        "unknown-trigger"
                    },
                    format!(
                        "unknown {} token `{}`",
                        if effect { "effects" } else { "triggers" },
                        n.key
                    ),
                    None,
                );
                continue;
            };
            if n.key == "is_scope_type" && self.scalar_field(n) {
                if n.scalar().contains(['$', '@', '[', ']']) {
                    self.gap(
                        n,
                        "scope type expression is unresolved; no concrete type can be assumed",
                        "scope-type-expression-unverified",
                        None,
                    );
                } else if !crate::rules::SCOPES
                    .split_whitespace()
                    .any(|s| s == n.scalar())
                {
                    self.error(
                        n,
                        "value",
                        format!("unknown Stellaris scope type {}", n.scalar()),
                        None,
                    );
                }
            }
            if self.parameter_structure(n, effect) {
                continue;
            }
            if effect && n.key == "queue_actions" {
                self.block(n);
                continue;
            }
            if rule.iterator_output.is_some() {
                if self.block(n) {
                    if let Some(limit) = n.field("limit") {
                        self.structure_script(limit.body(), false);
                    }
                    let body = n
                        .body()
                        .iter()
                        .filter(|c| {
                            !matches!(
                                c.key.as_str(),
                                "limit"
                                    | "count"
                                    | "position"
                                    | "order_by"
                                    | "percent"
                                    | "max_steps"
                                    | "min_steps"
                                    | "use_bypasses"
                                    | "inverse"
                                    | "weights"
                            )
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    self.structure_script(&body, effect);
                }
                continue;
            }
            if n.children.is_some() {
                if rule.boolean
                    && rule.parameters.is_empty()
                    && n.key != "end_fleet_contract"
                    && reference_category(&n.key).is_none()
                    && !matches!(
                        n.key.as_str(),
                        "has_resource" | "has_deposit" | "has_building"
                    )
                {
                    self.error(
                        n,
                        "structure",
                        format!("{} expects a scalar yes/no", n.key),
                        None,
                    );
                    continue;
                }
                for c in n.body() {
                    if matches!(c.key.as_str(), "effect" | "init_effect" | "init_effects") {
                        self.structure_script(c.body(), true);
                    } else if matches!(c.key.as_str(), "limit" | "trigger" | "potential") {
                        self.structure_script(c.body(), false);
                    }
                }
            } else {
                if event_scope(&n.key).is_some()
                    || matches!(
                        n.key.as_str(),
                        "set_variable"
                            | "change_variable"
                            | "create_ship"
                            | "create_fleet"
                            | "create_megastructure"
                            | "add_modifier"
                    )
                {
                    self.error(
                        n,
                        "structure",
                        format!("{} requires a parameter block", n.key),
                        None,
                    );
                }
                if rule.boolean
                    && reference_category(&n.key).is_none()
                    && !matches!(
                        n.key.as_str(),
                        "has_resource" | "has_deposit" | "has_building"
                    )
                {
                    self.boolean(n);
                }
                if let Some(category) = reference_category(&n.key) {
                    self.reference(category, n.scalar(), n);
                }
                if rule.numeric
                    && let Ok(v) = n.scalar().parse::<f64>()
                    && !v.is_finite()
                {
                    self.error(n, "value", "numeric value must be finite", None);
                }
            }
        }
    }
}
pub(crate) fn repeated(s: &str, word: &str) -> Option<usize> {
    if !s.is_empty() && s.len().is_multiple_of(word.len()) && s == word.repeat(s.len() / word.len())
    {
        Some(s.len() / word.len())
    } else {
        None
    }
}
fn reference_category(k: &str) -> Option<&'static str> {
    Some(match k {
        "has_technology" | "give_technology" => "technology",
        "has_building" | "add_building" | "remove_building" => "buildings",
        "has_district" => "districts",
        "has_trait" | "add_trait" | "remove_trait" => "traits",
        "is_planet_class" | "change_pc" => "planet_classes",
        "is_pop_category" => "pop_categories",
        "has_ethic" => "ethics",
        "has_civic" | "has_origin" => "governments/civics",
        "has_authority" => "governments/authorities",
        "has_ascension_perk" => "ascension_perks",
        "has_tradition" => "traditions",
        "has_component" => "component_templates",
        "is_ship_size" => "ship_sizes",
        "is_megastructure_type" => "megastructures",
        "has_modifier" | "remove_modifier" => "static_modifiers",
        _ => return None,
    })
}
