//! Demand-driven, structural writer TYPE discovery over effective loaded ASTs.
//! This index never executes scripts, establishes a caller, or writes a State.
//! A writer witness is not evidence that a target exists at the consumer.
use crate::{
    ast::Node,
    domain::{Context, Types, alias},
    engine::Engine,
    loader::{Program, event_scope},
    parser::ParserClient,
    rules::Rules,
    templates::render,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct SavedTargetTypes {
    types: BTreeMap<String, Types>,
    evidence: Vec<Value>,
}
struct Writer {
    key: String,
    types: Types,
    evidence: Value,
}
// Per-discovery, syntax-only storage. A full cache falls back to parsing; it
// never changes the writer walk, open set, provenance, or availability domain.
pub(crate) const EXPANDED_AST_CACHE_LIMIT: usize = 512;
const EXPANDED_AST_CACHE_NODES: usize = 100_000;
const EXPANDED_AST_CACHE_KEY_BYTES: usize = 8 * 1024 * 1024;
const READER_HEADER_CACHE_LIMIT: usize = 256;
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ExpansionKey {
    path: String,
    rendered: String,
    header: String,
}
struct Scan<'a> {
    program: &'a Program,
    rules: &'a Rules,
    parser: &'a mut ParserClient,
    wanted: BTreeSet<String>,
    writers: Vec<Writer>,
    open: BTreeSet<String>,
    active: BTreeSet<String>,
    called: BTreeSet<String>,
    writer_scripts: BTreeSet<String>,
    incoming: BTreeMap<String, BTreeSet<String>>,
    provenance: Vec<Value>,
    seen_calls: BTreeSet<String>,
    expanded_ast: BTreeMap<ExpansionKey, Vec<Node>>,
    expanded_ast_nodes: usize,
    expanded_ast_key_bytes: usize,
    reader_headers: BTreeMap<String, String>,
    work: usize,
    exhausted: bool,
}

fn target_save(key: &str) -> Option<&'static str> {
    match key {
        "save_event_target_as" => Some("event_target:"),
        "save_global_event_target_as" => Some("global_event_target:"),
        _ => None,
    }
}
fn fixed_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_')
}
fn source(n: &Node) -> String {
    n.macro_parts.as_ref().map_or_else(String::new, |parts| {
        parts
            .iter()
            .map(|part| {
                if part.parameter {
                    format!("${}$", part.text)
                } else {
                    part.text.clone()
                }
            })
            .collect()
    })
}
fn names_in(value: &str, wanted: &mut BTreeSet<String>) {
    for prefix in ["event_target:", "global_event_target:"] {
        let mut tail = value;
        while let Some(index) = tail.find(prefix) {
            tail = &tail[index + prefix.len()..];
            let name: String = tail
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                wanted.insert(name);
            }
        }
    }
}
fn inspect_demands(n: &Node, wanted: &mut BTreeSet<String>) {
    names_in(&n.key, wanted);
    names_in(n.scalar(), wanted);
    names_in(&source(n), wanted);
    for child in n.body() {
        inspect_demands(child, wanted);
    }
}
fn demanded_reads(
    n: &Node,
    reads: &BTreeMap<String, BTreeSet<String>>,
    wanted: &mut BTreeSet<String>,
) {
    let mut keys = vec![
        format!("true:{}", n.key),
        format!("false:{}", n.key),
        format!("on-action:{}", n.key),
        format!("value:{}", n.key),
    ];
    if let Some(id) = n.val("id") {
        keys.push(format!("event:{id}"));
    }
    if let Some(action) = n.val("on_action") {
        keys.push(format!("on-action:{action}"));
    }
    for value in [n.key.as_str(), n.scalar()] {
        if let Some((_, value)) = value.split_once("value:") {
            let name: String = value
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            keys.push(format!("value:{name}"));
        }
    }
    for key in keys {
        if let Some(values) = reads.get(&key) {
            for value in values {
                names_in(value, wanted);
            }
        }
    }
    for child in n.body() {
        demanded_reads(child, reads, wanted);
    }
}
fn assignments(text: &str) -> BTreeSet<String> {
    // Over-approximating call edges is safe. Comments/strings can add an edge,
    // never remove a real literal caller and falsely close a writer set.
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|name| !name.is_empty())
        .map(String::from)
        .collect()
}
fn calls(n: &Node, names: &BTreeSet<String>, out: &mut BTreeSet<String>) {
    if names.contains(&n.key) {
        out.insert(n.key.clone());
    }
    if n.children.is_none() && names.contains(n.scalar()) {
        out.insert(n.scalar().into());
    }
    if n.macro_parts.is_some() {
        out.extend(assignments(&source(n)).intersection(names).cloned());
    }
    for child in n.body() {
        calls(child, names, out);
    }
}
fn contains_save(n: &Node) -> bool {
    target_save(&n.key).is_some()
        || source(n).contains("save_event_target_as")
        || source(n).contains("save_global_event_target_as")
        || n.body().iter().any(contains_save)
}
fn contains_inline(n: &Node) -> bool {
    n.key == "inline_script"
        || source(n).contains("inline_script")
        || n.body().iter().any(contains_inline)
}
fn dynamic_dispatch(text: &str) -> bool {
    let mut tail = text;
    while let Some(start) = tail.find('$') {
        tail = &tail[start + 1..];
        let Some(end) = tail.find('$') else {
            break;
        };
        if tail[end + 1..].trim_start().starts_with('=') {
            return true;
        }
        tail = &tail[end + 1..];
    }
    false
}

impl<'a> Scan<'a> {
    fn reader_header(&mut self, path: &str) -> String {
        if let Some(header) = self.reader_headers.get(path) {
            return header.clone();
        }
        let header = self.program.reader_header(path);
        if self.reader_headers.len() < READER_HEADER_CACHE_LIMIT {
            self.reader_headers.insert(path.into(), header.clone());
        }
        header
    }
    fn clean_expansion(&mut self, path: &str, text: &str, header: &str) -> Option<Vec<Node>> {
        let key = ExpansionKey {
            path: path.into(),
            rendered: text.into(),
            header: header.into(),
        };
        if let Some(nodes) = self.expanded_ast.get(&key) {
            // Cache payloads have never been located or visited. Callers locate
            // their own clone and still walk it in every distinct context.
            return Some(nodes.clone());
        }
        let parsed = self.parser.parse(path, text, header).ok()?;
        if parsed.fatal.is_some() || !parsed.diagnostics.is_empty() {
            // No negative caching: each failed invocation retains its original
            // open-source evidence and source-specific parse attempt.
            return None;
        }
        fn node_count(nodes: &[Node]) -> usize {
            nodes.iter().map(|n| 1 + node_count(n.body())).sum()
        }
        let nodes = parsed.ast;
        let count = node_count(&nodes);
        let bytes = key.path.len() + key.rendered.len() + key.header.len();
        if self.expanded_ast.len() < EXPANDED_AST_CACHE_LIMIT
            && self.expanded_ast_nodes.saturating_add(count) <= EXPANDED_AST_CACHE_NODES
            && self.expanded_ast_key_bytes.saturating_add(bytes) <= EXPANDED_AST_CACHE_KEY_BYTES
        {
            self.expanded_ast_nodes += count;
            self.expanded_ast_key_bytes += bytes;
            self.expanded_ast.insert(key, nodes.clone());
        }
        Some(nodes)
    }
    fn relevant(&self, n: &Node) -> bool {
        contains_save(n)
            || dynamic_dispatch(&source(n))
            || n.key.contains(['$', '[', ']'])
            || self.writer_scripts.contains(&n.key)
            || (n.children.is_none() && self.writer_scripts.contains(n.scalar()))
            || contains_inline(n)
            || n.body().iter().any(|child| self.relevant(child))
    }
    fn open_source(&mut self, n: &Node, reason: &str) {
        // Failed macro/inline expansion may conceal either save namespace.
        // Its output names cannot be assumed different from a demand.
        self.open.insert("event_target:".into());
        self.open.insert("global_event_target:".into());
        self.writers.push(Writer {
            key: "*".into(), types: Types::new("?"),
            evidence: json!({"kind":"open-writer-source","path":n.source,"line":n.line,"reason":reason,"availability_proven":false,"runtime_invocation_proven":false}),
        });
    }
    fn save(&mut self, n: &Node, prefix: &str, ctx: &Context) {
        let name = n.scalar();
        if !fixed_name(name) || n.children.is_some() {
            // Relationship-indexed targets (x@root) cannot overwrite a plain
            // x identity. Other unresolved names may refer to any demand.
            if !name.contains('@') {
                self.open.insert(prefix.into());
                self.writers.push(Writer { key:"*".into(), types:Types::new("?"), evidence:json!({"kind":"dynamic-writer-name","path":n.source,"line":n.line,"namespace":prefix,"name_expression":name,"availability_proven":false}) });
            }
            return;
        }
        if !self.wanted.contains(name) {
            return;
        }
        self.writers.push(Writer {
            key: format!("{prefix}{name}"), types: ctx.this.clone(),
            evidence: json!({"kind":"saved-target-writer","path":n.source,"line":n.line,"source_location_exact":self.provenance.is_empty(),"expansion_call_chain":self.provenance,"target":format!("{prefix}{name}"),"receiver_types":ctx.this,"receiver_object":ctx.object,"caller_root":ctx.root,"conditional_or_future_execution_possible":true,"availability_proven":false,"runtime_invocation_proven":false}),
        });
    }
    fn selected(&self, value: &str, ctx: &Context) -> Context {
        let mut frame = ctx.clone();
        for part in value.trim_end_matches('?').split('.') {
            // Match the main resolver's reserved selector spelling rules,
            // without changing target names or saved identity case.
            let normalized = part.to_ascii_lowercase();
            let link = normalized.as_str();
            let types = if link == "root" {
                ctx.root.clone()
            } else if link == "this" {
                frame.this.clone()
            } else if let Some(depth) = crate::engine::repeated(link, "from") {
                ctx.froms
                    .get(depth - 1)
                    .cloned()
                    .unwrap_or_else(|| Types::new("?"))
            } else if let Some(depth) = crate::engine::repeated(link, "prev") {
                frame
                    .prevs
                    .get(depth - 1)
                    .cloned()
                    .unwrap_or_else(|| Types::new("?"))
            } else if let Some(rule) = self.rules.scopes.get(link) {
                // The output contract is independent of a successful input's
                // identity. Never infer the unknown caller from its consumer.
                let sources = if frame.this.unknown {
                    rule.input.iter().cloned().collect::<BTreeSet<_>>()
                } else {
                    frame.this.known.clone()
                };
                let mut result = Types::default();
                for input in &sources {
                    result = result.join(&Types::new(
                        rule.output_by_input
                            .get(input)
                            .map(String::as_str)
                            .or(rule.output.as_deref())
                            .unwrap_or("?"),
                    ));
                }
                if sources.is_empty() {
                    result.unknown = true;
                }
                result
            } else {
                Types::new("?")
            };
            frame = frame.push(types, part);
        }
        frame
    }
    fn macro_body(&mut self, def: &Node, call: &Node) -> Option<Vec<Node>> {
        if def.macro_parts.is_none() {
            return Some(def.body().to_vec());
        }
        let args = call
            .body()
            .iter()
            .filter(|f| f.value.is_some() && f.children.is_none())
            .map(|f| (f.key.clone(), f.scalar().to_string()))
            .collect();
        let text = match render(&source(def), &args) {
            Ok(text) => text,
            Err(_) => {
                self.open_source(def, "scripted macro writer requires unresolved arguments");
                return None;
            }
        };
        let vars = def
            .macro_variables
            .iter()
            .map(|(k, v)| format!("@{k}={v}\n"))
            .collect::<String>();
        let text = format!("{vars}\n__writer_body__={{\n{text}\n}}");
        let header = self.reader_header(&def.source);
        match self.clean_expansion(&def.source, &text, &header) {
            Some(nodes) => {
                // A scalar argument can close the artificial wrapper and add
                // another syntax-clean root. Discarding that sibling would
                // omit a possible writer and falsely close the type set.
                // Recheck even a syntax-cache hit; the cache is not evidence
                // that the entire expansion stayed in the expected body.
                if nodes.len() != 1
                    || nodes[0].key != "__writer_body__"
                    || nodes[0].op.as_str() != "="
                    || nodes[0].children.is_none()
                    || nodes[0].macro_parts.is_some()
                {
                    self.open_source(
                        def,
                        "scripted macro writer expansion boundary is unverified",
                    );
                    return None;
                }
                let mut body = nodes[0].body().to_vec();
                for node in &mut body {
                    node.locate(&def.source, &mut 0);
                }
                Some(body)
            }
            _ => {
                self.open_source(def, "scripted macro writer parse is incomplete");
                None
            }
        }
    }
    fn inline_body(&mut self, n: &Node) -> Option<Vec<Node>> {
        let name = n
            .value
            .as_deref()
            .or(n.val("script"))
            .unwrap_or("")
            .replace('\\', "/")
            .to_ascii_lowercase();
        let key = format!("inline:{name}");
        if !self.active.insert(key.clone()) {
            self.open_source(n, "recursive inline writer");
            return None;
        }
        let result = if let Some((path, text)) = self.program.inline.get(&name).cloned() {
            let args = n
                .body()
                .iter()
                .filter(|f| f.key != "script" && f.value.is_some() && f.children.is_none())
                .map(|f| (f.key.clone(), f.scalar().to_string()))
                .collect();
            let header = self.reader_header(&n.source);
            match render(&text, &args)
                .ok()
                .and_then(|text| self.clean_expansion(&path, &text, &header))
            {
                Some(mut nodes) => {
                    for node in &mut nodes {
                        node.locate(&path, &mut 0);
                    }
                    Some(nodes)
                }
                _ => {
                    self.open_source(n, "inline writer expansion is incomplete");
                    None
                }
            }
        } else {
            self.open_source(n, "inline writer source is missing");
            None
        };
        self.active.remove(&key);
        result
    }
    fn walk(&mut self, nodes: &[Node], ctx: &Context) {
        for n in nodes {
            if !self.relevant(n) {
                continue;
            }
            self.work += 1;
            if self.work > 100_000 || self.active.len() > 40 {
                self.exhausted = true;
                self.open_source(n, "writer structural work/context budget reached");
                return;
            }
            if let Some(prefix) = target_save(&n.key) {
                self.save(n, prefix, ctx);
                continue;
            }
            if n.key.contains(['$', '@', '[', ']']) {
                self.open_source(n, "dynamic effect name may conceal a saved-target writer");
                continue;
            }
            // Scalar callbacks (for example arrival_effect=helper) are not
            // ordinary scripted calls. Their receiver must stay unknown unless
            // a dedicated callback contract supplies it independently.
            if n.children.is_none()
                && self.writer_scripts.contains(n.scalar())
                && !self.program.scripts.contains_key(&(true, n.key.clone()))
            {
                let call = Node {
                    key: n.scalar().into(),
                    value: Some("yes".into()),
                    ..n.clone()
                };
                self.walk(&[call], &Context::unknown_caller());
                continue;
            }
            if n.key == "inline_script" {
                let key = format!(
                    "inline:{}",
                    n.value.as_deref().or(n.val("script")).unwrap_or("")
                );
                if self.active.contains(&key) {
                    self.open_source(n, "recursive inline writer");
                    continue;
                }
                if let Some(body) = self.inline_body(n) {
                    self.active.insert(key.clone());
                    self.provenance.push(json!({"kind":"inline-writer-expansion","call_path":n.source,"call_line":n.line,"name":key}));
                    self.walk(&body, ctx);
                    self.provenance.pop();
                    self.active.remove(&key);
                }
                continue;
            }
            if let Some(def) = self.program.scripts.get(&(true, n.key.clone())).cloned() {
                self.called.insert(n.key.clone());
                let key = format!("script:{}", n.key);
                if !self.active.insert(key.clone()) {
                    self.open_source(n, "recursive scripted writer");
                    continue;
                }
                let variant = format!(
                    "{}|{}|{}|{:?}|{:?}|{}",
                    def.id,
                    ctx.this.label(),
                    ctx.root.label(),
                    ctx.froms.iter().map(Types::label).collect::<Vec<_>>(),
                    ctx.prevs.iter().map(Types::label).collect::<Vec<_>>(),
                    n.signature()
                );
                if !self.seen_calls.insert(variant) {
                    self.active.remove(&key);
                    continue;
                }
                if let Some(body) = self.macro_body(&def, n) {
                    let expanded = def.macro_parts.is_some();
                    if expanded {
                        self.provenance.push(json!({"kind":"scripted-writer-expansion","definition":def.key,"definition_path":def.source,"definition_line":def.line,"call_path":n.source,"call_line":n.line}));
                    }
                    self.walk(&body, ctx);
                    if expanded {
                        self.provenance.pop();
                    }
                }
                self.active.remove(&key);
                continue;
            }
            if n.children.is_none() {
                continue;
            }
            let constructor = match n.key.as_str() {
                "create_country" => Some("country"),
                "create_species" => Some("species"),
                "create_leader" => Some("leader"),
                "create_fleet" | "create_random_fleet" => Some("fleet"),
                "create_ship" => Some("ship"),
                "create_army" => Some("army"),
                "create_starbase" => Some("starbase"),
                "create_pop_group" => Some("pop_group"),
                "create_ambient_object" => Some("ambient_object"),
                "create_megastructure" | "spawn_megastructure" => Some("megastructure"),
                "spawn_planet" => Some("planet"),
                "start_situation" => Some("situation"),
                _ => None,
            };
            if let Some(output) = constructor {
                for child in n.body() {
                    if matches!(child.key.as_str(), "effect" | "init_effect") {
                        self.walk(
                            child.body(),
                            &ctx.push(Types::new(output), &format!("writer-constructor:{}", n.key)),
                        );
                    } else if self.relevant(child) {
                        self.walk(std::slice::from_ref(child), &Context::unknown_caller());
                    }
                }
                continue;
            }
            if let Some(rule) = self
                .rules
                .effects
                .get(&n.key)
                .or_else(|| self.rules.triggers.get(&n.key))
                && let Some(output) = &rule.iterator_output
            {
                self.walk(
                    n.body(),
                    &ctx.push(Types::new(output), &format!("writer-iterator:{}", n.key)),
                );
                continue;
            }
            let first = n
                .key
                .trim_end_matches('?')
                .split('.')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            if self.rules.scopes.contains_key(&first)
                || matches!(first.as_str(), "root" | "this")
                || crate::engine::repeated(&first, "from").is_some()
                || crate::engine::repeated(&first, "prev").is_some()
                || n.key.starts_with("event_target:")
                || n.key.starts_with("global_event_target:")
                || n.key.contains('.')
            {
                self.walk(n.body(), &self.selected(&n.key, ctx));
                continue;
            }
            if matches!(
                n.key.as_str(),
                "immediate"
                    | "after"
                    | "option"
                    | "if"
                    | "else_if"
                    | "else"
                    | "hidden_effect"
                    | "hidden_tooltip"
                    | "random"
                    | "random_list"
            ) || n.key.parse::<f64>().is_ok()
            {
                let mut frame = ctx.clone();
                if let Some(kind) = n.field("limit").and_then(|l| l.val("is_scope_type"))
                    && crate::rules::SCOPES.split_whitespace().any(|s| s == kind)
                {
                    frame.this = Types::new(kind);
                }
                self.walk(n.body(), &frame);
            } else {
                self.walk(n.body(), &Context::unknown_caller());
            }
        }
    }
}

impl Engine {
    pub(crate) fn prepare_saved_target_types(&mut self) {
        let mut wanted = BTreeSet::new();
        for file in self.program.files.iter().filter(|f| f.target) {
            for node in &file.ast {
                inspect_demands(node, &mut wanted);
                // The existing read summaries already close literal call/value
                // dependencies. Do not union every unrelated upstream reader.
                demanded_reads(node, &self.reads, &mut wanted);
            }
        }
        if wanted.is_empty() {
            self.saved_target_types = SavedTargetTypes::default();
            return;
        }
        let names = self
            .program
            .scripts
            .keys()
            .filter(|(effect, _)| *effect)
            .map(|(_, name)| name.clone())
            .collect::<BTreeSet<_>>();
        let mut writer_scripts = BTreeSet::new();
        let mut incoming: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut edges = BTreeMap::new();
        for ((effect, name), node) in &self.program.scripts {
            if !effect {
                continue;
            }
            if contains_save(node) || contains_inline(node) || dynamic_dispatch(&source(node)) {
                writer_scripts.insert(name.clone());
            }
            let mut outgoing = BTreeSet::new();
            for child in node.body() {
                calls(child, &names, &mut outgoing);
            }
            if node.macro_parts.is_some() {
                outgoing.extend(assignments(&source(node)).intersection(&names).cloned());
            }
            for callee in &outgoing {
                incoming
                    .entry(callee.clone())
                    .or_default()
                    .insert(name.clone());
            }
            edges.insert(name.clone(), outgoing);
        }
        loop {
            let previous = writer_scripts.len();
            for (name, callees) in &edges {
                if callees.iter().any(|callee| writer_scripts.contains(callee)) {
                    writer_scripts.insert(name.clone());
                }
            }
            if previous == writer_scripts.len() {
                break;
            }
        }
        let mut scan = Scan {
            program: &self.program,
            rules: &self.rules,
            parser: &mut self.parser,
            wanted,
            writers: vec![],
            open: BTreeSet::new(),
            active: BTreeSet::new(),
            called: BTreeSet::new(),
            writer_scripts,
            incoming,
            provenance: vec![],
            seen_calls: BTreeSet::new(),
            expanded_ast: BTreeMap::new(),
            expanded_ast_nodes: 0,
            expanded_ast_key_bytes: 0,
            reader_headers: BTreeMap::new(),
            work: 0,
            exhausted: false,
        };
        // Effective declaration/event providers only. Source-level overridden
        // definitions are not additional runtime writer candidates.
        let mut roots: Vec<(Arc<Node>, Context)> = Vec::new();
        for file in &self.program.files {
            if matches!(
                file.category.as_str(),
                "scripted_effects" | "scripted_triggers" | "scripted_variables"
            ) {
                continue;
            }
            for n in &file.ast {
                if file.category == "events" {
                    if let Some((scope, active)) =
                        n.val("id").and_then(|id| self.program.events.get(id))
                        && active.id == n.id
                    {
                        let mut ctx = Context::unknown_caller();
                        ctx.this = Types::new(scope);
                        roots.push((n.clone(), ctx));
                    }
                } else if file.category == "on_actions" {
                    // Same-name on_action fragments are not established to be
                    // last-wins. Retain every effective-file fragment, matching
                    // the action analyser rather than dropping possible writers.
                    roots.push((n.clone(), Context::unknown_caller()));
                } else if let Some(defs) = self.program.definitions.get(&file.category)
                    && defs.values().any(|active| active.id == n.id)
                {
                    roots.push((n.clone(), Context::unknown_caller()));
                }
            }
        }
        for (root, ctx) in roots {
            if event_scope(&root.key).is_some() {
                scan.walk(root.body(), &ctx);
            } else {
                scan.walk(root.body(), &Context::unknown_caller());
            }
        }
        let scripts = scan.writer_scripts.clone();
        // Walk caller roots before orphan/cyclic definitions, so an actual
        // constructor/selector call can provide THIS without a guessed entry.
        for orphan_pass in [false, true] {
            for name in &scripts {
                if scan.called.contains(name) {
                    continue;
                }
                let has_incoming = scan
                    .incoming
                    .get(name)
                    .is_some_and(|incoming| !incoming.is_empty());
                if has_incoming != orphan_pass {
                    continue;
                }
                let def = self.program.scripts[&(true, name.clone())].clone();
                scan.called.insert(name.clone());
                let mut call = def.clone_header();
                call.children = None;
                if let Some(body) = scan.macro_body(&def, &call) {
                    let expanded = def.macro_parts.is_some();
                    if expanded {
                        scan.provenance.push(json!({"kind":"scripted-writer-expansion","definition":def.key,"definition_path":def.source,"definition_line":def.line,"caller_proven":false}));
                    }
                    scan.walk(&body, &Context::unknown_caller());
                    if expanded {
                        scan.provenance.pop();
                    }
                }
            }
        }
        let mut result = SavedTargetTypes::default();
        for writer in scan.writers {
            if writer.key != "*" {
                let merged = result
                    .types
                    .get(&writer.key)
                    .cloned()
                    .unwrap_or_default()
                    .join(&writer.types);
                result.types.insert(writer.key, merged);
            }
            result.evidence.push(writer.evidence);
        }
        for name in &scan.wanted {
            for prefix in &scan.open {
                result
                    .types
                    .entry(format!("{prefix}{name}"))
                    .or_default()
                    .unknown = true;
            }
        }
        for (target, types) in &result.types {
            result.evidence.push(json!({"kind":"saved-target-type-candidates","target":target,"candidate_types":types,"closed_static_writer_set":!types.unknown&&!types.known.is_empty(),"availability_proven":false,"runtime_invocation_proven":false}));
        }
        if scan.exhausted {
            self.complete = false;
        }
        result.evidence.push(json!({"kind":"writer-type-discovery-summary","demanded_names":scan.wanted,"structural_nodes":scan.work,"budget_exhausted":scan.exhausted,"open_namespaces":scan.open,"availability_proven":false,"semantic_full_game_scan":false}));
        result.evidence.sort_by_key(Value::to_string);
        result.evidence.dedup();
        self.saved_target_types = result;
    }
    pub(crate) fn saved_target_type_candidates(&self, key: &str) -> Option<Types> {
        let exact = self.saved_target_types.types.get(key).cloned();
        if key.starts_with("event_target:") {
            let global = self.saved_target_types.types.get(&alias(key)).cloned();
            match (exact, global) {
                (Some(a), Some(b)) => Some(a.join(&b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            }
        } else {
            exact
        }
    }
    pub fn saved_target_type_evidence(&self) -> Vec<Value> {
        self.saved_target_types.evidence.clone()
    }
}
