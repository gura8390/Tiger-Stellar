//! Owned AST bridge for the standalone Stellaris syntax core.
use std::path::PathBuf;
use serde_json::{Value, json};
use crate::block::{Block, BlockItem, BV};
use crate::fileset::{FileEntry, FileKind, FileStage};
use crate::parse::{ParserMemory, pdxfile::parse_pdx_file};

fn block(b: &Block) -> Value {
    Value::Array(b.iter_items().map(|item| match item {
        BlockItem::Field(f) => {
            let mut n = json!({"key":f.0.as_str(),"op":f.1.to_string(),"line":f.0.loc.line,"column":f.0.loc.column});
            match &f.2 { BV::Value(t) => n["value"] = json!(t.as_str()), BV::Block(b) => {
                n["children"] = block(b);
                if let Some(source) = &b.source {
                    n["macro_parameters"] = json!(b.macro_parms());
                    n["macro_variables"] = json!(source.1.values().into_iter().collect::<std::collections::BTreeMap<_,_>>());
                    n["macro_parts"] = json!(source.0.iter().map(|p| json!({"parameter":p.kind() == crate::parse::pdxfile::MacroComponentKind::Macro,"text":p.token().as_str()})).collect::<Vec<_>>());
                }
            } }
            n
        },
        BlockItem::Value(t) => json!({"value":t.as_str(),"line":t.loc.line,"column":t.loc.column}),
        BlockItem::Block(b) => json!({"children":block(b),"line":b.loc.line,"column":b.loc.column}),
    }).collect())
}

pub fn parse(path: &str, content: String, globals: &str) -> Value {
    let entry = FileEntry::new(PathBuf::from(path), FileStage::NoStage, FileKind::Mod, PathBuf::from(path));
    let offset = if content.starts_with('\u{feff}') { 3 } else { 0 };
    let mut memory = ParserMemory::default();
    if !globals.is_empty() {
        crate::parse::pdxfile::parse_reader_export(&entry, globals.to_owned(), 0, &mut memory.pdxfile);
    }
    let b = parse_pdx_file(&entry, content, offset, &memory);
    let ast = block(&b);
    let mut diagnostics = vec![];
    for (m, occurrences) in crate::take_reports() {
        for pointers in occurrences {
            let p = pointers.first();
            diagnostics.push(json!({"severity":m.severity.to_string(),"key":m.key.to_string(),"message":m.msg,"line":p.map(|p|p.loc.line).unwrap_or(0),"column":p.map(|p|p.loc.column).unwrap_or(0)}));
        }
    }
    json!({"path":path,"ast":ast,"diagnostics":diagnostics})
}
