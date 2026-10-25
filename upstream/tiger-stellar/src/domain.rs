use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Types {
    pub known: BTreeSet<String>,
    pub unknown: bool,
}
impl Types {
    pub fn new(s: &str) -> Self {
        let mut t = Self::default();
        for v in s.split('|') {
            if matches!(v, "?" | "all" | "various" | "") {
                t.unknown = true;
            } else {
                t.known.insert(v.into());
            }
        }
        t
    }
    pub fn join(&self, b: &Self) -> Self {
        Self {
            known: self.known.union(&b.known).cloned().collect(),
            unknown: self.unknown || b.unknown,
        }
    }
    pub fn label(&self) -> String {
        let mut v = self.known.iter().cloned().collect::<Vec<_>>();
        if self.unknown {
            v.push("?".into());
        }
        v.join("|")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Availability {
    Absent,
    Maybe,
    Definite,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub types: Types,
    pub availability: Availability,
}
impl Default for Binding {
    fn default() -> Self {
        Self {
            types: Types::default(),
            availability: Availability::Absent,
        }
    }
}
impl Binding {
    pub fn assigned(types: Types) -> Self {
        Self {
            types,
            availability: Availability::Definite,
        }
    }
    pub fn join(&self, b: &Self) -> Self {
        Self {
            types: self.types.join(&b.types),
            availability: if self.availability == b.availability {
                self.availability
            } else {
                Availability::Maybe
            },
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub bindings: BTreeMap<String, Binding>,
    // Guard-conditioned deltas, not combinations of complete execution paths.
    pub guarded: BTreeMap<String, BTreeMap<String, Binding>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub numbers: BTreeMap<String, NumberFact>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub guarded_numbers: BTreeMap<String, BTreeMap<String, NumberFact>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumberFact {
    pub availability: Availability,
    // Canonical finite f64 text keeps equality deterministic and the domain
    // finite-height: differing constants join to an unknown runtime number.
    pub constant: Option<String>,
    #[serde(default)]
    pub nonzero: bool,
}
impl Default for NumberFact {
    fn default() -> Self {
        Self {
            availability: Availability::Maybe,
            constant: None,
            nonzero: false,
        }
    }
}
impl NumberFact {
    pub fn assigned(value: Option<f64>) -> Self {
        Self {
            availability: Availability::Definite,
            constant: value
                .filter(|v| v.is_finite() && (*v as f32) as f64 == *v)
                .map(|v| if v == 0.0 { "0".into() } else { v.to_string() }),
            nonzero: value.is_some_and(|v| {
                v.is_finite() && (v as f32).is_finite() && v != 0.0 && (v as f32) != 0.0
            }),
        }
    }
    pub fn value(&self) -> Option<f64> {
        self.constant.as_ref()?.parse().ok()
    }
    pub(crate) fn join(&self, other: &Self) -> Self {
        Self {
            availability: if self.availability == other.availability {
                self.availability
            } else {
                Availability::Maybe
            },
            constant: if self.constant == other.constant {
                self.constant.clone()
            } else {
                None
            },
            nonzero: self.nonzero && other.nonzero,
        }
    }
}
impl State {
    pub fn number_snapshot(&self) -> Self {
        Self {
            numbers: self.numbers.clone(),
            guarded_numbers: self.guarded_numbers.clone(),
            ..Self::default()
        }
    }
    pub fn number_key(object: &str, name: &str) -> String {
        format!("{object}\u{1f}{name}")
    }
    pub fn number(&self, key: &str) -> NumberFact {
        self.numbers.get(key).cloned().unwrap_or_default()
    }
    pub fn set_number(&mut self, key: String, value: NumberFact) {
        // Different lexical paths are not proof of distinct objects. A write
        // through owner/FROM/targets may alias a previously tracked receiver.
        let name = key.rsplit('\u{1f}').next().unwrap_or("");
        for (other, fact) in &mut self.numbers {
            if other != &key && other.rsplit('\u{1f}').next() == Some(name) {
                // The write may affect this object, or a different one. Join
                // both possibilities in both directions, including Absent ->
                // Maybe when a possible alias has just been assigned.
                *fact = fact.join(&value);
            }
        }
        self.numbers.insert(key.clone(), value);
        for facts in self.guarded_numbers.values_mut() {
            facts.retain(|other, _| other.rsplit('\u{1f}').next() != Some(name));
        }
        self.guarded_numbers.retain(|_, facts| !facts.is_empty());
    }
    pub fn clear_correlations(&mut self) {
        self.guarded.clear();
        self.guarded_numbers.clear();
    }
    pub fn forget_numbers(&mut self) {
        self.numbers.clear();
        self.guarded_numbers.clear();
    }
    pub fn retain_number_anchors(&mut self) {
        // Constructors can redirect owner/species/starbase lookup paths.
        let stable = |key: &str| {
            let owner = key.split('\u{1f}').next().unwrap_or("");
            owner == "root"
                || ((owner.starts_with("event:") || owner.starts_with("inferred-script:"))
                    && owner.ends_with(".this"))
        };
        self.numbers.retain(|key, _| stable(key));
        for facts in self.guarded_numbers.values_mut() {
            facts.retain(|key, _| stable(key));
        }
        self.guarded_numbers.retain(|_, facts| !facts.is_empty());
    }
    pub fn forget_number_object(&mut self, object: &str) {
        let matches = |key: &str| {
            let owner = key.split('\u{1f}').next().unwrap_or("");
            owner == object
                || owner
                    .strip_prefix(object)
                    .is_some_and(|tail| tail.starts_with('.'))
        };
        self.numbers.retain(|key, _| !matches(key));
        for facts in self.guarded_numbers.values_mut() {
            facts.retain(|key, _| !matches(key));
        }
        self.guarded_numbers.retain(|_, facts| !facts.is_empty());
    }
    pub fn forget_number_alias(&mut self, target: &str) {
        let other = alias(target);
        let matches = |key: &str| {
            key.split('\u{1f}')
                .next()
                .unwrap_or("")
                .split('.')
                .any(|part| part == target || part == other)
        };
        self.numbers.retain(|key, _| !matches(key));
        for facts in self.guarded_numbers.values_mut() {
            facts.retain(|key, _| !matches(key));
        }
        self.guarded_numbers.retain(|_, facts| !facts.is_empty());
    }
    pub fn forget_number_targets(&mut self) {
        let matches = |key: &str| {
            key.split('\u{1f}')
                .next()
                .unwrap_or("")
                .split('.')
                .any(|part| {
                    part.starts_with("event_target:") || part.starts_with("global_event_target:")
                })
        };
        self.numbers.retain(|key, _| !matches(key));
        for facts in self.guarded_numbers.values_mut() {
            facts.retain(|key, _| !matches(key));
        }
        self.guarded_numbers.retain(|_, facts| !facts.is_empty());
    }
    pub(crate) fn target_binding(&self, key: &str) -> Binding {
        self.bindings
            .get(key)
            .or_else(|| {
                if key.starts_with("event_target:") {
                    self.bindings.get(&alias(key))
                } else {
                    None
                }
            })
            .cloned()
            .unwrap_or_default()
    }
    pub fn get(&self, k: &str) -> Binding {
        self.bindings.get(k).cloned().unwrap_or_default()
    }
    pub fn set(&mut self, k: String, b: Binding) {
        self.bindings.insert(k.clone(), b);
        for facts in self.guarded.values_mut() {
            facts.remove(&k);
        }
        self.guarded.retain(|_, v| !v.is_empty());
    }
    pub fn join(&self, b: &Self) -> Self {
        let keys = self
            .bindings
            .keys()
            .chain(b.bindings.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let bindings = keys
            .into_iter()
            .map(|k| {
                let v = self.get(&k).join(&b.get(&k));
                (k, v)
            })
            .collect();
        let guarded = self
            .guarded
            .iter()
            .filter_map(|(k, v)| {
                let other = b.guarded.get(k)?;
                let common = v
                    .iter()
                    .filter(|(n, f)| other.get(*n) == Some(*f))
                    .map(|(n, f)| (n.clone(), f.clone()))
                    .collect::<BTreeMap<_, _>>();
                if common.is_empty() {
                    None
                } else {
                    Some((k.clone(), common))
                }
            })
            .collect();
        let numbers = self
            .numbers
            .keys()
            .chain(b.numbers.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|k| {
                let fact = self.number(&k).join(&b.number(&k));
                (k, fact)
            })
            .collect();
        let guarded_numbers = self
            .guarded_numbers
            .iter()
            .filter_map(|(k, facts)| {
                let other = b.guarded_numbers.get(k)?;
                let common = facts
                    .iter()
                    .filter(|(n, f)| other.get(*n) == Some(*f))
                    .map(|(n, f)| (n.clone(), f.clone()))
                    .collect::<BTreeMap<_, _>>();
                (!common.is_empty()).then(|| (k.clone(), common))
            })
            .collect();
        Self {
            bindings,
            guarded,
            numbers,
            guarded_numbers,
        }
    }
    pub fn assume(&mut self, key: &str) {
        if let Some(facts) = self.guarded.get(key).cloned() {
            self.bindings.extend(facts);
        }
        if let Some(facts) = self.guarded_numbers.get(key).cloned() {
            self.numbers.extend(facts);
        }
    }
    pub fn remember(&mut self, key: String, branch: &Self, base: &Self) {
        let keys = branch
            .bindings
            .keys()
            .chain(base.bindings.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let facts = keys
            .into_iter()
            .filter_map(|k| {
                let v = branch.get(&k);
                if v != base.get(&k) {
                    Some((k, v))
                } else {
                    None
                }
            })
            .collect::<BTreeMap<_, _>>();
        if !facts.is_empty() {
            self.guarded.insert(key.clone(), facts);
        }
        // A target binding can be rebound; do not use its mere existence as
        // a persistent numeric identity correlation.
        if !key.contains(":exists:") {
            let facts = branch
                .numbers
                .keys()
                .chain(base.numbers.keys())
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter_map(|k| {
                    let v = branch.number(&k);
                    (v != base.number(&k)).then_some((k, v))
                })
                .collect::<BTreeMap<_, _>>();
            if !facts.is_empty() {
                self.guarded_numbers.insert(key, facts);
            }
        }
    }
    pub fn project(&self, reads: &BTreeSet<String>) -> Self {
        if reads.contains("*") {
            return self.clone();
        }
        let needed = |k: &str| reads.contains(k) || reads.contains(&alias(k));
        let guarded = self
            .guarded
            .iter()
            .filter_map(|(g, facts)| {
                let f = facts
                    .iter()
                    .filter(|(k, _)| needed(k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<BTreeMap<_, _>>();
                if f.is_empty() {
                    None
                } else {
                    Some((g.clone(), f))
                }
            })
            .collect();
        Self {
            bindings: self
                .bindings
                .iter()
                .filter(|(k, _)| needed(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            guarded,
            numbers: self.numbers.clone(),
            guarded_numbers: self.guarded_numbers.clone(),
        }
    }
}
pub fn alias(k: &str) -> String {
    if let Some(v) = k.strip_prefix("global_event_target:") {
        format!("event_target:{v}")
    } else if let Some(v) = k.strip_prefix("event_target:") {
        format!("global_event_target:{v}")
    } else {
        k.into()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub this: Types,
    pub root: Types,
    pub froms: Vec<Types>,
    pub prevs: Vec<Types>,
    pub object: String,
    pub root_object: String,
    pub from_objects: Vec<String>,
    pub prev_objects: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub prev_tail_unknown: bool,
}
fn is_false(value: &bool) -> bool {
    !*value
}
impl Context {
    pub fn new(scope: &str) -> Self {
        Self {
            this: Types::new(scope),
            root: Types::new(scope),
            froms: vec![],
            prevs: vec![],
            object: "root".into(),
            root_object: "root".into(),
            from_objects: vec![],
            prev_objects: vec![],
            prev_tail_unknown: false,
        }
    }
    pub fn unknown_caller() -> Self {
        let mut ctx = Self::new("?");
        ctx.prev_tail_unknown = true;
        ctx
    }
    pub fn push(&self, t: Types, identity: &str) -> Self {
        let mut c = self.clone();
        c.prevs.insert(0, self.this.clone());
        c.prev_objects.insert(0, self.object.clone());
        c.this = t;
        c.object = format!("{}.{}", self.object, identity);
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absence_does_not_erase_type() {
        let b = Binding::assigned(Types::new("planet")).join(&Binding::default());
        assert_eq!(b.types.label(), "planet");
        assert_eq!(b.availability, Availability::Maybe);
    }
    #[test]
    fn assigned_unknown_keeps_known_candidates() {
        let b = Binding::assigned(Types::new("country")).join(&Binding::assigned(Types::new("?")));
        assert!(b.types.unknown);
        assert!(b.types.known.contains("country"));
    }
    #[test]
    fn assignment_overwrites_and_invalidates_guard() {
        let mut s = State::default();
        let mut yes = s.clone();
        yes.set(
            "event_target:t".into(),
            Binding::assigned(Types::new("planet")),
        );
        s.remember("g".into(), &yes, &s.clone());
        s.set(
            "event_target:t".into(),
            Binding::assigned(Types::new("country")),
        );
        s.assume("g");
        assert_eq!(s.get("event_target:t").types.label(), "country");
    }
}
