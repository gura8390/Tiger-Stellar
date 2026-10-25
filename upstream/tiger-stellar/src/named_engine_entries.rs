//! Planet-killer naming contracts, authorized by installed vanilla comment
//! markers and effective component registration, never by a name alone.
use crate::{
    ast::Node,
    domain::{Context, Types},
    engine::Engine,
    hash::sha256,
    loader::Input,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const TRIGGER_DOC: &str = "common/scripted_triggers/01_scripted_triggers_planet_killers.txt";
const ACTION_DOC: &str = "common/on_actions/01_planet_destruction.txt";

struct Evidence {
    value: Value,
    variants: bool,
}

fn evidence(kind: &str, path: &Path, bytes: &[u8]) -> Option<Evidence> {
    let (relative, required): (&str, &[&str]) = match kind {
        "triggers" => (
            TRIGGER_DOC,
            &[
                "this = planet being targeted",
                "from = fleet",
                "can_destroy_planet_with_planet_destruction_component = {",
            ],
        ),
        "on_actions" => (
            ACTION_DOC,
            &[
                "this/root = planet that have been fired upon",
                "from = fleet that fired",
                "on_destroy_planet_with_planet_destruction_component = {",
            ],
        ),
        _ => return None,
    };
    let text = std::str::from_utf8(bytes)
        .ok()?
        .trim_start_matches('\u{feff}');
    let comments = text
        .lines()
        .enumerate()
        .filter_map(|(line, text)| {
            let comment = text.trim_start().strip_prefix('#')?.trim();
            Some((line + 1, text, comment.to_ascii_lowercase()))
        })
        .collect::<Vec<_>>();
    let markers = required
        .iter()
        .map(|required| {
            let (line, text, _) = comments
                .iter()
                .find(|(_, _, comment)| comment.as_str() == *required)?;
            Some(json!({"line":line,"text":text}))
        })
        .collect::<Option<Vec<_>>>()?;
    let variant = comments
        .iter()
        .find(|(_, _, comment)| comment == "they use the same scopes as the standard events");
    Some(Evidence {
        value: json!({"relative_path":relative,"path":path.to_string_lossy(),"sha256":sha256(bytes),
            "evidence_kind":"installed vanilla documentation comment","markers":markers,
            "variant_scope_marker":variant.map(|(line,text,_)|json!({"line":line,"text":text}))}),
        variants: variant.is_some(),
    })
}

impl Engine {
    fn planet_killer_source(&mut self, kind: &str) -> Option<Evidence> {
        let relative = match kind {
            "triggers" => TRIGGER_DOC,
            "on_actions" => ACTION_DOC,
            _ => return None,
        };
        // launcher-settings is recorded from the explicit --game root, not a
        // dependency/mod directory. A layer-0 source input is a fallback for
        // library users that do not run the CLI launcher-version check.
        let path = self
            .program
            .inputs
            .iter()
            .find(|i| i.layer == 0 && i.relative_path == "launcher-settings.json")
            .and_then(|i| Path::new(&i.path).parent().map(|p| p.join(relative)))
            .or_else(|| {
                self.program
                    .inputs
                    .iter()
                    .find(|i| i.layer == 0 && i.relative_path == relative)
                    .map(|i| PathBuf::from(&i.path))
            })?
            .canonicalize()
            .ok()?;
        let bytes = std::fs::read(&path).ok()?;
        let digest = sha256(&bytes);
        if let Some(input) = self.program.inputs.iter().find(|i| {
            i.layer == 0
                && i.relative_path == relative
                && Path::new(&i.path).canonicalize().ok().as_deref() == Some(path.as_path())
        }) {
            if input.sha256 != digest {
                return None;
            }
        } else {
            // An overridden file may not be among effective parsed files, but
            // the original annotation is still an additional evidence input.
            self.program.inputs.push(Input {
                relative_path: relative.into(),
                path: path.to_string_lossy().into_owned(),
                sha256: digest,
                layer: 0,
            });
        }
        evidence(kind, &path, &bytes)
    }

    pub(crate) fn planet_killer_entry(
        &mut self,
        name: &str,
        kind: &str,
        node: &Node,
    ) -> Option<Context> {
        let prefix = match kind {
            "triggers" => "can_destroy_planet_with_",
            "on_actions" => "on_destroy_planet_with_",
            _ => return None,
        };
        name.strip_prefix(prefix)?;
        let source = self.planet_killer_source(kind)?;
        self.planet_killer_with_evidence(name, kind, node, source)
    }

    fn planet_killer_with_evidence(
        &mut self,
        name: &str,
        kind: &str,
        node: &Node,
        source: Evidence,
    ) -> Option<Context> {
        let prefix = match kind {
            "triggers" => "can_destroy_planet_with_",
            "on_actions" => "on_destroy_planet_with_",
            _ => return None,
        };
        let suffix = name.strip_prefix(prefix)?;
        let registered = |key: &str| {
            self.program
                .definitions
                .get("component_templates")?
                .get(key)
                .filter(|definition| definition.val("type") == Some("planet_killer"))
                .cloned()
        };
        let (component, definition, variant) = if let Some(definition) = registered(suffix) {
            (suffix, definition, None)
        } else if kind == "on_actions" && source.variants {
            let (base, variant) = suffix
                .strip_suffix("_unqueued")
                .map(|s| (s, "unqueued"))
                .or_else(|| suffix.strip_suffix("_queued").map(|s| (s, "queued")))?;
            (base, registered(base)?, Some(variant))
        } else {
            return None;
        };
        let mut ctx = Context::unknown_caller();
        ctx.this = Types::new("planet");
        ctx.object = format!("named-engine-entry:{kind}:{name}.this");
        ctx.root_object = format!("named-engine-entry:{kind}:{name}.unknown-root");
        if kind == "on_actions" {
            ctx.root = Types::new("planet");
            ctx.root_object = ctx.object.clone();
        }
        ctx.froms = vec![Types::new("fleet")];
        ctx.from_objects = vec![format!("named-engine-entry:{kind}:{name}.from")];
        let component_input = self
            .program
            .inputs
            .iter()
            .find(|i| i.path == definition.source.as_ref());
        let record = json!({"name":name,"kind":kind,"component":component,"variant":variant,
            "path":node.source,"line":node.line,"registration":"component and naming contract matched",
            "engine_entry_registered":true,"ordinary_script_call_proven":false,"runtime_invocation_proven":false,
            "runtime_availability_proven":false,"runtime_target_validity_proven":false,
            "context":{"this":ctx.this,"root":ctx.root,"froms":ctx.froms,"prevs":ctx.prevs},
            "component_registration":{"path":definition.source,"line":definition.line,"type":"planet_killer","source_sha256":component_input.map(|i|&i.sha256)},
            "evidence":source.value});
        if !self.engine_entries.contains(&record) {
            self.engine_entries.push(record.clone());
        }
        Some(ctx)
    }

    #[cfg(test)]
    pub(crate) fn planet_killer_test_entry(
        &mut self,
        name: &str,
        kind: &str,
        node: &Node,
        documentation: &str,
    ) -> Option<Context> {
        let source = evidence(
            kind,
            Path::new("fixture-vanilla.txt"),
            documentation.as_bytes(),
        )?;
        self.planet_killer_with_evidence(name, kind, node, source)
    }
}
