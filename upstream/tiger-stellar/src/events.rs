use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::{Engine, Pending},
};
use std::collections::BTreeSet;

impl Engine {
    pub(crate) fn schedule(&mut self, name: &str, mut ctx: Context, mut state: State) {
        // A receiving event can run on another object of the same type. A
        // caller's is_ai condition is not the receiving ROOT's condition.
        state.clear_correlations();
        // Future event execution is not an atomic continuation of current
        // numeric values. Other script execution can change them meanwhile.
        state.forget_numbers();
        ctx.object = format!("event:{name}.this");
        if ctx.root_object == "root" {
            ctx.root_object = ctx.object.clone();
        }
        let key = format!("{name}\n{}", serde_json::to_string(&ctx).unwrap());
        if !self.event_inputs.contains_key(&key)
            && self
                .event_inputs
                .keys()
                .filter(|k| k.starts_with(&format!("{name}\n")))
                .count()
                >= 64
        {
            self.complete = false;
            let n = self.program.events[name].1.clone();
            self.gap(
                &n,
                format!("event input context budget reached: {name}"),
                "analysis-limit",
                Some(&ctx),
            );
            return;
        }
        let reads = self
            .reads
            .get(&format!("event:{name}"))
            .cloned()
            .unwrap_or_else(|| BTreeSet::from(["*".into()]));
        let state = state.project(&reads);
        let changed = if let Some(old) = self.event_inputs.get_mut(&key) {
            let merged = old.state.join(&state);
            if merged == old.state {
                false
            } else {
                old.state = merged;
                true
            }
        } else {
            self.event_inputs.insert(
                key.clone(),
                Pending {
                    ctx,
                    state,
                    chain: self.chain.clone(),
                },
            );
            true
        };
        if changed && self.queued.insert(key.clone()) {
            self.queue.push_back(key);
        }
    }
    pub fn analyze(&mut self) {
        let total = std::time::Instant::now();
        let mut phase = std::time::Instant::now();
        self.prepare_templates();
        self.phase_seconds
            .insert("templates".into(), phase.elapsed().as_secs_f64());
        phase = std::time::Instant::now();
        let mut input_phase = std::time::Instant::now();
        self.prepare_reads();
        self.phase_seconds.insert(
            "input_demands_reads".into(),
            input_phase.elapsed().as_secs_f64(),
        );
        input_phase = std::time::Instant::now();
        self.prepare_saved_target_types();
        self.phase_seconds.insert(
            "input_demands_saved_targets".into(),
            input_phase.elapsed().as_secs_f64(),
        );
        input_phase = std::time::Instant::now();
        self.prepare_documented_actions();
        self.phase_seconds.insert(
            "input_demands_documented_actions".into(),
            input_phase.elapsed().as_secs_f64(),
        );
        self.phase_seconds
            .insert("input_demands".into(), phase.elapsed().as_secs_f64());
        phase = std::time::Instant::now();
        let targets = self
            .program
            .files
            .iter()
            .filter(|f| f.target)
            .cloned()
            .collect::<Vec<_>>();
        for file in &targets {
            if file.category == "events" {
                for n in &file.ast {
                    if let Some(id) = n.val("id")
                        && n.val("is_triggered_only") != Some("yes")
                        && let Some((scope, _)) = self.program.events.get(id).cloned()
                    {
                        let mut c = Context::new(&scope);
                        if scope == "none" {
                            c.root = Types::new("?");
                        }
                        self.schedule(id, c, State::default());
                    }
                }
            } else if !matches!(
                file.category.as_str(),
                "scripted_effects" | "scripted_triggers" | "scripted_variables"
            ) {
                for n in &file.ast {
                    self.definition(&file.category, n);
                }
            }
        }
        self.phase_seconds
            .insert("target_declarations".into(), phase.elapsed().as_secs_f64());
        phase = std::time::Instant::now();
        self.drain_events();
        self.phase_seconds
            .insert("reached_events".into(), phase.elapsed().as_secs_f64());
        phase = std::time::Instant::now();
        // Actions can provide an event's first legitimate input. Visit those
        // entries before a generic uninvoked-event fallback invents a ROOT.
        for file in targets.iter().filter(|f| f.category == "on_actions") {
            for n in &file.ast {
                self.on_action_fallback(n);
            }
        }
        self.drain_events();
        self.phase_seconds
            .insert("uninvoked_on_actions".into(), phase.elapsed().as_secs_f64());
        phase = std::time::Instant::now();
        // A target event without a proved caller still receives a check, with
        // unknown input facts. This does not prove an engine invocation.
        for file in &targets {
            if file.category != "events" {
                continue;
            }
            for n in &file.ast {
                if let Some(id) = n.val("id")
                    && !self.visited_events.contains(id)
                    && let Some((scope, _)) = self.program.events.get(id).cloned()
                {
                    let mut ctx = Context::new(&scope);
                    if scope == "none" {
                        ctx.root = Types::new("?");
                    }
                    self.schedule(id, ctx, State::default());
                }
            }
        }
        self.drain_events();
        self.phase_seconds
            .insert("uninvoked_events".into(), phase.elapsed().as_secs_f64());
        phase = std::time::Instant::now();
        let script_contracts = self
            .program
            .scripts
            .iter()
            .filter(|(_, n)| targets.iter().any(|f| f.path == n.source.as_ref()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        // Registered engine entries are independent calling roots, including
        // when an ordinary script also calls the same definition. Check these
        // before fallback so their real helper contexts are not lost to name
        // ordering in the generic unused-script pass.
        let mut registered = BTreeSet::new();
        for ((effect, name), n) in &script_contracts {
            if self.run_engine_script_entry(*effect, name, n) {
                registered.insert((*effect, name.clone()));
            }
        }
        for ((effect, name), n) in script_contracts {
            if registered.contains(&(effect, name.clone()))
                || self.invoked.contains(&(effect, name.clone()))
            {
                continue;
            }
            self.gap(
                &n,
                format!("scripted definition has no inferred caller context: {name}"),
                "caller-context-unknown",
                None,
            );
            let chain_depth = self.chain.len();
            self.chain.push(serde_json::json!({"kind":"uninvoked-script-contract","definition":name,"path":n.source,"line":n.line,"caller_proven":false,"engine_entry_registered":false}).into());
            let ctx = self.inferred_script_context(effect, &name, &n);
            let mut state = State::default();
            if n.macro_parts.is_none() {
                if effect {
                    self.effects(n.body(), &ctx, &mut state);
                } else {
                    self.triggers(n.body(), &ctx, &state);
                }
            }
            self.chain.truncate(chain_depth);
        }
        // Uninvoked target scripts are checked under an explicitly hypothetical
        // input contract. Calls discovered there still require callee checks;
        // previously their queued events/projects were left undrained.
        self.drain_events();
        self.phase_seconds
            .insert("uninvoked_scripts".into(), phase.elapsed().as_secs_f64());
        self.uninvoked_script_values();
        phase = std::time::Instant::now();
        self.diagnostics.sorted();
        self.phase_seconds
            .insert("diagnostic_sort".into(), phase.elapsed().as_secs_f64());
        self.phase_seconds
            .insert("semantic_total".into(), total.elapsed().as_secs_f64());
    }
    fn drain_events(&mut self) {
        let mut budget = 10000;
        while !self.queue.is_empty() || !self.project_queue.is_empty() {
            if budget == 0 {
                self.complete = false;
                let n = Node::default();
                self.gap(
                    &n,
                    "event transfer worklist budget exhausted",
                    "analysis-limit",
                    None,
                );
                return;
            }
            budget -= 1;
            if self.queue.is_empty() {
                let key = self.project_queue.pop_front().unwrap();
                self.project_callbacks(&key);
                continue;
            }
            let key = self.queue.pop_front().unwrap();
            self.queued.remove(&key);
            let name = key.split('\n').next().unwrap().to_string();
            let input = self.event_inputs[&key].clone();
            let Some((_, n)) = self.program.events.get(&name).cloned() else {
                continue;
            };
            self.visited_events.insert(name.clone());
            self.metrics.event_evaluations += 1;
            let saved = std::mem::replace(&mut self.chain, input.chain);
            self.chain.push(serde_json::json!({"kind":"event-body","definition":name,"path":n.source,"line":n.line,"context":input.ctx}).into());
            self.event_body(&n, &input.ctx, &mut input.state.clone());
            self.chain = saved;
        }
    }
    pub(crate) fn event_body(&mut self, n: &Node, ctx: &Context, state: &mut State) {
        let body = self
            .materialize(n.body(), ctx)
            .unwrap_or_else(|| n.body().to_vec());
        if n.field("base").is_some() {
            self.migration_gaps.insert("event_inheritance".into());
            self.gap(
                n,
                "event inheritance not yet migrated to Rust",
                "rule-unimplemented",
                Some(ctx),
            );
        }
        for c in &body {
            match c.key.as_str() {
                "trigger" | "abort_trigger" | "show_on_map_trigger" => {
                    if self.block(c) {
                        self.triggers(c.body(), ctx, state);
                    }
                }
                "mean_time_to_happen" | "weight_multiplier" => self.weighted(c, ctx, state),
                "id" | "base" | "immediate" | "option" | "after" => {}
                "is_triggered_only"
                | "hide_window"
                | "fire_only_once"
                | "major"
                | "major_triggered_only"
                | "diplomatic"
                | "notification"
                | "auto_opens"
                | "trackable"
                | "is_advisor_event"
                | "is_test_event"
                | "astral_rift"
                | "first_contact"
                | "espionage_operation"
                | "archaeology"
                | "force_open"
                | "auto_select" => self.boolean(c),
                "title"
                | "desc"
                | "message_desc"
                | "triggered_desc"
                | "picture"
                | "event_picture_background"
                | "notification_event_icon"
                | "picture_event_data" => {}
                "event_chain" | "specimen" => {
                    if self.scalar_field(c) {
                        let category = if c.key == "event_chain" {
                            "event_chains"
                        } else {
                            "specimens"
                        };
                        self.symbol_reference(category, c.scalar(), c);
                    }
                }
                "situation" => {
                    if self.scalar_field(c) {
                        let target = self.resolve(c.scalar(), ctx, state, c, false);
                        self.compatible(
                            &target.this,
                            &["situation".into()],
                            c,
                            "event situation",
                            ctx,
                        );
                    }
                }
                "show_sound" | "location" | "diplomatic_title" | "event_window_type"
                | "event_message_type" => {
                    self.scalar_field(c);
                }
                "duration" | "difficulty" | "notification_event_icon_frame" => {
                    self.numeric(c, Some(ctx))
                }
                k if k.starts_with('@') => {}
                k if k.strip_suffix("_clear").is_some_and(|base| {
                    matches!(
                        base,
                        "desc"
                            | "option"
                            | "trigger"
                            | "picture"
                            | "show_sound"
                            | "title"
                            | "immediate"
                            | "after"
                    )
                }) =>
                {
                    self.boolean(c)
                }
                _ => {
                    self.gap(
                        c,
                        format!("event field contract not yet migrated: {}", c.key),
                        "rule-unimplemented",
                        Some(ctx),
                    );
                    self.migration_gaps.insert("event_metadata".into());
                }
            }
        }
        for block in body.iter().filter(|c| c.key == "immediate") {
            if self.block(block) {
                let effects = block
                    .body()
                    .iter()
                    .filter(|c| c.key != "run_in_ai_mode")
                    .cloned()
                    .collect::<Vec<_>>();
                for c in block.body().iter().filter(|c| c.key == "run_in_ai_mode") {
                    self.boolean(c);
                }
                self.effects(&effects, ctx, state);
            }
        }
        // Installed paragon.1 creates/saves beholder_leader in immediate and
        // uses it as the event portrait. Display expressions consume the
        // post-immediate state, before any option or after effects.
        for field in &body {
            match field.key.as_str() {
                "title" | "desc" | "message_desc" | "triggered_desc" => {
                    self.description(field, ctx, state)
                }
                "picture" => self.conditional_picture(field, ctx, state),
                "event_picture_background" | "notification_event_icon" => {
                    self.option_icon(field, ctx, state)
                }
                "picture_event_data" => self.picture_event_data(field, ctx, state),
                _ => {}
            }
        }
        let mut outcomes = Vec::new();
        for option in body.iter().filter(|c| c.key == "option") {
            if !self.block(option) {
                continue;
            }
            // Inline scripts can insert option metadata as well as effects.
            // The shipped events/dynamic_resource_cost inserts `allow`; classify
            // it here, before generic effect expansion would treat it as code.
            let option_body = self
                .materialize(option.body(), ctx)
                .unwrap_or_else(|| option.body().to_vec());
            let mut current = state.clone();
            let mut effects = Vec::new();
            for c in &option_body {
                match c.key.as_str() {
                    "trigger" | "allow" | "highlight" | "exclusive_trigger" => {
                        if self.block(c) {
                            self.triggers(c.body(), ctx, &current);
                        }
                    }
                    "ai_chance" => self.weighted(c, ctx, &current),
                    "is_dialog_only"
                    | "default_hide_option"
                    | "hide_option_if_not_allowed"
                    | "default" => self.boolean(c),
                    "name" | "custom_tooltip" => self.localisation(c),
                    "icon" => self.option_icon(c, ctx, &current),
                    "response_text" | "sound" | "show_sound" | "tag" => {
                        self.scalar_field(c);
                    }
                    _ => effects.push(c.clone()),
                }
            }
            // Infeasible options still have statically checkable grammar and
            // fixed references. They must not execute their state transitions.
            self.structure_script(&effects, true);
            let mut frame = Some(ctx.clone());
            for condition in ["trigger", "allow"] {
                let conditions = option_body
                    .iter()
                    .filter(|c| c.key == condition)
                    .collect::<Vec<_>>();
                if conditions.len() > 1 {
                    // No engine contract establishes first/last/AND semantics
                    // for repeated condition containers, including inline ones.
                    self.gap(
                        option,
                        format!(
                            "repeated event option {condition} composition requires verification"
                        ),
                        "event-option-condition-composition-unverified",
                        Some(ctx),
                    );
                } else if let (Some(t), Some(before)) = (conditions.first(), frame.clone()) {
                    frame = self.refine(t.body(), &before, &mut current, true);
                }
            }
            if let Some(frame) = frame {
                self.effects(&effects, &frame, &mut current);
                outcomes.push(current);
            }
        }
        if let Some(first) = outcomes.first() {
            *state = outcomes[1..].iter().fold(first.clone(), |a, b| a.join(b));
        }
        for block in body.iter().filter(|c| c.key == "after") {
            if self.block(block) {
                self.effects(block.body(), ctx, state);
            }
        }
    }
}
