use crate::{ast::Node, hash::sha256, parser::ParserClient, report::Diagnostics};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug, Serialize)]
pub struct Input {
    pub relative_path: String,
    pub path: String,
    pub sha256: String,
    pub layer: usize,
}
#[derive(Clone)]
pub struct File {
    pub path: String,
    pub category: String,
    pub ast: Vec<Arc<Node>>,
    pub target: bool,
}
#[derive(Default)]
pub struct Program {
    pub files: Vec<File>,
    pub scripts: BTreeMap<(bool, String), Arc<Node>>,
    pub events: BTreeMap<String, (String, Arc<Node>)>,
    pub definitions: BTreeMap<String, BTreeMap<String, Arc<Node>>>,
    pub entities: BTreeMap<String, BTreeSet<String>>,
    pub inline: BTreeMap<String, (String, String)>,
    pub globals: String,
    pub inputs: Vec<Input>,
    pub loc: BTreeMap<String, BTreeSet<String>>,
    pub target_files: usize,
    pub next_id: u64,
    pub readers: BTreeMap<String, Vec<String>>,
    pub weapon_stats: BTreeMap<String, crate::csv::WeaponStats>,
    /// Effective resource providers, using the same overlay and replace_path as scripts.
    /// Bytes are read and hash-bound only when a semantic rule references them.
    pub asset_files: BTreeMap<String, (PathBuf, usize)>,
    pub asset_health: BTreeMap<String, bool>,
    pub relative_sources: BTreeMap<String, String>,
}
pub fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map(|s| s.trim_start_matches('\u{feff}').to_string())
        .map_err(|e| format!("{}: {e}", path.display()))
}
fn walk(root: &Path, base: &Path, files: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    if !base.exists() {
        return Ok(());
    }
    let mut entries = std::fs::read_dir(base)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let kind = e.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            return Err(format!("linked input is not supported: {}", p.display()));
        }
        if kind.is_dir() {
            walk(root, &p, files)?;
        } else if kind.is_file() {
            files.insert(
                p.strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_ascii_lowercase(),
                p,
            );
        }
    }
    Ok(())
}
pub fn event_scope(k: &str) -> Option<&'static str> {
    Some(match k {
        "country_event" | "observer_event" => "country",
        "planet_event" => "planet",
        "ship_event" => "ship",
        "fleet_event" => "fleet",
        "pop_event" => "pop",
        "pop_group_event" => "pop_group",
        "leader_event" => "leader",
        "starbase_event" => "starbase",
        "system_event" => "galactic_object",
        "situation_event" => "situation",
        "colony_event" => "colony",
        "carrier_event" => "planet|ship",
        "event" => "none",
        "astral_rift_event" => "astral_rift",
        "first_contact_event" => "first_contact",
        "espionage_operation_event" => "espionage_operation",
        "agreement_event" => "agreement",
        "bypass_event" => "bypass",
        "pop_faction_event" => "pop_faction",
        _ => return None,
    })
}
impl Program {
    pub fn reader_header(&self, path: &str) -> String {
        let local = self.readers.get(path).cloned().unwrap_or_default();
        let names = local
            .iter()
            .filter_map(|line| reader_name(line))
            .collect::<BTreeSet<_>>();
        let global = self
            .globals
            .lines()
            .filter(|line| reader_name(line).is_none_or(|name| !names.contains(name)))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{global}\n{}", local.join("\n"))
    }
    pub fn index(&mut self, file: File) {
        if matches!(file.category.as_str(), "gfx" | "interface") {
            fn graphics_index(n: &Node, entities: &mut BTreeMap<String, BTreeSet<String>>) {
                let kind = n.key.to_ascii_lowercase();
                if matches!(kind.as_str(), "spritetype" | "pdxmesh" | "entity") {
                    if let Some(name) = n.val("name") {
                        entities
                            .entry(format!("gfx_{kind}"))
                            .or_default()
                            .insert(name.into());
                    }
                } else if kind == "portraits" {
                    entities.entry("gfx_portrait".into()).or_default().extend(
                        n.body()
                            .iter()
                            .filter(|c| !c.key.is_empty() && !c.key.starts_with('@'))
                            .map(|c| c.key.clone()),
                    );
                }
                for c in n.body() {
                    graphics_index(c, entities);
                }
            }
            for n in &file.ast {
                graphics_index(n, &mut self.entities);
            }
        }
        for n in &file.ast {
            if n.key.is_empty() || n.key.starts_with('@') {
                continue;
            }
            if file.category == "events" {
                if let Some(scope) = event_scope(&n.key)
                    && let Some(id) = n.val("id")
                {
                    self.events.insert(id.into(), (scope.into(), n.clone()));
                }
            } else if matches!(
                file.category.as_str(),
                "scripted_effects" | "scripted_triggers"
            ) {
                self.scripts.insert(
                    (file.category == "scripted_effects", n.key.clone()),
                    n.clone(),
                );
            } else {
                let id = match file.category.as_str() {
                    "component_templates"
                    | "component_sets"
                    | "section_templates"
                    | "special_projects" => n.val("key").unwrap_or(&n.key),
                    "global_ship_designs" => n.val("name").unwrap_or(&n.key),
                    _ => &n.key,
                }
                .to_string();
                self.entities
                    .entry(file.category.clone())
                    .or_default()
                    .insert(id.clone());
                self.definitions
                    .entry(file.category.clone())
                    .or_default()
                    .insert(id, n.clone());
            }
        }
        if file.target {
            self.target_files += 1;
        }
        self.files.push(file);
    }
    pub fn load(
        game: &Path,
        mods: &[PathBuf],
        parser: &mut ParserClient,
        diagnostics: &mut Diagnostics,
    ) -> Result<Self, String> {
        let mut program = Self::default();
        let mut final_files = BTreeMap::new();
        let layers = std::iter::once(game.to_path_buf())
            .chain(mods.iter().cloned())
            .collect::<Vec<_>>();
        for (layer, root) in layers.iter().enumerate() {
            let root = root
                .canonicalize()
                .map_err(|e| format!("{}: {e}", root.display()))?;
            let descriptor = root.join("descriptor.mod");
            if layer > 0 && descriptor.exists() {
                let text = read(&descriptor)?;
                program.record(&descriptor, "descriptor.mod", layer)?;
                let parsed = parser.parse(&descriptor.to_string_lossy(), &text, "")?;
                for n in parsed.ast {
                    if n.key == "replace_path" {
                        let prefix = n.scalar().trim_matches('/').to_ascii_lowercase() + "/";
                        final_files.retain(|rel: &String, _| !rel.starts_with(&prefix));
                        program
                            .asset_files
                            .retain(|rel, _| !rel.starts_with(&prefix));
                    }
                }
            }
            let mut found = BTreeMap::new();
            for folder in [
                "common",
                "events",
                "gfx",
                "interface",
                "localisation",
                "flags",
            ] {
                walk(&root, &root.join(folder), &mut found)?;
            }
            for (rel, p) in found {
                if rel.starts_with("gfx/") || rel.starts_with("flags/") {
                    program.asset_files.insert(rel.clone(), (p.clone(), layer));
                }
                let suffix = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let base = p
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                if base.starts_with("readme")
                    || base.starts_with("99_readme")
                    || matches!(
                        base.as_str(),
                        "how_to_make_new_ships.txt"
                            | "reference.txt"
                            | "license.txt"
                            | "startup_info.txt"
                    )
                {
                    continue;
                }
                let allowed = if rel.starts_with("flags/") {
                    // Only swatches are a modeled flag declaration source.
                    // Usage metadata is not a gfx/common declaration file.
                    rel == "flags/colors.txt"
                } else if rel.starts_with("localisation/") {
                    suffix == "yml"
                } else if rel.starts_with("common/") || rel.starts_with("events/") {
                    suffix == "txt"
                        || (rel.starts_with("common/component_templates/") && suffix == "csv")
                } else {
                    matches!(suffix.as_str(), "txt" | "gfx" | "asset" | "gui")
                };
                if allowed {
                    final_files.insert(rel, (p, layer));
                }
            }
        }
        for (rel, (p, _)) in &final_files {
            if rel.starts_with("common/scripted_variables/") {
                program.globals.push_str(&read(p)?);
                program.globals.push('\n');
            }
        }
        for (rel, (path, layer)) in final_files {
            program.record(&path, &rel, layer)?;
            let text = read(&path)?;
            if rel.starts_with("common/component_templates/") && rel.ends_with(".csv") {
                let source = path.to_string_lossy().into_owned();
                match crate::csv::weapons(&text, &source) {
                    Ok(rows) => program.weapon_stats.extend(rows),
                    Err(e) => {
                        let n = Node {
                            source: source.into(),
                            ..Default::default()
                        };
                        diagnostics.emit(
                            &n,
                            "weapon-csv",
                            e,
                            ("error", "confirmed-rule-violation"),
                            None,
                            &[],
                        );
                    }
                }
                continue;
            }
            if rel.starts_with("localisation/") {
                let lang = text
                    .lines()
                    .find_map(|line| {
                        line.trim()
                            .strip_prefix("l_")
                            .and_then(|s| s.strip_suffix(':'))
                    })
                    .map(str::to_string);
                if let Some(lang) = lang {
                    let keys = program.loc.entry(lang).or_default();
                    for line in text.lines() {
                        if let Some((key, rest)) = line.trim().split_once(':')
                            && !key.starts_with('#')
                            && rest.contains('"')
                        {
                            keys.insert(key.into());
                        }
                    }
                }
                continue;
            }
            let source = path.to_string_lossy().into_owned();
            program.relative_sources.insert(source.clone(), rel.clone());
            program.readers.insert(
                source.clone(),
                text.lines()
                    .filter(|line| reader_name(line).is_some())
                    .map(str::to_string)
                    .collect(),
            );
            if let Some(name) = rel
                .strip_prefix("common/inline_scripts/")
                .and_then(|s| s.strip_suffix(".txt"))
            {
                program.inline.insert(name.into(), (source, text));
                continue;
            }
            let mut result = parser.parse(&source, &text, &program.globals)?;
            for n in &mut result.ast {
                n.locate(&source, &mut program.next_id);
            }
            let target = mods.is_empty() || layer == mods.len();
            if target {
                for d in result.diagnostics {
                    let n = Node {
                        source: source.clone().into(),
                        line: d["line"].as_u64().unwrap_or(0) as u32,
                        column: d["column"].as_u64().unwrap_or(0) as u32,
                        ..Default::default()
                    };
                    let severity = d["severity"].as_str().unwrap_or("error");
                    diagnostics.emit(
                        &n,
                        d["key"].as_str().unwrap_or("syntax"),
                        d["message"].as_str().unwrap_or("parser diagnostic").into(),
                        (
                            severity,
                            if severity == "error" {
                                "confirmed-rule-violation"
                            } else {
                                "formatting-suspicion"
                            },
                        ),
                        None,
                        &[],
                    );
                }
            }
            if rel == "flags/colors.txt" {
                program
                    .entities
                    .entry("flag_colors".into())
                    .or_default()
                    .extend(
                        result
                            .ast
                            .iter()
                            .filter(|n| n.key == "colors")
                            .flat_map(|n| n.body())
                            .filter(|n| !n.key.is_empty() && !n.key.starts_with('@'))
                            .map(|n| n.key.clone()),
                    );
                continue;
            }
            let parts = rel.split('/').collect::<Vec<_>>();
            let category = if parts[0] == "common" {
                if matches!(parts.get(1), Some(&"governments") | Some(&"patrons")) {
                    parts[1..parts.len() - 1].join("/")
                } else {
                    parts.get(1).unwrap_or(&"").to_string()
                }
            } else {
                parts[0].into()
            };
            program.index(File {
                path: source,
                category,
                ast: result.ast.into_iter().map(Arc::new).collect(),
                target,
            });
        }
        Ok(program)
    }
    pub fn record(&mut self, path: &Path, rel: &str, layer: usize) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        self.inputs.push(Input {
            relative_path: rel.into(),
            path: path.to_string_lossy().into_owned(),
            sha256: sha256(&bytes),
            layer,
        });
        Ok(())
    }
}
fn reader_name(line: &str) -> Option<&str> {
    let (key, _) = line.trim_start().split_once('=')?;
    let key = key.trim().strip_prefix('@')?;
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        None
    } else {
        Some(key)
    }
}
