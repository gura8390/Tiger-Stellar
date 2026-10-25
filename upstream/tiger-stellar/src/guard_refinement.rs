//! Factored boolean guard transfer. Alternatives join facts; they never export
//! a fact belonging to only one arm. No binding-path combinations are enumerated.
use crate::{
    ast::Node,
    domain::{Availability, Context, NumberFact, State, Types, alias},
    engine::Engine,
};
use std::collections::BTreeSet;
struct GuardWork {
    remaining: usize,
    active: BTreeSet<String>,
    scope_predicates: bool,
}

impl Engine {
    pub(crate) fn refine(
        &mut self,
        nodes: &[Node],
        ctx: &Context,
        state: &mut State,
        positive: bool,
    ) -> Option<Context> {
        self.refine_mode(nodes, ctx, state, positive, true)
    }
    pub(crate) fn refine_guard_entry(
        &mut self,
        nodes: &[Node],
        ctx: &Context,
        state: &mut State,
    ) -> Option<Context> {
        self.refine_mode(nodes, ctx, state, true, false)
    }
    fn refine_mode(
        &mut self,
        nodes: &[Node],
        ctx: &Context,
        state: &mut State,
        positive: bool,
        scope_predicates: bool,
    ) -> Option<Context> {
        self.refine_guard(
            nodes,
            ctx,
            state,
            positive,
            0,
            &mut GuardWork {
                remaining: 512,
                active: BTreeSet::new(),
                scope_predicates,
            },
        )
    }
    fn guard_group(
        &mut self,
        nodes: &[Node],
        ctx: &Context,
        state: &mut State,
        mode: (bool, bool),
        depth: usize,
        work: &mut GuardWork,
    ) -> Option<Context> {
        let (conjunctive, positive) = mode;
        if conjunctive == positive {
            let mut frame = ctx.clone();
            for node in nodes {
                if work.remaining == 0 {
                    break;
                }
                frame = self.refine_guard(
                    std::slice::from_ref(node),
                    &frame,
                    state,
                    positive,
                    depth + 1,
                    work,
                )?;
            }
            Some(frame)
        } else {
            let mut joined: Option<(Context, State)> = None;
            for node in nodes {
                if work.remaining == 0 {
                    return Some(ctx.clone());
                }
                let mut arm = state.clone();
                if let Some(frame) = self.refine_guard(
                    std::slice::from_ref(node),
                    ctx,
                    &mut arm,
                    positive,
                    depth + 1,
                    work,
                ) {
                    joined = Some(match joined {
                        None => (frame, arm),
                        Some((mut accumulated, facts)) => {
                            accumulated.this = accumulated.this.join(&frame.this);
                            (accumulated, facts.join(&arm))
                        }
                    });
                }
            }
            joined.map(|(frame, facts)| {
                *state = facts;
                frame
            })
        }
    }
    fn refine_guard(
        &mut self,
        nodes: &[Node],
        ctx: &Context,
        state: &mut State,
        positive: bool,
        depth: usize,
        work: &mut GuardWork,
    ) -> Option<Context> {
        // A precision bound, never permission to assume a predicate succeeds.
        if depth >= 64 || work.remaining == 0 {
            return Some(ctx.clone());
        }
        work.remaining -= 1;
        if let Some(key) = self.guard_key(nodes, ctx, positive) {
            state.assume(&key);
        }
        if nodes.len() != 1 {
            return self.guard_group(nodes, ctx, state, (true, positive), depth, work);
        }
        let n = &nodes[0];
        let key = n.key.to_ascii_lowercase();
        match key.as_str() {
            "and" | "hidden_trigger" if n.children.is_some() => {
                return self.guard_group(n.body(), ctx, state, (true, positive), depth + 1, work);
            }
            "or" if n.children.is_some() => {
                return self.guard_group(n.body(), ctx, state, (false, positive), depth + 1, work);
            }
            "nor" if n.children.is_some() => {
                return self.guard_group(n.body(), ctx, state, (false, !positive), depth + 1, work);
            }
            "nand" if n.children.is_some() => {
                return self.guard_group(n.body(), ctx, state, (true, !positive), depth + 1, work);
            }
            "not" if n.body().len() == 1 => {
                return self.refine_guard(n.body(), ctx, state, !positive, depth + 1, work);
            }
            // The singular NOT contract does not establish how a multi-entry
            // NOT should be combined. Retain the input facts for that form.
            "not" => return Some(ctx.clone()),
            _ => {}
        }
        // THIS is an alias of the current object, but it still pushes a scope
        // frame for inner PREV reads. Project only the narrowed current type
        // back to the caller; ROOT/FROM/PREV and optional access are distinct.
        if n.children.is_some()
            && n.op.as_str() == "="
            && !n.key.contains('?')
            && n.key
                .split('.')
                .all(|part| part.eq_ignore_ascii_case("this"))
        {
            let mut selected = ctx.clone();
            for _ in n.key.split('.') {
                let object = selected.object.clone();
                selected = selected.push(selected.this.clone(), "this");
                selected.object = object;
            }
            let refined =
                self.refine_guard(n.body(), &selected, state, positive, depth + 1, work)?;
            let mut frame = ctx.clone();
            frame.this = refined.this;
            return Some(frame);
        }
        if !self.rules.triggers.contains_key(&n.key)
            && let Some(def) = self.program.scripts.get(&(false, n.key.clone())).cloned()
            && (def.macro_parts.is_some() || matches!(n.value.as_deref(), Some("yes" | "no")))
        {
            if !work.active.insert(n.key.clone()) {
                return Some(ctx.clone());
            }
            let result = if def.macro_parts.is_some() {
                if let Some(body) = self.macro_guard_body(n, ctx, &def) {
                    self.refine_guard(&body, ctx, state, positive, depth + 1, work)
                } else {
                    Some(ctx.clone())
                }
            } else {
                self.refine_guard(
                    def.body(),
                    ctx,
                    state,
                    positive == (n.scalar() == "yes"),
                    depth + 1,
                    work,
                )
            };
            work.active.remove(&n.key);
            return result;
        }
        // A successful ordinary named-scope predicate establishes its own
        // receiver and can constrain that receiver's type. Optional access can
        // succeed without the receiver; a negative predicate can likewise be
        // true because the receiver was absent. Neither proves existence.
        let (head, tail) = n
            .key
            .split_once('.')
            .map_or((n.key.as_str(), None), |(h, t)| (h, Some(t)));
        if work.scope_predicates
            && n.children.is_some()
            && n.op.as_str() == "="
            && (head.starts_with("event_target:") || head.starts_with("global_event_target:"))
            && !n.key.contains('?')
            && !head.contains(['@', '$', '[', ']'])
            && tail.is_none_or(|t| t.split('.').all(|s| s.eq_ignore_ascii_case("this")))
        {
            let mut binding = state.target_binding(head);
            if !positive && binding.availability != Availability::Definite {
                return Some(ctx.clone());
            }
            if binding.types.known.is_empty()
                && !binding.types.unknown
                && let Some(types) = self.saved_target_type_candidates(head)
            {
                binding.types = types;
            }
            if binding.types.known.is_empty() {
                binding.types.unknown = true;
            }
            let actual = if state.bindings.contains_key(head) {
                head.to_string()
            } else if head.starts_with("event_target:") && state.bindings.contains_key(&alias(head))
            {
                alias(head)
            } else {
                head.to_string()
            };
            let mut selected = state.clone();
            binding.availability = Availability::Definite;
            selected.bindings.insert(actual.clone(), binding.clone());
            let mut frame = ctx.push(binding.types, head);
            if let Some(tail) = tail {
                for _ in tail.split('.') {
                    let object = frame.object.clone();
                    frame = frame.push(frame.this.clone(), "this");
                    frame.object = object;
                }
            }
            let refined =
                self.refine_guard(n.body(), &frame, &mut selected, positive, depth + 1, work)?;
            let mut receiver = selected.get(&actual);
            receiver.types = refined.this;
            selected.bindings.insert(actual, receiver);
            *state = selected;
            return Some(ctx.clone());
        }
        let mut c = ctx.clone();
        if key == "is_variable_set" && crate::variables::variable_name(n.scalar()) {
            let name = State::number_key(&ctx.object, n.scalar());
            let fact = state.number(&name);
            if (positive && fact.availability == Availability::Absent)
                || (!positive && fact.availability == Availability::Definite)
            {
                return None;
            }
            state.numbers.insert(
                name,
                NumberFact {
                    availability: if positive {
                        Availability::Definite
                    } else {
                        Availability::Absent
                    },
                    constant: if positive { fact.constant } else { None },
                    nonzero: positive && fact.nonzero,
                },
            );
        }
        if key == "check_variable"
            && let (Some(name), Some(value)) = (n.val("which"), n.field("value"))
            && crate::variables::variable_name(name)
            && let Ok(bound) = value.scalar().parse::<f64>()
            && bound.is_finite()
            && (bound as f32) as f64 == bound
        {
            let key = State::number_key(&ctx.object, name);
            let mut fact = state.number(&key);
            if let Some(actual) = fact.value() {
                let truth = match value.op.as_str() {
                    "=" | "==" => Some(actual == bound),
                    "!=" => Some(actual != bound),
                    ">" => Some(actual > bound),
                    ">=" => Some(actual >= bound),
                    "<" => Some(actual < bound),
                    "<=" => Some(actual <= bound),
                    _ => None,
                };
                if truth.is_some_and(|truth| truth != positive) {
                    return None;
                }
            }
            let op = value.op.as_str();
            let zero = bound == 0.0
                && ((matches!(op, "=" | "==") && positive) || (op == "!=" && !positive));
            let nonzero = if positive {
                (op == "!=" && bound == 0.0)
                    || (op == ">" && bound >= 0.0)
                    || (op == ">=" && bound > 0.0)
                    || (op == "<" && bound <= 0.0)
                    || (op == "<=" && bound < 0.0)
                    || (matches!(op, "=" | "==") && bound != 0.0)
            } else {
                (matches!(op, "=" | "==") && bound == 0.0)
                    || (op == ">=" && bound <= 0.0)
                    || (op == ">" && bound < 0.0)
                    || (op == "<=" && bound >= 0.0)
                    || (op == "<" && bound > 0.0)
                    || (op == "!=" && bound != 0.0)
            };
            if zero {
                if fact.nonzero {
                    return None;
                }
                fact.constant = Some("0".into());
                fact.nonzero = false;
            } else if nonzero {
                fact.nonzero = true;
            }
            state.numbers.insert(key, fact);
        }
        if key == "always"
            && matches!(n.scalar(), "yes" | "no")
            && (n.scalar() == "yes") != positive
        {
            return None;
        }
        if key == "is_scope_type" {
            if n.children.is_some()
                || n.op.as_str() != "="
                || !crate::rules::SCOPES
                    .split_whitespace()
                    .any(|s| s == n.scalar())
            {
                // Invalid/dynamic kinds retain their static diagnostics, but
                // never become a fabricated concrete type or an empty path.
                return Some(c);
            }
            if positive {
                if !c.this.unknown && !c.this.known.contains(n.scalar()) {
                    return None;
                }
                c.this = Types::new(n.scalar());
            } else if !c.this.unknown {
                c.this.known.remove(n.scalar());
                if c.this.known.is_empty() {
                    return None;
                }
            }
        }
        if key == "exists" && positive {
            let key = n.scalar();
            if (key.starts_with("event_target:") || key.starts_with("global_event_target:"))
                && !key.contains(['.', '@', '$', '[', ']', '?'])
            {
                let actual = if state.bindings.contains_key(key) {
                    key.to_string()
                } else if key.starts_with("event_target:")
                    && state.bindings.contains_key(&alias(key))
                {
                    alias(key)
                } else {
                    key.to_string()
                };
                let mut binding = state.get(&actual);
                if binding.types.known.is_empty()
                    && !binding.types.unknown
                    && let Some(types) = self.saved_target_type_candidates(&actual)
                {
                    // Populate a newly introduced existence fact, without
                    // narrowing a previously open or future-rebound binding.
                    binding.types = types;
                }
                binding.availability = Availability::Definite;
                if binding.types.known.is_empty() {
                    binding.types.unknown = true;
                }
                state.bindings.insert(actual, binding);
            }
        }
        Some(c)
    }
}
