use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub const SCOPES: &str = "agreement ambient_object archaeological_site army astral_rift bypass colony cosmic_storm cosmic_storm_influence_field country debris deposit design dlc_recommendation espionage_asset espionage_operation exhibit federation first_contact fleet galactic_community galactic_object job leader megastructure mission none planet pop pop_faction pop_group sector ship ship_growth_stage situation species spy_network starbase trait war all various no_scope ?";
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub input: Vec<String>,
    pub output: Option<String>,
    pub iterator_output: Option<String>,
    #[serde(default)]
    pub output_by_input: BTreeMap<String, String>,
    #[serde(default)]
    pub boolean: bool,
    #[serde(default)]
    pub numeric: bool,
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default)]
    pub description: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Rules {
    pub game_id: String,
    pub game_version: String,
    pub triggers: BTreeMap<String, Rule>,
    pub effects: BTreeMap<String, Rule>,
    pub scopes: BTreeMap<String, Rule>,
    #[serde(default)]
    pub modifiers: BTreeSet<String>,
    #[serde(default)]
    pub numeric_expressions: BTreeMap<String, Rule>,
    pub sources: serde_json::Value,
}
impl Rules {
    pub fn validate(&self) -> Result<(), String> {
        if self.game_id != "stellaris" {
            return Err("wrong-game: Tiger Stellar requires Stellaris rules".into());
        }
        let valid = SCOPES.split_whitespace().collect::<BTreeSet<_>>();
        for (group, rows) in [
            ("triggers", &self.triggers),
            ("effects", &self.effects),
            ("scopes", &self.scopes),
            ("numeric_expressions", &self.numeric_expressions),
        ] {
            for (name, r) in rows {
                let mut kinds = r.input.iter().map(String::as_str).collect::<Vec<_>>();
                kinds.extend(
                    r.output
                        .as_deref()
                        .unwrap_or("")
                        .split('|')
                        .filter(|s| !s.is_empty()),
                );
                kinds.extend(
                    r.iterator_output
                        .as_deref()
                        .unwrap_or("")
                        .split('|')
                        .filter(|s| !s.is_empty()),
                );
                for (a, b) in &r.output_by_input {
                    kinds.push(a);
                    kinds.extend(b.split('|'));
                }
                if let Some(k) = kinds.into_iter().find(|k| !valid.contains(k)) {
                    return Err(format!("wrong-game: foreign scope {k} in {group}/{name}"));
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct FieldSchema {
    #[serde(default)]
    pub forms: Vec<String>,
    #[serde(default)]
    pub scalar_types: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Declarations {
    pub game_id: String,
    pub categories: BTreeMap<String, BTreeMap<String, FieldSchema>>,
}
