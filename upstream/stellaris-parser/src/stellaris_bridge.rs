//! Owned AST bridge for the standalone Stellaris syntax core.
use crate::block::{BV, Block, BlockItem};
use crate::fileset::{FileEntry, FileKind, FileStage};
use crate::parse::{
    ParserMemory,
    pdxfile::{parse_pdx_file, parse_reader_export},
};
use serde_json::{Value, json};
use std::{collections::VecDeque, path::PathBuf};
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

fn file_entry(path: &str) -> FileEntry {
    FileEntry::new(
        PathBuf::from(path),
        FileStage::NoStage,
        FileKind::Mod,
        PathBuf::from(path),
    )
}

fn parse_body(entry: &FileEntry, path: &str, content: String, memory: &ParserMemory) -> Value {
    let offset = if content.starts_with('\u{feff}') {
        3
    } else {
        0
    };
    let b = parse_pdx_file(entry, content, offset, memory);
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

/// Uncached syntax oracle. In particular, direct callers do not acquire a
/// process-global cache or share reader definitions with another parse.
pub fn parse(path: &str, content: String, globals: &str) -> Value {
    let entry = file_entry(path);
    let mut memory = ParserMemory::default();
    if !globals.is_empty() {
        let _ = parse_reader_export(&entry, globals.to_owned(), 0, &mut memory.pdxfile);
    }
    parse_body(&entry, path, content, &memory)
}

const READER_CACHE_ENTRIES: usize = 4;
const READER_CACHE_KEY_BYTES: usize = 1024 * 1024;
const READER_CACHE_HEADER_BYTES: usize = 512 * 1024;

struct CompiledReaderHeader {
    path: String,
    header: String,
    memory: ParserMemory,
    key_bytes: usize,
}

/// A worker-local session caching only successful, report-free reader-header
/// compilation. The budgets count entry keys' UTF-8 bytes, not total heap usage:
/// upstream tokens/source arenas have a separate worker-lifetime ownership rule.
/// The body always uses the parser's fresh CombinedMemory local overlay.
pub struct ParserSession {
    headers: VecDeque<CompiledReaderHeader>,
    key_bytes: usize,
    entry_limit: usize,
    key_byte_limit: usize,
    header_byte_limit: usize,
    #[cfg(test)]
    hits: usize,
    #[cfg(test)]
    header_compilations: usize,
}

impl Default for ParserSession {
    fn default() -> Self {
        Self {
            headers: VecDeque::new(),
            key_bytes: 0,
            entry_limit: READER_CACHE_ENTRIES,
            key_byte_limit: READER_CACHE_KEY_BYTES,
            header_byte_limit: READER_CACHE_HEADER_BYTES,
            #[cfg(test)]
            hits: 0,
            #[cfg(test)]
            header_compilations: 0,
        }
    }
}

fn clean_header(start_empty: bool, parse_success: bool, end_empty: bool) -> bool {
    start_empty && parse_success && end_empty
}

impl ParserSession {
    fn admitted_key_bytes(&self, path: &str, header: &str) -> Option<usize> {
        let key_bytes = path.len().checked_add(header.len())?;
        (self.entry_limit != 0
            && header.len() <= self.header_byte_limit
            && key_bytes <= self.key_byte_limit)
            .then_some(key_bytes)
    }

    fn admit(&mut self, path: &str, header: &str, memory: ParserMemory) {
        let Some(key_bytes) = self.admitted_key_bytes(path, header) else {
            return;
        };
        while self.headers.len() >= self.entry_limit
            || self.key_bytes > self.key_byte_limit - key_bytes
        {
            let Some(oldest) = self.headers.pop_front() else {
                break;
            };
            self.key_bytes -= oldest.key_bytes;
        }
        self.key_bytes += key_bytes;
        self.headers.push_back(CompiledReaderHeader {
            path: path.to_owned(),
            header: header.to_owned(),
            memory,
            key_bytes,
        });
    }

    pub fn parse(&mut self, path: &str, content: String, globals: &str) -> Value {
        let entry = file_entry(path);
        if globals.is_empty() {
            return parse_body(&entry, path, content, &ParserMemory::default());
        }
        // A worker request normally starts with empty storage. If another caller
        // left pending reports, retain the uncached merge path and never drain
        // that caller's diagnostics to manufacture a cache admission decision.
        let start_empty = crate::report::reports_are_empty();
        if start_empty
            && let Some(index) = self
                .headers
                .iter()
                .position(|cached| cached.path == path && cached.header == globals)
        {
            let cached = self.headers.remove(index).unwrap();
            #[cfg(test)]
            {
                self.hits += 1;
            }
            let parsed = parse_body(&entry, path, content, &cached.memory);
            self.headers.push_back(cached);
            return parsed;
        }

        #[cfg(test)]
        {
            self.header_compilations += 1;
        }
        let mut memory = ParserMemory::default();
        let success = parse_reader_export(&entry, globals.to_owned(), 0, &mut memory.pdxfile);
        // Inspect before the body: a dirty body must not invalidate an otherwise
        // clean header, and a clean body must not hide a dirty/failed header.
        let cacheable = clean_header(start_empty, success, crate::report::reports_are_empty());
        let parsed = parse_body(&entry, path, content, &memory);
        if cacheable {
            self.admit(path, globals, memory);
        }
        parsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    // The upstream report store is process-global. These tests compare complete
    // parse transactions, so keep their report observations in one sequence.
    static BRIDGE_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn transaction() -> MutexGuard<'static, ()> {
        BRIDGE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn normalized(mut parsed: Value) -> Value {
        // take_reports exposes hash-map iteration order. Compare all diagnostic
        // values/locations, without treating that private iteration as ordering.
        parsed["diagnostics"]
            .as_array_mut()
            .unwrap()
            .sort_by_key(Value::to_string);
        parsed
    }

    fn same_as_oracle(session: &mut ParserSession, path: &str, body: &str, header: &str) -> Value {
        let expected = normalized(parse(path, body.to_owned(), header));
        let actual = normalized(session.parse(path, body.to_owned(), header));
        assert_eq!(actual, expected, "{path}: {header:?}: {body:?}");
        actual
    }

    fn limited(entries: usize, key_bytes: usize, header_bytes: usize) -> ParserSession {
        ParserSession {
            entry_limit: entries,
            key_byte_limit: key_bytes,
            header_byte_limit: header_bytes,
            ..ParserSession::default()
        }
    }

    #[test]
    fn nested_parameter_tokens_keep_their_delimiters_and_literal_names_do_not_change() {
        let _transaction = transaction();
        let parsed=parse("macro.txt", "country_event={id=a immediate={fire_on_action={on_action=$ACTION$} set_country_flag=ACTION}}".into(), "");
        assert_eq!(
            parsed["ast"][0]["children"][1]["children"][0]["children"][0]["value"],
            "$ACTION$"
        );
        assert_eq!(
            parsed["ast"][0]["children"][1]["children"][1]["value"],
            "ACTION"
        );
    }
    #[test]
    fn bare_and_key_parameter_positions_remain_distinct_from_literals() {
        let _transaction = transaction();
        let parsed = parse(
            "macro.txt",
            "helper={list={$ITEM$ ITEM} $FIELD$=ITEM}".into(),
            "",
        );
        let body = &parsed["ast"][0]["children"];
        assert_eq!(body[0]["children"][0]["value"], "$ITEM$");
        assert_eq!(body[0]["children"][1]["value"], "ITEM");
        assert_eq!(body[1]["key"], "$FIELD$");
        assert_eq!(body[1]["value"], "ITEM");
    }

    #[test]
    fn reader_session_reuses_only_the_header_and_reparses_each_body() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        let first = same_as_oracle(&mut session, "same.txt", "first=@G", "@G=2\n");
        let second = same_as_oracle(&mut session, "same.txt", "second={value=@G}", "@G=2\n");
        assert_eq!(first["ast"][0]["key"], "first");
        assert_eq!(second["ast"][0]["key"], "second");
        assert_eq!(session.header_compilations, 1);
        assert_eq!(session.hits, 1);
        assert_eq!(session.headers.len(), 1);
    }

    #[test]
    fn reader_session_distinguishes_complete_header_bytes_in_a_b_a_sequence() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        for (header, expected) in [("@G=1\n", "1"), ("@G=2\n", "2"), ("@G=1\n", "1")] {
            let parsed = same_as_oracle(&mut session, "same.txt", "value=@G", header);
            assert_eq!(parsed["ast"][0]["value"], expected);
        }
        assert_eq!(session.header_compilations, 2);
        assert_eq!(session.hits, 1);
        // Whitespace is intentionally part of the key as well.
        same_as_oracle(&mut session, "same.txt", "value=@G", "@G = 1\n");
        assert_eq!(session.header_compilations, 3);
    }

    #[test]
    fn reader_session_keeps_different_paths_and_definition_token_locations_separate() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        for path in ["alpha.txt", "beta.txt", "alpha.txt"] {
            same_as_oracle(&mut session, path, "value=@G", "@G=2\n");
        }
        assert_eq!(session.header_compilations, 2);
        assert_eq!(session.hits, 1);
        for cached in &session.headers {
            let combined =
                crate::parse::pdxfile::memory::CombinedMemory::new(&cached.memory.pdxfile);
            let token = combined.get_variable("G").unwrap();
            assert_eq!(token.loc.fullpath(), std::path::Path::new(&cached.path));
            assert_eq!(token.loc.pathname(), std::path::Path::new(&cached.path));
            assert_eq!(token.loc.line, 1);
        }
    }

    #[test]
    fn reader_session_body_local_definitions_and_overrides_do_not_leak() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        let local = same_as_oracle(
            &mut session,
            "same.txt",
            "@G=8\n@BODY_ONLY=9\nvalue=@G",
            "@G=2\n",
        );
        assert_eq!(local["ast"][0]["value"], "8");
        let global = same_as_oracle(&mut session, "same.txt", "value=@G", "@G=2\n");
        assert_eq!(global["ast"][0]["value"], "2");
        let missing_local = same_as_oracle(&mut session, "same.txt", "value=@BODY_ONLY", "@G=2\n");
        assert!(!missing_local["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(session.header_compilations, 1);
        assert_eq!(session.hits, 2);
    }

    #[test]
    fn reader_session_rejects_ck3_reader_directives_without_caching_their_header() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        let header = "@G=2\n@:define insert_value={value=$N$ base=@G}\n";
        for value in ["3", "4"] {
            let body = format!("@:insert insert_value={{N={value}}}");
            let parsed = same_as_oracle(&mut session, "macro.txt", &body, header);
            // lexer.rs explicitly emits WrongGame for @: directives outside
            // CK3. The frozen a0c8dbe2 oracle reports one header error and one
            // body error; accepting their expansion is not a clean parse.
            let diagnostics = parsed["diagnostics"].as_array().unwrap();
            assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
            assert!(
                diagnostics.iter().all(|diagnostic| {
                    diagnostic["key"] == "wrong-game"
                        && diagnostic["severity"] == "error"
                        && diagnostic["message"] == "reader directives are only for CK3 so far"
                }),
                "{diagnostics:?}"
            );
            assert_eq!(parsed["ast"][0]["value"], value);
        }
        assert_eq!(session.header_compilations, 2);
        assert_eq!(session.hits, 0);
        assert!(session.headers.is_empty());
        assert_eq!(session.key_bytes, 0);
    }

    #[test]
    fn reader_session_preserves_stellaris_macro_blocks_and_local_reader_snapshots() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        let header = "@G=2\n";
        let first = same_as_oracle(
            &mut session,
            "macro.txt",
            "@G=8\nhelper={value=$FIRST$ base=@G}",
            header,
        );
        assert!(first["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(first["ast"][0]["macro_parameters"], json!(["FIRST"]));
        assert_eq!(first["ast"][0]["macro_variables"]["G"], "8");
        assert_eq!(first["ast"][0]["children"][1]["value"], "8");
        let second = same_as_oracle(
            &mut session,
            "macro.txt",
            "helper={value=$SECOND$ base=@G}",
            header,
        );
        assert!(second["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(second["ast"][0]["macro_parameters"], json!(["SECOND"]));
        assert!(
            second["ast"][0]["macro_variables"]
                .as_object()
                .unwrap()
                .is_empty()
        );
        assert_eq!(second["ast"][0]["children"][1]["value"], "2");
        assert_eq!(session.header_compilations, 1);
        assert_eq!(session.hits, 1);
    }

    #[test]
    fn reader_session_dirty_headers_repeat_their_diagnostics_and_are_never_admitted() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        for _ in 0..2 {
            let parsed = same_as_oracle(&mut session, "dirty.txt", "value=@G", "@G=@MISSING\n");
            assert!(!parsed["diagnostics"].as_array().unwrap().is_empty());
        }
        assert_eq!(session.header_compilations, 2);
        assert_eq!(session.hits, 0);
        assert!(session.headers.is_empty());
        assert_eq!(session.key_bytes, 0);
    }

    #[test]
    fn reader_session_clean_header_and_dirty_body_have_separate_cache_eligibility() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        let dirty = same_as_oracle(&mut session, "same.txt", "value=@MISSING", "@G=2\n");
        assert!(!dirty["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(session.headers.len(), 1);
        let clean = same_as_oracle(&mut session, "same.txt", "value=@G", "@G=2\n");
        assert!(clean["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(session.header_compilations, 1);
        assert_eq!(session.hits, 1);
    }

    #[test]
    fn reader_session_report_peek_never_drains_or_reorders_pending_header_reports() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        same_as_oracle(&mut session, "same.txt", "value=@G", "@G=2\n");
        let entry = file_entry("pending.txt");
        let mut pending_memory = ParserMemory::default();
        let _ = parse_reader_export(
            &entry,
            "@PENDING=@MISSING\n".into(),
            0,
            &mut pending_memory.pdxfile,
        );
        assert!(!crate::report::reports_are_empty());
        assert!(!crate::report::reports_are_empty());
        let parsed = session.parse("same.txt", "value=@G".into(), "@G=2\n");
        assert!(!parsed["diagnostics"].as_array().unwrap().is_empty());
        assert!(crate::report::reports_are_empty());
        assert_eq!(session.hits, 0);
        assert_eq!(session.header_compilations, 2);
        assert_eq!(session.headers.len(), 1);
    }

    #[test]
    fn reader_session_entry_limit_is_lru_and_hits_refresh_recency() {
        let _transaction = transaction();
        let mut session = ParserSession::default();
        for value in 1..=4 {
            same_as_oracle(
                &mut session,
                "same.txt",
                "value=@G",
                &format!("@G={value}\n"),
            );
        }
        same_as_oracle(&mut session, "same.txt", "value=@G", "@G=1\n");
        same_as_oracle(&mut session, "same.txt", "value=@G", "@G=5\n");
        assert_eq!(session.headers.len(), 4);
        assert!(
            !session
                .headers
                .iter()
                .any(|cached| cached.header == "@G=2\n")
        );
        assert!(
            session
                .headers
                .iter()
                .any(|cached| cached.header == "@G=1\n")
        );
        same_as_oracle(&mut session, "same.txt", "value=@G", "@G=2\n");
        assert_eq!(session.header_compilations, 6);
        assert_eq!(session.hits, 1);
    }

    #[test]
    fn reader_session_key_storage_budget_counts_utf8_path_and_header_bytes() {
        let _transaction = transaction();
        let header = "@G=1\n#界";
        let path = "变量.txt";
        let exact = path.len() + header.len();
        let mut exact_session = limited(4, exact, header.len());
        same_as_oracle(&mut exact_session, path, "value=@G", header);
        assert_eq!(exact_session.headers.len(), 1);
        assert_eq!(exact_session.key_bytes, exact);
        let mut too_small = limited(4, exact - 1, header.len());
        same_as_oracle(&mut too_small, path, "value=@G", header);
        assert!(too_small.headers.is_empty());
        assert_eq!(too_small.key_bytes, 0);
    }

    #[test]
    fn reader_session_total_key_budget_evicts_without_changing_parse_results() {
        let _transaction = transaction();
        let path = "same.txt";
        let header = "@G=1\n#abcdefghij";
        let one_key = path.len() + header.len();
        let mut session = limited(4, one_key * 2 - 1, 128);
        for header in [header, "@G=2\n#abcdefghij", header] {
            same_as_oracle(&mut session, path, "value=@G", header);
            assert_eq!(session.headers.len(), 1);
            assert!(session.key_bytes <= session.key_byte_limit);
        }
        assert_eq!(session.header_compilations, 3);
        assert_eq!(session.hits, 0);
    }

    #[test]
    fn reader_session_oversized_headers_are_uncached_without_evicting_valid_entries() {
        let _transaction = transaction();
        let mut session = limited(4, 128, "@G=1\n".len());
        same_as_oracle(&mut session, "same.txt", "value=@G", "@G=1\n");
        for _ in 0..2 {
            same_as_oracle(&mut session, "same.txt", "value=@G", "@G=2\n ");
        }
        same_as_oracle(&mut session, "same.txt", "value=@G", "@G=1\n");
        assert_eq!(session.headers.len(), 1);
        assert_eq!(session.header_compilations, 3);
        assert_eq!(session.hits, 1);
        let production = ParserSession::default();
        assert_eq!(production.entry_limit, 4);
        assert_eq!(production.key_byte_limit, 1024 * 1024);
        assert_eq!(production.header_byte_limit, 512 * 1024);
    }

    #[test]
    fn reader_session_failed_or_dirty_header_policy_never_manufactures_clean_admission() {
        // A parser Err can print stderr without storing a report. Its explicit
        // failure bit still forbids admission; it must execute again next time.
        assert!(!clean_header(true, false, true));
        assert!(!clean_header(false, true, true));
        assert!(!clean_header(true, true, false));
        assert!(clean_header(true, true, true));
    }

    #[test]
    fn reader_session_empty_headers_and_separate_sessions_do_not_share_cache_state() {
        let _transaction = transaction();
        let mut first = ParserSession::default();
        same_as_oracle(&mut first, "same.txt", "value=1", "");
        assert!(first.headers.is_empty());
        assert_eq!(first.header_compilations, 0);
        same_as_oracle(&mut first, "same.txt", "value=@G", "@G=2\n");
        let mut next_worker = ParserSession::default();
        same_as_oracle(&mut next_worker, "same.txt", "value=@G", "@G=2\n");
        assert_eq!(next_worker.hits, 0);
        assert_eq!(next_worker.header_compilations, 1);
    }
}
