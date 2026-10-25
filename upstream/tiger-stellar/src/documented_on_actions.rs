//! Installed layer-zero annotations describe engine entries. Mod comments and
//! ordinary scripted dispatch do not establish these engine input contracts.
use crate::{
    ast::Node,
    domain::{Context, State, Types},
    engine::{Engine, repeated},
    hash::sha256,
    rules::{Rules, SCOPES},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub(crate) struct DocumentedAction {
    pub context: Context,
    pub evidence: serde_json::Value,
    pub optional: BTreeSet<String>,
}

fn annotation_type(phrase: &str, rules: &Rules) -> Option<Types> {
    let lower = phrase.to_ascii_lowercase();
    // Natural-language alternatives are not a single engine scope contract.
    if lower
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|s| s == "or")
    {
        return None;
    }
    let name = lower.split([',', '.', '(']).next()?.trim();
    let aliases = [
        ("solar system", "galactic_object"),
        ("astral rift", "astral_rift"),
        ("pop group", "pop_group"),
        ("no scope", "none"),
        ("system", "galactic_object"),
    ];
    if let Some((_, scope)) = aliases
        .iter()
        .find(|(term, _)| name == *term || name.starts_with(&format!("{term} ")))
    {
        return Some(Types::new(scope));
    }
    if let Some(owner) = name.strip_prefix("owner of ") {
        let words = owner.split_whitespace().collect::<Vec<_>>();
        if words.len() > 2 || words.len() == 2 && !words[1].chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let source = *words.first()?;
        let rule = rules.scopes.get("owner")?;
        if !SCOPES.split_whitespace().any(|s| s == source)
            || !rule.input.iter().any(|s| s == source || s == "all")
        {
            return None;
        }
        let output = rule
            .output_by_input
            .get(source)
            .map(String::as_str)
            .or(rule.output.as_deref())?;
        return (output == "country").then(|| Types::new("country"));
    }
    let word = name.split_whitespace().next()?;
    SCOPES
        .split_whitespace()
        .any(|s| s == word && !matches!(s, "all" | "various" | "no_scope" | "?"))
        .then(|| Types::new(word))
}

pub(crate) fn parse_annotations(
    text: &str,
    line: u32,
    name: &str,
    rules: &Rules,
) -> Option<DocumentedAction> {
    let lines = text
        .trim_start_matches('\u{feff}')
        .lines()
        .collect::<Vec<_>>();
    let index = line.checked_sub(1)? as usize;
    let declaration = lines.get(index)?.trim_start();
    if !declaration
        .strip_prefix(name)?
        .trim_start()
        .starts_with('=')
    {
        return None;
    }
    let mut start = index;
    while start > 0
        && (lines[start - 1].trim().is_empty() || lines[start - 1].trim_start().starts_with('#'))
    {
        start -= 1;
    }
    let mut slots = BTreeMap::<String, Types>::new();
    let mut optional = BTreeSet::new();
    let mut markers = Vec::new();
    let mut conflicts = BTreeSet::new();
    for (offset, raw) in lines[start..index].iter().enumerate() {
        let Some(comment) = raw.trim_start().strip_prefix('#') else {
            continue;
        };
        let Some((label, phrase)) = comment.trim().split_once('=') else {
            continue;
        };
        let label = label.trim().to_ascii_lowercase();
        if !matches!(label.as_str(), "this/root" | "this" | "root" | "scope")
            && repeated(&label, "from").is_none_or(|depth| depth > 6)
        {
            continue;
        }
        let labels: &[&str] = match label.as_str() {
            "this/root" => &["this", "root"],
            "scope" => &["this"],
            _ => &[label.as_str()],
        };
        markers.push(serde_json::json!({"line":start+offset+1,"text":raw,"slot":label}));
        let Some(mut types) = annotation_type(phrase, rules) else {
            conflicts.extend(labels.iter().map(|s| (*s).to_string()));
            continue;
        };
        let lower = phrase.to_ascii_lowercase();
        let nullable = ["if ", "optional", "invalid"]
            .iter()
            .any(|s| lower.contains(s));
        if nullable {
            types.unknown = true;
        }
        for slot in labels {
            if let Some(old) = slots.insert((*slot).into(), types.clone())
                && old != types
            {
                conflicts.insert((*slot).to_string());
            }
            if nullable {
                optional.insert((*slot).into());
            }
        }
    }
    for slot in conflicts {
        if slots.contains_key(&slot) {
            slots.insert(slot, Types::new("?"));
        }
    }
    if !slots.contains_key("this") && !slots.contains_key("root") {
        return None;
    }
    let mut context = Context::unknown_caller();
    context.this = slots
        .get("this")
        .cloned()
        .unwrap_or_else(|| Types::new("?"));
    context.root = slots
        .get("root")
        .cloned()
        .unwrap_or_else(|| Types::new("?"));
    context.object = format!("engine-action:{name}.this");
    context.root_object = if label_is_shared(&slots, &markers) {
        context.object.clone()
    } else {
        format!("engine-action:{name}.root")
    };
    let depth = slots
        .keys()
        .filter_map(|s| repeated(s, "from"))
        .max()
        .unwrap_or(0);
    for i in 1..=depth {
        context.froms.push(
            slots
                .get(&"from".repeat(i))
                .cloned()
                .unwrap_or_else(|| Types::new("?")),
        );
        context
            .from_objects
            .push(format!("engine-action:{name}.from:{i}"));
    }
    Some(DocumentedAction {
        context,
        optional,
        evidence: serde_json::json!({"markers":markers}),
    })
}

fn label_is_shared(slots: &BTreeMap<String, Types>, markers: &[serde_json::Value]) -> bool {
    slots
        .get("this")
        .is_some_and(|s| !s.unknown && Some(s) == slots.get("root"))
        && markers
            .iter()
            .any(|m| m["slot"].as_str() == Some("this/root"))
}

impl Engine {
    pub(crate) fn prepare_documented_actions(&mut self) {
        self.documented_action_entries.clear();
        let files = self
            .program
            .files
            .iter()
            .filter(|f| f.category == "on_actions")
            .cloned()
            .collect::<Vec<_>>();
        let mut conflicted = BTreeSet::new();
        for file in files {
            let Some(input) = self
                .program
                .inputs
                .iter()
                .find(|i| i.path == file.path && i.layer == 0)
                .cloned()
            else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&input.path) else {
                continue;
            };
            if sha256(&bytes) != input.sha256 {
                continue;
            }
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            for n in &file.ast {
                if !n.key.starts_with("on_") || conflicted.contains(&n.key) {
                    continue;
                }
                let Some(mut entry) = parse_annotations(&text, n.line, &n.key, &self.rules) else {
                    continue;
                };
                entry.evidence["path"] = input.path.clone().into();
                entry.evidence["relative_path"] = input.relative_path.clone().into();
                entry.evidence["sha256"] = input.sha256.clone().into();
                entry.evidence["declaration_line"] = n.line.into();
                entry.evidence["evidence_kind"] = "installed vanilla documentation comment".into();
                if let Some(old) = self.documented_action_entries.get(&n.key)
                    && old.context != entry.context
                {
                    self.documented_action_entries.remove(&n.key);
                    conflicted.insert(n.key.clone());
                    continue;
                }
                self.documented_action_entries.insert(n.key.clone(), entry);
            }
        }
    }
    pub(crate) fn documented_action_entry(&mut self, n: &Node) -> Option<Context> {
        let entry = self.documented_action_entries.get(&n.key)?.clone();
        let record = serde_json::json!({"name":n.key,"kind":"on_actions","source":n.source,"line":n.line,"context":entry.context,"optional_scopes":entry.optional,"registration":"explicit vanilla on_action annotation","engine_entry_registered":true,"ordinary_script_call_proven":false,"runtime_invocation_proven":false,"runtime_availability_proven":false,"runtime_target_validity_proven":false,"evidence":entry.evidence});
        if !self.engine_entries.contains(&record) {
            self.engine_entries.push(record);
        }
        Some(entry.context)
    }
    pub(crate) fn run_engine_script_entry(&mut self, effect: bool, name: &str, n: &Node) -> bool {
        let Some(ctx) =
            self.planet_killer_entry(name, if effect { "effects" } else { "triggers" }, n)
        else {
            return false;
        };
        let depth = self.chain.len();
        self.chain.push(serde_json::json!({"kind":"registered-engine-script-entry","definition":name,"path":n.source,"line":n.line,"context":ctx,"caller_proven":false,"runtime_invocation_proven":false}).into());
        if n.macro_parts.is_none() {
            if effect {
                self.effects(n.body(), &ctx, &mut State::default());
            } else {
                self.triggers(n.body(), &ctx, &State::default());
            }
        } else {
            self.gap(
                n,
                "engine entry macro arguments are not established",
                "engine-entry-expansion-unverified",
                Some(&ctx),
            );
        }
        self.chain.truncate(depth);
        true
    }
}
