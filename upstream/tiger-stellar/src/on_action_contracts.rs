//! Script-fired action contracts. effects.log documents explicit FROM slots;
//! it does not establish default ROOT/PREV, callback timing or target lifetime.
use crate::{
    ast::Node,
    domain::{Availability, Context, State, Types},
    engine::{Engine, repeated},
};
use std::{collections::BTreeSet, sync::Arc};

fn dynamic(value: &str) -> bool {
    value.contains(['$', '@', ':', '[', ']', '|'])
}
fn unexpanded(n: &Node) -> bool {
    n.macro_parts
        .as_ref()
        .is_some_and(|parts| parts.iter().any(|p| p.parameter))
}

impl Engine {
    fn action_definitions(&self, name: &str) -> Vec<Arc<Node>> {
        let mut nodes = self
            .program
            .files
            .iter()
            .filter(|f| f.category == "on_actions")
            .flat_map(|f| f.ast.iter())
            .filter(|n| n.key == name)
            .cloned()
            .collect::<Vec<_>>();
        if nodes.is_empty()
            && let Some(n) = self
                .program
                .definitions
                .get("on_actions")
                .and_then(|m| m.get(name))
        {
            nodes.push(n.clone());
        }
        nodes
    }
    pub(crate) fn fire_action_structure(&mut self, n: &Node) {
        if unexpanded(n) {
            self.gap(
                n,
                "on_action block needs macro expansion before interpreting its fields",
                "on-action-expansion-unverified",
                None,
            );
            return;
        }
        if n.op.as_str() != "=" {
            self.error(n, "operator", "fire_on_action requires =", None);
        }
        if !self.block(n) {
            return;
        }
        if n.field("on_action").is_none() {
            self.error(
                n,
                "missing-field",
                "fire_on_action requires on_action",
                None,
            );
        }
        let mut seen = BTreeSet::new();
        for field in n.body() {
            if field.op.as_str() != "=" {
                self.error(
                    field,
                    "operator",
                    "fire_on_action parameters require =",
                    None,
                );
            }
            if !seen.insert(field.key.clone()) {
                self.error(
                    field,
                    "duplicate-field",
                    "duplicate fire_on_action parameter",
                    None,
                );
            }
            match field.key.as_str() {
                "on_action" => {
                    if self.scalar_field(field) {
                        if dynamic(field.scalar()) {
                            self.gap(
                                field,
                                "on_action identity requires runtime expansion",
                                if field.scalar().contains('$') {
                                    "on-action-expansion-unverified"
                                } else {
                                    "dynamic-on-action-identity"
                                },
                                None,
                            );
                        } else {
                            self.symbol_reference("on_actions", field.scalar(), field);
                        }
                    }
                }
                "scopes" => {
                    if !self.block(field) {
                        continue;
                    }
                    let mut slots = BTreeSet::new();
                    for slot in field.body() {
                        let key = slot.key.to_ascii_lowercase();
                        if slot.op.as_str() != "=" {
                            self.error(
                                slot,
                                "operator",
                                "on_action scope overrides require =",
                                None,
                            );
                        }
                        if !slots.insert(key.clone()) {
                            self.error(
                                slot,
                                "duplicate-field",
                                "duplicate on_action scope override",
                                None,
                            );
                        }
                        if repeated(&key, "from").is_none() {
                            self.error(
                                slot,
                                "unknown-field",
                                "on_action scope override expects from/fromfrom/...",
                                None,
                            );
                        }
                        self.scalar_field(slot);
                    }
                }
                _ => self.error(field, "unknown-field", "unknown fire_on_action field", None),
            }
        }
    }
    fn action_event_entries(&mut self, list: &Node, ctx: &Context) -> Vec<(Node, bool)> {
        let mut entries = Vec::new();
        if list.op.as_str() != "=" {
            self.error(list, "operator", "on_action event list requires =", None);
        }
        if !self.block(list) {
            return entries;
        }
        let body = self
            .materialize(list.body(), ctx)
            .unwrap_or_else(|| list.body().to_vec());
        for item in body {
            if item.is_memory_directive() {
                continue;
            }
            if item.children.is_some() || item.value.is_none() {
                self.error(
                    &item,
                    "structure",
                    "on_action expects event IDs or weight = event ID",
                    Some(ctx),
                );
                continue;
            }
            let weight = if list.key == "random_events" {
                if item.op.as_str() != "=" {
                    self.error(&item, "operator", "random_events weights require =", None);
                }
                match item.key.parse::<f64>() {
                    Ok(w) if w.is_finite() && w >= 0.0 => w,
                    _ => {
                        self.error(
                            &item,
                            "value",
                            "random_events expects finite non-negative numeric weights",
                            Some(ctx),
                        );
                        continue;
                    }
                }
            } else if item.key.is_empty() {
                1.0
            } else {
                self.error(
                    &item,
                    "structure",
                    "events expects an unkeyed event ID list",
                    Some(ctx),
                );
                continue;
            };
            if list.key == "random_events" && item.scalar() == "0" {
                continue;
            }
            if dynamic(item.scalar()) {
                self.gap(
                    &item,
                    "on_action event identity requires runtime expansion",
                    "dynamic-on-action-event-identity",
                    Some(ctx),
                );
            } else if !self.program.events.contains_key(item.scalar()) {
                self.error(
                    &item,
                    "missing-event",
                    format!("unknown event {}", item.scalar()),
                    Some(ctx),
                );
            } else {
                entries.push((item, weight > 0.0));
            }
        }
        entries
    }
    pub(crate) fn on_action_structure(&mut self, n: &Node) {
        if n.is_memory_directive() {
            return;
        }
        self.structure_visit(n, true);
        if unexpanded(n) {
            self.gap(
                n,
                "on_action definition needs macro expansion",
                "on-action-expansion-unverified",
                None,
            );
            return;
        }
        if n.op.as_str() != "=" {
            self.error(n, "operator", "on_action definition requires =", None);
        }
        if !self.block(n) {
            return;
        }
        let ctx = Context::unknown_caller();
        let body = self
            .materialize(n.body(), &ctx)
            .unwrap_or_else(|| n.body().to_vec());
        for field in &body {
            if field.is_memory_directive() {
                continue;
            }
            match field.key.as_str() {
                "events" | "random_events" => {
                    self.action_event_entries(field, &ctx);
                }
                "effect" => {
                    if field.children.is_some() {
                        if field.op.as_str() != "=" {
                            self.error(
                                field,
                                "operator",
                                "on_action effect block requires =",
                                None,
                            );
                        }
                        self.structure_script(field.body(), true);
                    } else {
                        self.gap(
                            field,
                            "scalar on_action effect form is not documented",
                            "on-action-effect-form-unverified",
                            None,
                        );
                    }
                }
                _ => self.gap(
                    field,
                    "on_action definition field is not yet modeled",
                    "rule-unimplemented",
                    None,
                ),
            }
        }
    }
    pub(crate) fn fire_action_parameters(
        &mut self,
        n: &Node,
        ctx: &Context,
        state: &mut State,
        inputs: &State,
    ) {
        self.fire_action_structure(n);
        if unexpanded(n) {
            return;
        }
        let Some(name) = n.val("on_action") else {
            return;
        };
        let mut frame = Context::unknown_caller();
        frame.object = format!("on-action:{name}.this");
        frame.root_object = format!("unknown-on-action-root:{name}");
        if let Some(scopes) = n.field("scopes") {
            for slot in scopes.body().iter().filter(|s| s.children.is_none()) {
                let Some(depth) = repeated(&slot.key.to_ascii_lowercase(), "from") else {
                    continue;
                };
                if depth > 6 {
                    self.complete = false;
                    self.gap(
                        slot,
                        "on_action FROM depth exceeds the six-frame analysis budget",
                        "analysis-limit",
                        Some(ctx),
                    );
                    continue;
                }
                if slot.scalar() == "none" {
                    self.gap(
                        slot,
                        "null on_action scope override semantics are not documented",
                        "on-action-null-scope-unverified",
                        Some(ctx),
                    );
                    continue;
                }
                let target = self.resolve(slot.scalar(), ctx, inputs, slot, false);
                frame
                    .froms
                    .resize(depth.max(frame.froms.len()), Types::new("?"));
                frame
                    .from_objects
                    .resize(depth.max(frame.from_objects.len()), String::new());
                frame.froms[depth - 1] = target.this;
                frame.from_objects[depth - 1] = target.object;
            }
        }
        if dynamic(name) {
            return;
        }
        let definitions = self.action_definitions(name);
        if definitions.is_empty() {
            return;
        }
        let active = format!("on-action:{name}");
        if self.active.contains(&active) || self.active.len() >= 64 {
            self.complete = false;
            self.gap(
                n,
                "recursive on_action execution exceeds analysis contract",
                "analysis-limit",
                Some(ctx),
            );
            return;
        }
        self.invoked_actions.insert(name.into());
        let depth = self.chain.len();
        self.active.push(active);
        self.chain.push(serde_json::json!({"kind":"on-action-call","on_action":name,"path":n.source,"line":n.line,"input_context":ctx,"action_context":frame,"default_context_proven":false,"runtime_invocation_proven":false}).into());
        self.gap(
            n,
            "on_action default THIS, ROOT, PREV and omitted FROM slots are not established",
            "on-action-default-context-unverified",
            Some(ctx),
        );
        if definitions.len() > 1 {
            self.gap(n, "multiple on_action fragments require engine composition verification; all possible fragments are checked", "on-action-definition-composition-unverified", Some(ctx));
        }
        for definition in &definitions {
            self.on_action_structure(definition);
            self.on_action_body(definition, &frame, state);
        }
        self.active.pop();
        self.chain.truncate(depth);
    }
    pub(crate) fn on_action_fallback(&mut self, n: &Node) {
        if n.is_memory_directive() {
            return;
        }
        let depth = self.chain.len();
        let entry = self
            .documented_action_entry(n)
            .or_else(|| self.planet_killer_entry(&n.key, "on_actions", n));
        if let Some(ctx) = entry {
            self.chain.push(serde_json::json!({"kind":"registered-engine-on-action","definition":n.key,"path":n.source,"line":n.line,"context":ctx,"caller_proven":false,"runtime_invocation_proven":false}).into());
            self.on_action_body(n, &ctx, &mut State::default());
            self.chain.truncate(depth);
            return;
        }
        if n.body().is_empty() || self.invoked_actions.contains(&n.key) {
            return;
        }
        self.chain.push(serde_json::json!({"kind":"uninvoked-on-action","definition":n.key,"path":n.source,"line":n.line,"caller_proven":false}).into());
        self.gap(
            n,
            "on_action has no proved scripted caller; engine entry context is unknown",
            "on-action-caller-context-unknown",
            None,
        );
        self.on_action_body(n, &Context::unknown_caller(), &mut State::default());
        self.chain.truncate(depth);
    }
    fn on_action_body(&mut self, n: &Node, ctx: &Context, caller: &mut State) {
        if unexpanded(n) {
            return;
        }
        let body = self
            .materialize(n.body(), ctx)
            .unwrap_or_else(|| n.body().to_vec());
        let mut input = caller.clone();
        input.clear_correlations();
        input.forget_numbers();
        for binding in input.bindings.values_mut() {
            binding.availability = Availability::Maybe;
        }
        for field in &body {
            match field.key.as_str() {
                "events" | "random_events" => {
                    if input.bindings.is_empty() {
                        self.on_action_list(field, ctx);
                    } else {
                        self.on_action_dispatch_list(field, ctx, &input);
                    }
                }
                "effect" if field.children.is_some() => {
                    self.gap(field, "on_action effect execution and default receiver contract require engine verification", "on-action-effect-engine-unverified", Some(ctx));
                    let mut outcome = input.clone();
                    self.effects(field.body(), ctx, &mut outcome);
                    // Only changed bindings are possible outputs. An unknown
                    // callback must never lend an unconditional target write.
                    let keys = input
                        .bindings
                        .keys()
                        .chain(outcome.bindings.keys())
                        .cloned()
                        .collect::<BTreeSet<_>>();
                    for key in keys {
                        if input.get(&key) != outcome.get(&key) {
                            caller.set(key.clone(), caller.get(&key).join(&outcome.get(&key)));
                        }
                    }
                    caller.clear_correlations();
                }
                _ => {}
            }
        }
    }
    pub(crate) fn on_action_dispatch_list(&mut self, list: &Node, ctx: &Context, input: &State) {
        for (item, enabled) in self.action_event_entries(list, ctx) {
            if !enabled {
                continue;
            }
            let (scope, _) = self.program.events[item.scalar()].clone();
            let mut entry = Context::new(&scope);
            // Only engine annotations can supply a known ROOT here. Script
            // dispatch deliberately leaves the default ROOT unknown.
            entry.root = ctx.root.clone();
            entry.root_object = ctx.root_object.clone();
            entry.froms = ctx.froms.clone();
            entry.from_objects = ctx.from_objects.clone();
            entry.prev_tail_unknown = true;
            self.gap(
                &item,
                "on_action callback timing and runtime input validity remain unverified; ROOT types use only explicit entry evidence",
                "engine-contract-unverified",
                Some(ctx),
            );
            self.chain.push(serde_json::json!({"kind":"on-action-event","path":item.source,"line":item.line,"event":item.scalar(),"context":entry,"documented_root_type":!ctx.root.unknown,"runtime_input_validity_proven":false}).into());
            self.schedule(item.scalar(), entry, input.clone());
            self.chain.pop();
        }
    }
}
