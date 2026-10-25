use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt,
    ops::Deref,
    sync::{Arc, LazyLock},
};

/// Most nodes have no macro variables. Keep an empty map inline-free while
/// preserving the existing JSON object representation and sorted iteration.
#[derive(Clone, Debug, Default)]
pub struct MacroVariables(Option<Arc<BTreeMap<String, String>>>);
impl Deref for MacroVariables {
    type Target = BTreeMap<String, String>;
    fn deref(&self) -> &Self::Target {
        static EMPTY: BTreeMap<String, String> = BTreeMap::new();
        self.0.as_deref().unwrap_or(&EMPTY)
    }
}
impl Serialize for MacroVariables {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.deref().serialize(s)
    }
}
impl<'de> Deserialize<'de> for MacroVariables {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let variables = BTreeMap::<String, String>::deserialize(d)?;
        Ok(Self((!variables.is_empty()).then(|| Arc::new(variables))))
    }
}
/// Comparison operators are overwhelmingly drawn from this small vocabulary.
/// Unknown strings are retained exactly, rather than normalized or discarded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operator(Arc<str>);
impl Default for Operator {
    fn default() -> Self {
        Self::from("")
    }
}
impl Operator {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn owned_capacity(&self) -> usize {
        if matches!(
            self.as_str(),
            "" | "=" | "!=" | "<" | "<=" | ">" | ">=" | "?="
        ) {
            0
        } else {
            self.0.len()
        }
    }
}
impl From<&str> for Operator {
    fn from(s: &str) -> Self {
        static COMMON: LazyLock<[Arc<str>; 8]> =
            LazyLock::new(|| ["", "=", "!=", "<", "<=", ">", ">=", "?="].map(Arc::from));
        let index = match s {
            "" => 0,
            "=" => 1,
            "!=" => 2,
            "<" => 3,
            "<=" => 4,
            ">" => 5,
            ">=" => 6,
            "?=" => 7,
            _ => return Self(Arc::from(s)),
        };
        Self(COMMON[index].clone())
    }
}
impl PartialEq<&str> for Operator {
    fn eq(&self, s: &&str) -> bool {
        self.as_str() == *s
    }
}
impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl Serialize for Operator {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for Operator {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Operator;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a comparison operator string")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Operator, E> {
                Ok(s.into())
            }
        }
        d.deserialize_str(Visitor)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MacroPart {
    pub parameter: bool,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Node {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub children: Option<Box<[Node]>>,
    #[serde(default)]
    pub line: u32,
    #[serde(default)]
    pub column: u32,
    #[serde(default)]
    pub op: Operator,
    #[serde(default)]
    pub macro_parts: Option<Box<[MacroPart]>>,
    #[serde(default)]
    pub macro_variables: MacroVariables,
    #[serde(skip, default = "empty_source")]
    pub source: std::sync::Arc<str>,
    #[serde(skip)]
    pub id: u64,
    #[serde(skip)]
    pub reader_globals: Option<std::sync::Arc<str>>,
}
fn empty_source() -> Arc<str> {
    static EMPTY: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from(""));
    EMPTY.clone()
}
impl Default for Node {
    fn default() -> Self {
        Self {
            key: String::new(),
            value: None,
            children: None,
            line: 0,
            column: 0,
            op: Operator::default(),
            macro_parts: None,
            macro_variables: MacroVariables::default(),
            source: empty_source(),
            id: 0,
            reader_globals: None,
        }
    }
}
impl Node {
    pub fn is_memory_directive(&self) -> bool {
        self.key.is_empty() && self.children.is_none() && self.scalar() == "optimize_memory"
    }
    pub fn body(&self) -> &[Node] {
        self.children.as_deref().unwrap_or(&[])
    }
    pub fn field(&self, key: &str) -> Option<&Node> {
        self.body().iter().rev().find(|n| n.key == key)
    }
    pub fn val(&self, key: &str) -> Option<&str> {
        self.field(key).and_then(|n| n.value.as_deref())
    }
    pub fn scalar(&self) -> &str {
        self.value.as_deref().unwrap_or("")
    }
    pub fn locate(&mut self, path: &str, next: &mut u64) {
        self.locate_shared(&std::sync::Arc::from(path), next);
    }
    fn locate_shared(&mut self, source: &std::sync::Arc<str>, next: &mut u64) {
        self.source = source.clone();
        self.id = *next;
        *next += 1;
        if let Some(items) = self.children.as_mut() {
            for n in items.iter_mut() {
                n.locate_shared(source, next);
            }
        }
    }
    pub(crate) fn clone_header(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
            children: None,
            line: self.line,
            column: self.column,
            op: self.op.clone(),
            macro_parts: self.macro_parts.clone(),
            macro_variables: self.macro_variables.clone(),
            source: self.source.clone(),
            id: self.id,
            reader_globals: self.reader_globals.clone(),
        }
    }
    pub fn signature(&self) -> String {
        // Semantic identity, independent of physical line numbers and IDs.
        if let Some(v) = &self.value {
            format!("{}{}{:?}", self.key, self.op, v)
        } else {
            format!(
                "{}{}{{{}}}",
                self.key,
                self.op,
                self.body()
                    .iter()
                    .map(Self::signature)
                    .collect::<Vec<_>>()
                    .join(";")
            )
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Parsed {
    #[serde(default)]
    pub ast: Vec<Node>,
    #[serde(default)]
    pub diagnostics: Vec<serde_json::Value>,
    pub fatal: Option<String>,
}
