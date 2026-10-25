use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::Engine,
    loader::event_scope,
    rules::Rule,
    templates::render,
};
use std::collections::{BTreeMap, BTreeSet};

struct Expansion {
    body: Vec<Node>,
    syntax_clean: bool,
}
struct ScriptExpansion {
    body: std::sync::Arc<Vec<Node>>,
    syntax_clean: bool,
    wrapper_intact: bool,
}
#[derive(Clone, Copy)]
enum ScriptReturn {
    Boolean,
    Count,
    Unknown,
}

impl Engine {
    pub(crate) fn scripted_call_shape(&mut self, n: &Node, effect: bool) -> bool {
        let Some(def) = self.program.scripts.get(&(effect, n.key.clone())).cloned() else {
            return false;
        };
        if let Some(parts) = &def.macro_parts {
            let mut args = BTreeMap::new();
            for field in n.body() {
                if field.children.is_some() {
                    self.gap(
                        field,
                        "block-valued scripted macro argument",
                        "rule-unimplemented",
                        None,
                    );
                    return true;
                }
                args.insert(field.key.clone(), field.scalar().to_string());
            }
            let source = parts
                .iter()
                .map(|part| {
                    if part.parameter {
                        format!("${}$", part.text)
                    } else {
                        part.text.clone()
                    }
                })
                .collect::<String>();
            if let Err(error) = render(&source, &args) {
                self.error(n, "macro-argument", error, None);
            }
        }
        true
    }
    fn script_return(&self, def: &Node, visiting: &mut BTreeSet<String>) -> ScriptReturn {
        let body = def
            .body()
            .iter()
            .filter(|n| !n.is_memory_directive() && !n.key.starts_with('@'))
            .collect::<Vec<_>>();
        if body.len() != 1 {
            return if body.iter().any(|n| {
                n.key.starts_with("count_")
                    || matches!(n.key.as_str(), "add" | "subtract" | "multiply" | "divide")
            }) {
                ScriptReturn::Unknown
            } else {
                ScriptReturn::Boolean
            };
        }
        let n = body[0];
        // Native pop_amount/sapient_pop_amount wrap count_owned_pop_amount
        // without a threshold. A known count iterator yields the number;
        // count={...threshold...} instead yields a boolean predicate.
        if n.key.starts_with("count_")
            && n.children.is_some()
            && n.field("count").is_none()
            && self
                .rules
                .triggers
                .get(&n.key)
                .is_some_and(|r| r.iterator_output.is_some())
        {
            return ScriptReturn::Count;
        }
        if n.scalar() == "yes"
            && !self.rules.triggers.contains_key(&n.key)
            && let Some(callee) = self.program.scripts.get(&(false, n.key.clone()))
        {
            if !visiting.insert(n.key.clone()) {
                return ScriptReturn::Unknown;
            }
            let result = self.script_return(callee, visiting);
            visiting.remove(&n.key);
            return result;
        }
        ScriptReturn::Boolean
    }
    pub(crate) fn materialize(&mut self, nodes: &[Node], ctx: &Context) -> Option<Vec<Node>> {
        if !nodes.iter().any(|n| n.key == "inline_script") {
            return None;
        }
        let mut out = Vec::new();
        for n in nodes {
            if n.key == "inline_script" {
                if let Some(body) = self.expand_inline(n, ctx) {
                    out.extend(body);
                }
            } else {
                out.push(n.clone());
            }
        }
        Some(out)
    }
    pub(crate) fn expand_inline(&mut self, n: &Node, ctx: &Context) -> Option<Vec<Node>> {
        self.expand_inline_mode(n, ctx, true)
    }
    fn expand_inline_mode(&mut self, n: &Node, ctx: &Context, deep: bool) -> Option<Vec<Node>> {
        self.chain.push(serde_json::json!({"kind":"inline-call","caller":n.source,"line":n.line,"name":n.value.as_deref().or(n.val("script")).unwrap_or("<missing>"),"parameters":n.body().iter().filter(|c|c.key!="script").map(|c|c.key.as_str()).collect::<Vec<_>>(),"context":ctx}).into());
        let result = self.expand_inline_body(n, ctx, deep);
        self.chain.pop();
        result
    }
    fn expand_inline_body(&mut self, n: &Node, ctx: &Context, deep: bool) -> Option<Vec<Node>> {
        let name = n
            .value
            .as_deref()
            .or(n.val("script"))
            .unwrap_or("")
            .replace('\\', "/")
            .to_ascii_lowercase();
        let key = format!("inline:{name}");
        if self.active.contains(&key) || self.active.len() >= 40 {
            self.gap(
                n,
                format!("recursive inline script: {name}"),
                "analysis-limit",
                Some(ctx),
            );
            self.complete = false;
            return None;
        }
        let Some((path, text)) = self.program.inline.get(&name).cloned() else {
            self.error(
                n,
                "missing-inline-script",
                format!("unknown inline script {name}"),
                Some(ctx),
            );
            return None;
        };
        let mut args = BTreeMap::new();
        for c in n.body() {
            if c.key != "script" {
                if c.children.is_some() {
                    self.gap(
                        c,
                        "block-valued inline argument is not supported",
                        "rule-unimplemented",
                        Some(ctx),
                    );
                    return None;
                }
                args.insert(c.key.clone(), c.scalar().to_string());
            }
        }
        let text = match render(&text, &args) {
            Ok(t) => t,
            Err(e) => {
                self.error(n, "macro-argument", e, Some(ctx));
                return None;
            }
        };
        let header = n
            .reader_globals
            .clone()
            .unwrap_or_else(|| self.program.reader_header(&n.source).into());
        self.active.push(key);
        let result = self.parse_expanded_with_header(&path, &text, n, ctx, header);
        let result = result.map(|parsed| {
            if deep {
                self.expand_tree(&parsed.body, ctx)
            } else {
                let mut roots = vec![];
                for node in parsed.body {
                    if node.key == "inline_script" {
                        if let Some(body) = self.expand_inline_mode(&node, ctx, false) {
                            roots.extend(body);
                        }
                    } else if event_scope(&node.key).is_some() && node.val("id").is_none() {
                        // IDs supplied by nested header templates still need
                        // expansion before they can enter the event index.
                        roots.extend(self.expand_tree(std::slice::from_ref(&node), ctx));
                    } else {
                        roots.push(node);
                    }
                }
                roots
            }
        });
        self.active.pop();
        result
    }
    pub(crate) fn script_value_body(
        &mut self,
        def: &Node,
        call: &Node,
        args: &BTreeMap<String, String>,
        ctx: &Context,
    ) -> Option<Vec<Node>> {
        let Some(parts) = &def.macro_parts else {
            if !args.is_empty() {
                self.gap(
                    call,
                    "arguments supplied to a script value without macro parameters require verification",
                    "numeric-information-unresolved",
                    Some(ctx),
                );
                return None;
            }
            return Some(def.body().to_vec());
        };
        let source = parts
            .iter()
            .map(|p| {
                if p.parameter {
                    format!("${}$", p.text)
                } else {
                    p.text.clone()
                }
            })
            .collect::<String>();
        let text = match render(&source, args) {
            Ok(text) => text,
            Err(error) => {
                self.error(call, "macro-argument", error, Some(ctx));
                return None;
            }
        };
        let vars = def
            .macro_variables
            .iter()
            .map(|(k, v)| format!("@{k}={v}\n"))
            .collect::<String>();
        let parsed = self.parse_expanded(
            &def.source,
            &format!("{vars}expanded={{\n{text}\n}}"),
            call,
            ctx,
        )?;
        if !parsed.syntax_clean {
            return None;
        }
        if parsed.body.len() != 1
            || parsed.body[0].key != "expanded"
            || parsed.body[0].children.is_none()
            || parsed.body[0].value.is_some()
        {
            self.gap(
                call,
                "script-value replacement escaped its synthetic body wrapper; numeric composition is unverified",
                "script-value-expansion-boundary-unverified",
                Some(ctx),
            );
            let mut body = Vec::new();
            for (index, mut root) in parsed.body.into_iter().enumerate() {
                if index == 0 && root.key == "expanded" && root.children.is_some() {
                    body.extend(root.children.take().unwrap().into_vec());
                } else {
                    body.push(root);
                }
            }
            relocate(&mut body, def.line as i64 - vars.lines().count() as i64 - 2);
            let partial = Node {
                children: Some(body.into_boxed_slice()),
                macro_parts: None,
                ..def.clone_header()
            };
            // Keep context-free structure/reference checks for every root,
            // but do not evaluate a truncated body or manufacture a constant.
            self.script_value_structure(&partial);
            return None;
        }
        let mut body = parsed.body.into_iter().next()?.children?.into_vec();
        fn relocate(nodes: &mut [Node], shift: i64) {
            for node in nodes {
                node.line = (node.line as i64 + shift).max(1) as u32;
                if let Some(children) = node.children.as_mut() {
                    relocate(children, shift);
                }
            }
        }
        relocate(&mut body, def.line as i64 - vars.lines().count() as i64 - 2);
        Some(body)
    }
    fn parse_expanded(
        &mut self,
        path: &str,
        text: &str,
        call: &Node,
        ctx: &Context,
    ) -> Option<Expansion> {
        let header = self.program.reader_header(path).into();
        self.parse_expanded_with_header(path, text, call, ctx, header)
    }
    fn parse_expanded_with_header(
        &mut self,
        path: &str,
        text: &str,
        call: &Node,
        ctx: &Context,
        header: std::sync::Arc<str>,
    ) -> Option<Expansion> {
        let parsed = self.parser.parse(path, text, &header);
        match parsed {
            Err(e) => {
                self.error(call, "macro-argument", e, Some(ctx));
                None
            }
            Ok(mut p) => {
                let syntax_clean = p.diagnostics.is_empty();
                for d in p.diagnostics {
                    self.diagnostics.emit(
                        call,
                        d["key"].as_str().unwrap_or("syntax"),
                        d["message"]
                            .as_str()
                            .unwrap_or("expansion diagnostic")
                            .into(),
                        (
                            d["severity"].as_str().unwrap_or("error"),
                            "confirmed-rule-violation",
                        ),
                        Some(ctx),
                        &self.chain,
                    );
                }
                for c in &mut p.ast {
                    c.locate(path, &mut self.program.next_id);
                    fn reader_context(n: &mut Node, header: &std::sync::Arc<str>) {
                        n.reader_globals = Some(header.clone());
                        if let Some(children) = n.children.as_mut() {
                            for c in children.iter_mut() {
                                reader_context(c, header);
                            }
                        }
                    }
                    reader_context(c, &header);
                }
                Some(Expansion {
                    body: p.ast,
                    syntax_clean,
                })
            }
        }
    }
    pub(crate) fn expand_tree(&mut self, nodes: &[Node], ctx: &Context) -> Vec<Node> {
        let mut out = Vec::new();
        for node in nodes {
            if node.key == "inline_script" {
                if let Some(body) = self.expand_inline(node, ctx) {
                    out.extend(body);
                }
            } else {
                if node.macro_parts.is_none() {
                    let mut expanded = node.clone_header();
                    expanded.children = node
                        .children
                        .as_ref()
                        .map(|children| self.expand_tree(children, ctx).into_boxed_slice());
                    out.push(expanded);
                } else {
                    out.push(node.clone());
                }
            }
        }
        out
    }
    pub(crate) fn prepare_templates(&mut self) {
        let files = std::mem::take(&mut self.program.files);
        let mut expanded = Vec::new();
        for mut file in files {
            self.chain.push(serde_json::json!({"kind":"template-preparation","caller":file.path,"category":file.category,"target":file.target}).into());
            let mut roots = Vec::new();
            for n in &file.ast {
                if n.key == "inline_script" {
                    let deep = file.target || file.category != "events";
                    if let Some(body) = self.expand_inline_mode(n, &Context::new("none"), deep) {
                        roots.extend(body.into_iter().map(std::sync::Arc::new));
                    }
                } else if file.target || file.category == "component_templates" {
                    roots.extend(
                        self.expand_tree(std::slice::from_ref(n.as_ref()), &Context::new("none"))
                            .into_iter()
                            .map(std::sync::Arc::new),
                    );
                } else {
                    roots.push(n.clone());
                }
            }
            file.ast = roots;
            self.chain.pop();
            expanded.push(file);
        }
        self.program.scripts.clear();
        self.program.events.clear();
        self.program.definitions.clear();
        // Color swatches are indexed from flags/colors.txt, outside the
        // declaration files rebuilt below. Do not discard that effective index.
        let flag_colors = self.program.entities.remove("flag_colors");
        self.program.entities.clear();
        if let Some(colors) = flag_colors {
            self.program.entities.insert("flag_colors".into(), colors);
        }
        self.program.target_files = 0;
        for file in expanded {
            self.program.index(file);
        }
        if let Some(categories) = self.program.definitions.get("economic_categories") {
            for (name, node) in categories {
                for suffix in ["add", "mult"] {
                    if let Some(kinds) = node.field(&format!("generate_{suffix}_modifiers")) {
                        for kind in kinds
                            .body()
                            .iter()
                            .map(Node::scalar)
                            .filter(|s| matches!(*s, "cost" | "upkeep" | "produces"))
                        {
                            self.rules
                                .modifiers
                                .insert(format!("{name}_{kind}_{suffix}"));
                            for resource in self
                                .program
                                .entities
                                .get("strategic_resources")
                                .into_iter()
                                .flatten()
                            {
                                self.rules
                                    .modifiers
                                    .insert(format!("{name}_{resource}_{kind}_{suffix}"));
                            }
                        }
                    }
                }
            }
        }
        for job in self.program.entities.get("pop_jobs").into_iter().flatten() {
            for suffix in [
                "add",
                "per_pop",
                "per_crime",
                "per_pop_group",
                "per_planet_size",
                "per_district",
                "per_pop_infrastructure",
                "per_free_job",
            ] {
                self.rules.modifiers.insert(format!("job_{job}_{suffix}"));
            }
            for resource in self
                .program
                .entities
                .get("strategic_resources")
                .into_iter()
                .flatten()
            {
                for kind in ["produces", "upkeep"] {
                    for suffix in ["add", "mult"] {
                        self.rules
                            .modifiers
                            .insert(format!("job_{job}_{resource}_{kind}_{suffix}"));
                    }
                }
            }
        }
    }
    fn script_macro_body(
        &mut self,
        n: &Node,
        ctx: &Context,
        def: &Node,
    ) -> Option<ScriptExpansion> {
        let parts = def.macro_parts.as_ref()?;
        let mut args = BTreeMap::new();
        for c in n.body() {
            if c.children.is_some() {
                self.gap(
                    c,
                    "block-valued scripted macro argument",
                    "rule-unimplemented",
                    Some(ctx),
                );
                return None;
            }
            args.insert(c.key.clone(), c.scalar().into());
        }
        let source = parts
            .iter()
            .map(|p| {
                if p.parameter {
                    format!("${}$", p.text)
                } else {
                    p.text.clone()
                }
            })
            .collect::<String>();
        let text = match render(&source, &args) {
            Ok(s) => s,
            Err(e) => {
                self.error(n, "macro-argument", e, Some(ctx));
                return None;
            }
        };
        let vars = def
            .macro_variables
            .iter()
            .map(|(k, v)| format!("@{k}={v}\n"))
            .collect::<String>();
        let text = format!("{vars}expanded={{\n{text}\n}}");
        let header: std::sync::Arc<str> = self.program.reader_header(&def.source).into();
        // A definition ID alone is not an expansion identity. The real parsed
        // text includes macro reader variables, and the complete reader header
        // is part of the key. Only syntax, never caller/State facts, is cached.
        let cache_key = (
            def.id,
            def.source.to_string(),
            def.line,
            text.clone(),
            header.to_string(),
        );
        if let Some(body) = self.script_bodies.get(&cache_key).cloned() {
            self.metrics.script_body_cache_hits += 1;
            return Some(ScriptExpansion {
                body,
                syntax_clean: true,
                wrapper_intact: true,
            });
        }
        self.metrics.script_body_compilations += 1;
        let parsed = self.parse_expanded_with_header(&def.source, &text, n, ctx, header)?;
        let wrapper_intact = parsed.body.len() == 1
            && parsed.body[0].key == "expanded"
            && parsed.body[0].children.is_some()
            && parsed.body[0].value.is_none();
        let syntax_clean = parsed.syntax_clean && wrapper_intact;
        if !wrapper_intact {
            self.gap(
                n,
                "scripted macro replacement escaped its synthetic body wrapper; result composition is unverified",
                "script-expansion-boundary-unverified",
                Some(ctx),
            );
        }
        let mut body = Vec::new();
        for (index, mut root) in parsed.body.into_iter().enumerate() {
            if index == 0 && root.key == "expanded" && root.children.is_some() {
                body.extend(root.children.take().unwrap().into_vec());
            } else {
                // Preserve every unexpected root for ordinary static/semantic
                // diagnostics; it is never a clean cached boolean guard.
                body.push(root);
            }
        }
        fn relocate(nodes: &mut [Node], shift: i64) {
            for node in nodes {
                node.line = (node.line as i64 + shift).max(1) as u32;
                if let Some(children) = node.children.as_mut() {
                    relocate(children, shift);
                }
            }
        }
        relocate(
            &mut body,
            def.line as i64 - (vars.lines().count() as i64) - 2,
        );
        let body = std::sync::Arc::new(body);
        if syntax_clean && self.script_bodies.len() < 512 {
            self.script_bodies.insert(cache_key, body.clone());
        }
        Some(ScriptExpansion {
            body,
            syntax_clean,
            wrapper_intact,
        })
    }

    fn guard_boolean_body(
        &mut self,
        nodes: &[Node],
        ctx: &Context,
        visiting: &mut BTreeSet<String>,
        remaining: &mut usize,
    ) -> bool {
        for n in nodes {
            if n.is_memory_directive() || n.key.starts_with('@') {
                continue;
            }
            if *remaining == 0 {
                return false;
            }
            *remaining -= 1;
            let key = n.key.to_ascii_lowercase();
            if matches!(key.as_str(), "if" | "else_if" | "else" | "switch") {
                // These tokens are present in the exported trigger table, but
                // their statement-result composition is not a flat boolean
                // conjunction. The guard transfer does not model it yet.
                return false;
            }
            if self.is_scope(&n.key)
                || matches!(
                    key.as_str(),
                    "and" | "or" | "not" | "nor" | "nand" | "hidden_trigger"
                )
            {
                if n.children.is_none()
                    || !self.guard_boolean_body(n.body(), ctx, visiting, remaining)
                {
                    return false;
                }
            } else if self.rules.triggers.contains_key(&n.key) {
                // A count iterator with no comparison threshold is a query,
                // not a boolean guard. Do not lend its inner type predicates
                // to the outer caller.
                if n.key.starts_with("count_")
                    && n.children.is_some()
                    && n.field("count").is_none()
                    && self.rules.triggers[&n.key].iterator_output.is_some()
                {
                    return false;
                }
            } else if let Some(def) = self.program.scripts.get(&(false, n.key.clone())).cloned() {
                if !visiting.insert(n.key.clone()) {
                    return false;
                }
                let boolean = if def.macro_parts.is_some() {
                    self.script_macro_body(n, ctx, &def)
                        .is_some_and(|expanded| {
                            expanded.syntax_clean
                                && self.guard_boolean_body(&expanded.body, ctx, visiting, remaining)
                        })
                } else {
                    matches!(n.value.as_deref(), Some("yes" | "no"))
                        && self.guard_boolean_body(def.body(), ctx, visiting, remaining)
                };
                visiting.remove(&n.key);
                if !boolean {
                    return false;
                }
            } else {
                // Unknown tokens, inline forms and statement-control result
                // contracts remain unproved. Ordinary call traversal retains
                // their own diagnostics and does not share this classification.
                return false;
            }
        }
        true
    }

    pub(crate) fn macro_guard_body(
        &mut self,
        n: &Node,
        ctx: &Context,
        def: &Node,
    ) -> Option<std::sync::Arc<Vec<Node>>> {
        // A parameter block invokes the rendered boolean body. Scalar `no`
        // with a macro is not a documented parameter/comparison form; do not
        // invent its polarity from the ordinary yes/no scripted-call contract.
        if n.op.as_str() != "=" || (n.children.is_none() && n.value.as_deref() != Some("yes")) {
            self.gap(
                n,
                "scripted macro guard comparison form requires verification",
                "script-guard-contract-unverified",
                Some(ctx),
            );
            return None;
        }
        let mut seen = BTreeSet::new();
        if n.body().iter().any(|arg| {
            arg.key.is_empty()
                || arg.value.is_none()
                || arg.children.is_some()
                || arg.op.as_str() != "="
                || !seen.insert(&arg.key)
        }) {
            self.gap(
                n,
                "scripted macro guard argument composition is unverified",
                "script-guard-contract-unverified",
                Some(ctx),
            );
            return None;
        }
        let expanded = self.script_macro_body(n, ctx, def)?;
        if !expanded.syntax_clean {
            self.gap(
                n,
                "scripted macro guard has incomplete syntax; no condition facts are assumed",
                "script-guard-contract-unverified",
                Some(ctx),
            );
            return None;
        }
        if !self.guard_boolean_body(
            &expanded.body,
            ctx,
            &mut BTreeSet::from([n.key.clone()]),
            &mut 128,
        ) {
            self.gap(
                n,
                "scripted macro guard does not have an established boolean result",
                "script-guard-return-unverified",
                Some(ctx),
            );
            return None;
        }
        Some(expanded.body)
    }

    pub(crate) fn call(&mut self, n: &Node, effect: bool, ctx: &Context, state: &mut State) {
        self.invoked.insert((effect, n.key.clone()));
        let key = format!("{}:{}", if effect { "effect" } else { "trigger" }, n.key);
        if self.active.contains(&key) || self.active.len() >= 40 {
            self.gap(
                n,
                format!(
                    "recursive scripted call requires SCC transfer (not yet migrated): {}",
                    n.key
                ),
                "analysis-limit",
                Some(ctx),
            );
            self.complete = false;
            self.migration_gaps
                .insert("recursive_call_summaries".into());
            return;
        }
        let Some(def) = self.program.scripts.get(&(effect, n.key.clone())).cloned() else {
            return;
        };
        self.metrics.summary_evaluations += 1;
        if def.macro_parts.is_some() {
            if let Some(expanded) = self.script_macro_body(n, ctx, &def) {
                if !expanded.wrapper_intact {
                    self.chain.push(serde_json::json!({"kind":"scripted-call","caller":n.source,"line":n.line,"definition":def.source,"name":n.key,"context":ctx,"composition_unverified":true}).into());
                    self.structure_script(&expanded.body, effect);
                    self.chain.pop();
                    if effect {
                        // The escaped fragment has no established composition.
                        // Neither old facts nor apparent saves/assignments in
                        // its roots may become certain postconditions.
                        for binding in state.bindings.values_mut() {
                            binding.types = Types::new("?");
                            binding.availability = crate::domain::Availability::Maybe;
                        }
                        state.guarded.clear();
                        for fact in state.numbers.values_mut() {
                            *fact = crate::domain::NumberFact::default();
                        }
                        state.guarded_numbers.clear();
                        fn unknown_saves(nodes: &[Node], state: &mut State) {
                            for node in nodes {
                                let prefix = match node.key.as_str() {
                                    "save_event_target_as" => Some("event_target:"),
                                    "save_global_event_target_as" => Some("global_event_target:"),
                                    _ => None,
                                };
                                if let Some(prefix) = prefix
                                    && node.children.is_none()
                                    && !node.scalar().is_empty()
                                    && node
                                        .scalar()
                                        .chars()
                                        .all(|c| c.is_alphanumeric() || c == '_')
                                {
                                    state.bindings.insert(
                                        format!("{prefix}{}", node.scalar()),
                                        crate::domain::Binding {
                                            types: Types::new("?"),
                                            availability: crate::domain::Availability::Maybe,
                                        },
                                    );
                                }
                                unknown_saves(node.body(), state);
                            }
                        }
                        unknown_saves(&expanded.body, state);
                    }
                    return;
                }
                // Dirty bodies retain ordinary diagnostics/traversal. Only
                // guard transfer refuses to treat them as boolean evidence.
                self.evaluate_script(n, effect, ctx, state, &def, &expanded.body);
            }
        } else {
            let result = if effect {
                ScriptReturn::Boolean
            } else {
                self.script_return(&def, &mut BTreeSet::from([n.key.clone()]))
            };
            if n.children.is_some() {
                self.error(
                    n,
                    "structure",
                    format!("scripted call {} requires a scalar comparison", n.key),
                    Some(ctx),
                );
            } else if !matches!(n.scalar(), "yes" | "no") {
                match result {
                    ScriptReturn::Count => {
                        self.numeric_in_state(n, ctx, state);
                    }
                    ScriptReturn::Unknown => self.gap(
                        n,
                        format!("script return type unresolved: {}", n.key),
                        "numeric-information-unresolved",
                        Some(ctx),
                    ),
                    ScriptReturn::Boolean => self.error(
                        n,
                        "value",
                        format!(
                            "scripted call {} expects yes/no, received `{}`",
                            n.key,
                            n.scalar()
                        ),
                        Some(ctx),
                    ),
                }
            }
            self.evaluate_script(n, effect, ctx, state, &def, def.body());
        }
    }
    fn evaluate_script(
        &mut self,
        n: &Node,
        effect: bool,
        ctx: &Context,
        state: &mut State,
        def: &Node,
        body: &[Node],
    ) {
        let key = format!("{}:{}", if effect { "effect" } else { "trigger" }, n.key);
        self.active.push(key);
        self.chain.push(serde_json::json!({"kind":"scripted-call","caller":n.source,"line":n.line,"definition":def.source,"name":n.key,"context":ctx}).into());
        if effect {
            self.effects(body, ctx, state);
        } else {
            self.triggers(body, ctx, state);
        }
        self.chain.pop();
        self.active.pop();
    }
    pub(crate) fn weighted(&mut self, n: &Node, ctx: &Context, state: &State) {
        if !self.block(n) {
            return;
        }
        for c in n.body() {
            match c.key.as_str() {
                "factor" | "weight" | "base" | "add" | "mult" | "subtract" | "multiply"
                | "divide" | "min" | "max" | "set" | "days" | "months" | "years" => {
                    self.numeric_in_state(c, ctx, state);
                }
                "modifier" => self.weighted(c, ctx, state),
                "desc" | "text" => {}
                "limit" => self.triggers(c.body(), ctx, state),
                "inline_script" => {
                    if let Some(body) = self.expand_inline(c, ctx) {
                        let wrapper = Node {
                            children: Some(body.into_boxed_slice()),
                            ..c.clone()
                        };
                        self.weighted(&wrapper, ctx, state);
                    }
                }
                _ => self.triggers(std::slice::from_ref(c), ctx, state),
            }
        }
    }
    pub(crate) fn parameters(
        &mut self,
        n: &Node,
        effect: bool,
        ctx: &Context,
        state: &mut State,
        rule: &Rule,
    ) {
        let numeric_inputs = state.clone();
        if effect && !crate::variables::preserves_numbers(&n.key) {
            state.forget_numbers();
        } else if effect
            && matches!(
                n.key.as_str(),
                "create_country"
                    | "create_species"
                    | "create_fleet"
                    | "create_random_fleet"
                    | "create_ship"
                    | "create_leader"
                    | "create_army"
                    | "spawn_planet"
                    | "create_megastructure"
                    | "spawn_megastructure"
                    | "spawn_system"
                    | "create_ambient_object"
                    | "start_situation"
            )
        {
            state.retain_number_anchors();
        }
        if effect && n.key == "create_random_fleet" {
            self.random_fleet_parameters(n, ctx, state, &numeric_inputs);
            return;
        }
        if effect && n.key == "spawn_planet" {
            self.spawn_planet_parameters(n, ctx, state, &numeric_inputs);
            return;
        }
        if effect && n.key == "add_timeline_event" {
            self.timeline_parameters(n, ctx, &numeric_inputs);
            return;
        }
        if effect && n.key == "fire_on_action" {
            self.fire_action_parameters(n, ctx, state, &numeric_inputs);
            return;
        }
        if effect && self.agreement_debris_parameters(n, ctx, &numeric_inputs) {
            return;
        }
        if effect && self.effect_map_parameters(n) {
            return;
        }
        if self.explicit_parameters(n, effect, ctx, state) {
            return;
        }
        if n.key == "queue_actions" {
            self.fleet_actions(n, ctx, state, &mut BTreeSet::new());
            return;
        }
        if event_scope(&n.key).is_some() {
            let Some(id) = n.val("id") else {
                self.error(
                    n,
                    "missing-field",
                    format!("{} requires id", n.key),
                    Some(ctx),
                );
                return;
            };
            if let Some((scope, _)) = self.program.events.get(id).cloned() {
                if n.key != "observer_event"
                    && !Types::new(event_scope(&n.key).unwrap())
                        .known
                        .iter()
                        .any(|t| Types::new(&scope).known.contains(t))
                {
                    self.error(
                        n,
                        "event-type",
                        format!("event type mismatch: {id}"),
                        Some(ctx),
                    );
                } else {
                    let mut entry = Context::new(&scope);
                    entry.root = if scope == "none" {
                        Types::new("?")
                    } else {
                        Types::new(&scope)
                    };
                    entry.froms = std::iter::once(ctx.root.clone())
                        .chain(ctx.froms.iter().cloned())
                        .take(6)
                        .collect();
                    entry.from_objects = std::iter::once(ctx.root_object.clone())
                        .chain(ctx.from_objects.iter().cloned())
                        .take(6)
                        .collect();
                    if let Some(overrides) = n.field("scopes") {
                        if !self.block(overrides) {
                            return;
                        }
                        for c in overrides.body() {
                            if !self.scalar_field(c) {
                                continue;
                            }
                            let target = self.resolve(c.scalar(), ctx, state, c, false);
                            if c.key == "root" {
                                entry.root = target.this;
                                entry.root_object = target.object;
                            } else if let Some(depth) = super::engine::repeated(&c.key, "from") {
                                if depth <= 6 {
                                    // Replacing FROM must retain inherited
                                    // FROMFROM and deeper frames.
                                    if entry.froms.len() < depth {
                                        entry.froms.resize(depth, Types::new("?"));
                                    }
                                    entry.froms[depth - 1] = target.this;
                                    if entry.from_objects.len() < depth {
                                        entry.from_objects.resize(depth, String::new());
                                    }
                                    entry.from_objects[depth - 1] = target.object;
                                }
                            } else {
                                self.error(
                                    c,
                                    "unknown-field",
                                    "unknown event scope override",
                                    Some(ctx),
                                );
                            }
                        }
                    }
                    self.schedule(id, entry, state.clone());
                }
            } else {
                self.error(n, "missing-event", format!("unknown event {id}"), Some(ctx));
            }
            for c in n.body() {
                match c.key.as_str() {
                    "id" | "scopes" => {}
                    "days" | "random" | "random_days" | "months" | "years" | "hours" => {
                        self.numeric_in_state(c, ctx, &numeric_inputs);
                    }
                    _ => self.error(
                        c,
                        "unknown-field",
                        format!("{}: unknown parameter {}", n.key, c.key),
                        Some(ctx),
                    ),
                }
            }
            return;
        }
        let target_country = ["who", "owner", "country", "surveyor"];
        let known = match n.key.as_str() {
            "set_variable" | "change_variable" | "multiply_variable" | "subtract_variable"
            | "divide_variable" | "check_variable" => Some((vec!["which", "value"], vec![])),
            "relative_power" => Some((vec!["who", "value"], vec!["category"])),
            "end_fleet_contract" => Some((vec![], vec!["initiator", "reason"])),
            "add_modifier" => Some((
                vec!["modifier"],
                vec![
                    "days",
                    "months",
                    "years",
                    "mult",
                    "multiplier",
                    "time_multiplier",
                    "clear_on_owner_change",
                ],
            )),
            "create_fleet" => Some((
                vec![],
                vec!["name", "effect", "settings", "parent", "set_take_point"],
            )),
            "create_ship" => Some((
                vec![],
                vec![
                    "name",
                    "design",
                    "prefix",
                    "suffix",
                    "random_existing_design",
                    "graphical_culture",
                    "graphical_culture_fallback",
                    "effect",
                    "upgradable",
                    "colonizer_species",
                    "age",
                    "rarity",
                    "growth_stage",
                    "create_colony",
                ],
            )),
            "create_megastructure" => Some((
                vec!["type"],
                vec![
                    "location",
                    "owner",
                    "planet",
                    "build_percentage",
                    "graphical_culture",
                    "effect",
                ],
            )),
            "has_resource" => Some((vec!["type", "amount"], vec![])),
            _ => None,
        };
        if let Some((required, _)) = &known {
            for k in required {
                if n.field(k).is_none() {
                    self.error(
                        n,
                        "missing-field",
                        format!("{} requires {k}", n.key),
                        Some(ctx),
                    );
                }
            }
        }
        for c in n.body() {
            if effect && self.creation_callback(n, c, ctx, state) {
                continue;
            }
            if effect && self.creation_parent_field(n, c, ctx, state, &numeric_inputs) {
                continue;
            }
            if effect && self.effect_map_parameter_child(n, c, ctx, &numeric_inputs) {
                continue;
            }
            if self.explicit_parameter_child(n, c, ctx, state) {
                continue;
            }
            let allowed = known.as_ref().map_or_else(
                || rule.parameters.contains(&c.key),
                |(a, b)| a.contains(&c.key.as_str()) || b.contains(&c.key.as_str()),
            );
            if !allowed {
                if known.is_some() {
                    self.error(
                        c,
                        "unknown-field",
                        format!("{}: unknown parameter {}", n.key, c.key),
                        Some(ctx),
                    );
                } else {
                    self.gap(
                        c,
                        format!("undocumented parameter: {}/{}", n.key, c.key),
                        "rule-unimplemented",
                        Some(ctx),
                    );
                }
                continue;
            }
            match c.key.as_str() {
                "limit" | "trigger" | "potential" => self.triggers(c.body(), ctx, state),
                "effect" | "init_effect" | "init_effects" => {
                    let output = match n.key.as_str() {
                        "create_country" => "country",
                        "create_species" | "modify_species" => "species",
                        "create_fleet" => "fleet",
                        "create_ship" => "ship",
                        "create_leader" => "leader",
                        "create_army" => "army",
                        "spawn_planet" => "planet",
                        "create_megastructure" | "spawn_megastructure" => "megastructure",
                        "spawn_system" => "galactic_object",
                        "create_ambient_object" => "ambient_object",
                        "create_starbase" => "starbase",
                        "start_situation" => "situation",
                        _ => "?",
                    };
                    if output == "?" {
                        self.gap(
                            c,
                            format!("callback output not yet migrated: {}/{}", n.key, c.key),
                            "engine-contract-unverified",
                            Some(ctx),
                        );
                    }
                    let frame = ctx.push(Types::new(output), &format!("created:{}", n.id));
                    let before_numbers = state.number_snapshot();
                    state.forget_number_object(&frame.object);
                    self.effects(c.body(), &frame, state);
                    let conditional = before_numbers.join(&state.number_snapshot());
                    state.numbers = conditional.numbers;
                    state.guarded_numbers = conditional.guarded_numbers;
                }
                "modifier" => self.reference(
                    if matches!(
                        n.key.as_str(),
                        "add_opinion_modifier" | "remove_opinion_modifier" | "has_opinion_modifier"
                    ) {
                        "opinion_modifiers"
                    } else {
                        "static_modifiers"
                    },
                    c.scalar(),
                    c,
                ),
                "initiator" if n.key == "end_fleet_contract" => {
                    if !self.scalar_field(c) {
                        continue;
                    }
                    self.resolve(c.scalar(), ctx, state, c, true);
                }
                "reason" if n.key == "end_fleet_contract" => {
                    self.scalar_field(c);
                }
                "type" if n.key == "create_megastructure" => {
                    self.reference("megastructures", c.scalar(), c)
                }
                "resource" | "type" if n.key == "has_resource" => {
                    self.reference("strategic_resources", c.scalar(), c)
                }
                "amount" | "days" | "months" | "years" | "mult" | "multiplier" | "progress"
                | "age" | "growth_stage" | "duration" | "build_percentage" => {
                    self.numeric_in_state(c, ctx, &numeric_inputs);
                }
                "value"
                    if n.key == "relative_power"
                        && !matches!(
                            c.scalar(),
                            "pathetic" | "inferior" | "equivalent" | "superior" | "overwhelming"
                        ) =>
                {
                    self.error(c, "value", "invalid relative_power threshold", Some(ctx));
                }
                // A generic parameter named value is not itself a documented
                // numeric contract. Typed variable operations are handled above.
                "value" => self.numeric(c, Some(ctx)),
                "category"
                    if n.key == "relative_power"
                        && !matches!(c.scalar(), "fleet" | "economy" | "technology" | "all") =>
                {
                    self.error(c, "value", "invalid relative_power category", Some(ctx));
                }
                _ if target_country.contains(&c.key.as_str()) => {
                    let t = self.resolve(c.scalar(), ctx, state, c, false);
                    let accepted = if n.key == "relative_power" && c.key == "who" {
                        vec!["country".into(), "federation".into()]
                    } else {
                        vec!["country".into()]
                    };
                    self.compatible(&t.this, &accepted, c, &format!("{} {}", n.key, c.key), ctx);
                }
                _ if c.children.is_some() => self.gap(
                    c,
                    format!("parameter subgrammar not yet migrated: {}/{}", n.key, c.key),
                    "rule-unimplemented",
                    Some(ctx),
                ),
                _ => {}
            }
        }
        let _ = effect;
    }
    pub(crate) fn prepare_reads(&mut self) {
        fn inspect(
            n: &Node,
            reads: &mut BTreeSet<String>,
            edges: &mut BTreeSet<String>,
            program: &crate::loader::Program,
        ) {
            for value in [n.key.as_str(), n.value.as_deref().unwrap_or("")] {
                if let Some((_, expression)) = value.split_once("value:") {
                    let name = expression
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect::<String>();
                    if name.is_empty() {
                        reads.insert("*".into());
                    } else {
                        edges.insert(format!("value:{name}"));
                    }
                }
                for prefix in ["event_target:", "global_event_target:"] {
                    let mut rest = value;
                    while let Some(i) = rest.find(prefix) {
                        rest = &rest[i + prefix.len()..];
                        let name = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect::<String>();
                        if !name.is_empty() {
                            reads.insert(format!("{prefix}{name}"));
                        }
                        if rest.contains('$') || rest.contains('@') {
                            reads.insert("*".into());
                        }
                    }
                }
            }
            if n.macro_parts.is_some() || n.key == "inline_script" {
                reads.insert("*".into());
            }
            for effect in [false, true] {
                if program.scripts.contains_key(&(effect, n.key.clone())) {
                    edges.insert(format!("{effect}:{}", n.key));
                }
            }
            if n.key == "arrival_effect" && n.children.is_none() {
                let name = n.scalar();
                if name.contains(['$', '@', '[', ']']) {
                    reads.insert("*".into());
                } else {
                    edges.insert(format!("true:{name}"));
                }
            }
            if n.key == "fire_on_action"
                && let Some(name) = n.val("on_action")
            {
                if name.contains(['$', '@', ':', '[', ']', '|']) {
                    reads.insert("*".into());
                } else {
                    edges.insert(format!("on-action:{name}"));
                }
            }
            if event_scope(&n.key).is_some()
                && let Some(id) = n.val("id")
            {
                edges.insert(format!("event:{id}"));
            }
            for c in n.body() {
                inspect(c, reads, edges, program);
            }
        }
        let mut graph = BTreeMap::new();
        for file in self
            .program
            .files
            .iter()
            .filter(|f| f.category == "on_actions")
        {
            for node in &file.ast {
                let key = format!("on-action:{}", node.key);
                let (mut reads, mut edges) = (BTreeSet::new(), BTreeSet::new());
                inspect(node, &mut reads, &mut edges, &self.program);
                for list in node
                    .body()
                    .iter()
                    .filter(|n| matches!(n.key.as_str(), "events" | "random_events"))
                {
                    for event in list
                        .body()
                        .iter()
                        .filter(|e| e.children.is_none() && e.scalar() != "0")
                    {
                        if event.key == "inline_script"
                            || event.scalar().contains(['$', '@', '[', ']'])
                        {
                            reads.insert("*".into());
                        } else {
                            edges.insert(format!("event:{}", event.scalar()));
                        }
                    }
                }
                self.reads.entry(key.clone()).or_default().extend(reads);
                graph.entry(key).or_insert_with(BTreeSet::new).extend(edges);
            }
        }
        if let Some(values) = self.program.definitions.get("script_values") {
            for (name, node) in values {
                let key = format!("value:{name}");
                let (mut reads, mut edges) = (BTreeSet::new(), BTreeSet::new());
                inspect(node, &mut reads, &mut edges, &self.program);
                self.reads.insert(key.clone(), reads);
                graph.insert(key, edges);
            }
        }
        for ((kind, name), node) in &self.program.scripts {
            let key = format!("{kind}:{name}");
            let (mut reads, mut edges) = (BTreeSet::new(), BTreeSet::new());
            inspect(node, &mut reads, &mut edges, &self.program);
            self.reads.insert(key.clone(), reads);
            graph.insert(key, edges);
        }
        for (name, (_, node)) in &self.program.events {
            let key = format!("event:{name}");
            let (mut reads, mut edges) = (BTreeSet::new(), BTreeSet::new());
            inspect(node, &mut reads, &mut edges, &self.program);
            self.reads.insert(key.clone(), reads);
            graph.insert(key, edges);
        }
        // Reverse dependency worklist, not repeated full-AST rescans.
        let mut reverse: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (a, edges) in &graph {
            for b in edges {
                reverse.entry(b.clone()).or_default().insert(a.clone());
            }
        }
        let mut queue = self
            .reads
            .keys()
            .cloned()
            .collect::<std::collections::VecDeque<_>>();
        let mut pending = self.reads.keys().cloned().collect::<BTreeSet<_>>();
        while let Some(key) = queue.pop_front() {
            pending.remove(&key);
            let mut next = self.reads[&key].clone();
            for callee in &graph[&key] {
                if let Some(r) = self.reads.get(callee) {
                    next.extend(r.iter().cloned());
                } else {
                    next.insert("*".into());
                }
            }
            if next != self.reads[&key] {
                self.reads.insert(key.clone(), next);
                for caller in reverse.get(&key).into_iter().flatten() {
                    if pending.insert(caller.clone()) {
                        queue.push_back(caller.clone());
                    }
                }
            }
        }
    }
}
