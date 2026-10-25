use serde_json::json;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use tiger_stellar::{
    ast::Node,
    engine::Engine,
    hash::sha256,
    loader::{Program, read},
    parser::{ParserClient, worker},
    report::Diagnostics,
    rules::{Declarations, Rules},
};

const RULES: &str = include_str!("../../../rules/stellaris-4.5.2.json");
const DECLARATIONS: &str = include_str!("../../../rules/declarations-4.5.2.json");
const TOOL_VERSION: &str = "0.1.9tmp";
#[derive(Default)]
struct Options {
    game: PathBuf,
    mod_path: Option<PathBuf>,
    dependencies: Vec<PathBuf>,
    output: PathBuf,
    rules: Option<PathBuf>,
    language: Option<String>,
    cache: Option<PathBuf>,
    syntax_only: bool,
}
fn options(args: &[String]) -> Result<Options, String> {
    let mut o = Options::default();
    let mut i = 0;
    while i < args.len() {
        let key = &args[i];
        i += 1;
        if key == "--syntax-only" {
            o.syntax_only = true;
            continue;
        }
        if !matches!(
            key.as_str(),
            "--game" | "--mod" | "--dependency" | "--output" | "--rules" | "--language" | "--cache"
        ) {
            return Err(format!(
                "unsupported argument {key}; no Python fallback is available"
            ));
        }
        let val = args
            .get(i)
            .ok_or_else(|| format!("missing value for {key}"))?;
        i += 1;
        match key.as_str() {
            "--game" => o.game = val.into(),
            "--mod" => o.mod_path = Some(val.into()),
            "--dependency" => o.dependencies.push(val.into()),
            "--output" => o.output = val.into(),
            "--rules" => o.rules = Some(val.into()),
            "--language" => o.language = Some(val.clone()),
            "--cache" => o.cache = Some(val.into()),
            _ => unreachable!(),
        }
    }
    if o.game.as_os_str().is_empty() || o.output.as_os_str().is_empty() {
        return Err("--game and --output are required".into());
    }
    if o.mod_path.is_none() && !o.dependencies.is_empty() {
        return Err("--dependency requires an explicit --mod check target".into());
    }
    Ok(o)
}
fn save<T: serde::Serialize + ?Sized>(path: &std::path::Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut out = std::io::BufWriter::with_capacity(
        65536,
        std::fs::File::create(path).map_err(|e| e.to_string())?,
    );
    serde_json::to_writer_pretty(&mut out, value).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())
}
#[derive(serde::Serialize)]
struct Report<'a> {
    #[serde(flatten)]
    metadata: serde_json::Value,
    diagnostics: &'a [tiger_stellar::report::Diagnostic],
    warning_summary: std::collections::BTreeMap<String, usize>,
    checked_statements: &'a tiger_stellar::engine::Metrics,
    migration_gaps: &'a std::collections::BTreeSet<String>,
    phase_seconds: &'a std::collections::BTreeMap<String, f64>,
    ship_feasibility: &'a [tiger_stellar::ship_design::Feasibility],
    #[serde(rename = "component_potential_proofs")]
    potential_proofs: &'a [tiger_stellar::potential_proof::RequiredSetProof],
    inferred_script_inputs: &'a [serde_json::Value],
    engine_entries: &'a [serde_json::Value],
    saved_target_type_evidence: Vec<serde_json::Value>,
    input_manifest: &'a [tiger_stellar::loader::Input],
    rules_provenance: &'a serde_json::Value,
}
fn run(o: Options) -> Result<i32, String> {
    let start = Instant::now();
    let rules_text = if let Some(p) = &o.rules {
        read(p)?
    } else {
        RULES.into()
    };
    let rules: Rules =
        serde_json::from_str(&rules_text).map_err(|e| format!("invalid rules: {e}"))?;
    rules.validate()?;
    let declarations: Declarations =
        serde_json::from_str(DECLARATIONS).map_err(|e| e.to_string())?;
    if declarations.game_id != "stellaris" {
        return Err("wrong-game declaration rules".into());
    }
    let launcher = o.game.join("launcher-settings.json");
    let settings: serde_json::Value =
        serde_json::from_str(&read(&launcher)?).map_err(|e| e.to_string())?;
    if settings["gameId"].as_str() != Some("stellaris") {
        return Err("wrong-game: Tiger Stellar accepts only Stellaris".into());
    }
    let version = settings["rawVersion"]
        .as_str()
        .ok_or("missing game version")?
        .to_string();
    let mut diagnostics = Diagnostics::default();
    if version != rules.game_version {
        let n = Node {
            source: o.game.to_string_lossy().into_owned().into(),
            ..Default::default()
        };
        diagnostics.emit(
            &n,
            "version-mismatch",
            format!(
                "rules are {}, installed game is {version}; validation continues",
                rules.game_version
            ),
            ("warning", "input-configuration"),
            None,
            &[],
        );
    }
    let mut mods = o.dependencies.clone();
    if let Some(p) = &o.mod_path {
        mods.push(p.clone());
    }
    let mut parser = ParserClient::new().with_cache(o.cache.clone())?;
    let load_started = Instant::now();
    let mut program = Program::load(&o.game, &mods, &mut parser, &mut diagnostics)?;
    program.record(&launcher, "launcher-settings.json", 0)?;
    let syntax_files = program.files.len();
    let targets = program.target_files;
    let input_state = if targets == 0 {
        "no-script-targets"
    } else {
        "script-targets"
    };
    if targets == 0 {
        let n = Node {
            source: o
                .mod_path
                .as_ref()
                .unwrap_or(&o.game)
                .to_string_lossy()
                .into_owned()
                .into(),
            ..Default::default()
        };
        diagnostics.emit(
            &n,
            "empty-target",
            "No script target files found; this is not semantic validation evidence".into(),
            ("warning", "input-configuration"),
            None,
            &[],
        );
    }
    let mut engine = Engine::new(rules, declarations, program, parser, diagnostics);
    engine.phase_seconds.insert(
        "input_load_parse".into(),
        load_started.elapsed().as_secs_f64(),
    );
    engine.language = o.language.clone();
    if !o.syntax_only {
        engine.analyze();
    }
    let (errors, warnings) = engine.diagnostics.counts();
    let implementation = sha256(
        &std::fs::read(std::env::current_exe().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
    );
    let input_hash =
        sha256(&serde_json::to_vec(&engine.program.inputs).map_err(|e| e.to_string())?);
    let status = if targets == 0 {
        "empty_input_not_validation"
    } else if errors > 0 {
        "errors"
    } else if !engine.migration_gaps.is_empty() {
        "migration_incomplete"
    } else if warnings > 0 {
        "warnings_require_review"
    } else {
        "static_checks_pass"
    };
    let metadata = json!({"tool":"Tiger Stellar","tool_version":TOOL_VERSION,"crate_version":env!("CARGO_PKG_VERSION"),"temporary_prerelease":true,"development_track":"0.2-rework","runtime":"native Rust; no Python semantic runtime","game_version":version,
        "status":status,"input_state":input_state,"full_dsl_validated":false,"full_018_contract_parity":false,"engine_not_run":true,"release_allowed":false,
        "validation_mode":if o.syntax_only{"syntax-only"}else{"Rust bounded semantic core"},"analysis_completed_within_budget":engine.complete,
        "errors":errors,"warnings":warnings,"syntax_files":syntax_files,"target_files":targets,"elapsed_seconds":start.elapsed().as_secs_f64(),
        "rules_sha256":sha256(rules_text.as_bytes()),"declarations_sha256":sha256(DECLARATIONS.as_bytes()),
        "binding":{"implementation":{"binary_sha256":implementation,"source_sha256":env!("TIGER_STELLAR_SOURCE_SHA256")},"input_set_sha256":input_hash,"dependencies":o.dependencies,"target":o.mod_path,"selected_language":o.language},
        "parser":{"ownership_policy":"same Rust executable; isolated parser worker recycled every 100 uncached requests","cache_hits":engine.parser.cache_hits,"uncached_parses":engine.parser.parses,"worker_starts":engine.parser.worker_starts},
        "algorithm":{"bindings":"factored per-target type/availability domain; no binding path enumeration","event_dispatch":"changed-input worklist","script_summaries":"direct transfer; SCC summaries not yet migrated","diagnostic_policy":"raw severity retained; unknown capability is not validation success"}});
    let report = Report {
        metadata,
        diagnostics: &engine.diagnostics.rows,
        warning_summary: engine.diagnostics.reasons(),
        checked_statements: &engine.metrics,
        migration_gaps: &engine.migration_gaps,
        phase_seconds: &engine.phase_seconds,
        ship_feasibility: &engine.ship_feasibility,
        potential_proofs: &engine.potential_proofs,
        inferred_script_inputs: &engine.inferred_script_inputs,
        engine_entries: &engine.engine_entries,
        saved_target_type_evidence: engine.saved_target_type_evidence(),
        input_manifest: &engine.program.inputs,
        rules_provenance: &engine.rules.sources,
    };
    save(&o.output, &report)?;
    println!(
        "{}",
        json!({"tool":"Tiger Stellar","tool_version":TOOL_VERSION,"errors":errors,"warnings":warnings,"target_files":targets,"status":status,"elapsed_seconds":start.elapsed().as_secs_f64()})
    );
    Ok(if errors > 0 {
        1
    } else if warnings > 0 || targets == 0 {
        2
    } else {
        0
    })
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--parse-worker"] {
        if let Err(e) = worker() {
            eprintln!("{e}");
            std::process::exit(3);
        }
        return;
    }
    if args == ["--version"] {
        println!(
            "Tiger Stellar {} (native Rust temporary prerelease)",
            TOOL_VERSION
        );
        return;
    }
    if args == ["--help"] {
        println!(
            "Tiger Stellar native Rust checker\n--game PATH --mod PATH --dependency PATH --output JSON --cache PATH --rules FILE --language LANGUAGE --syntax-only\nTemporary prerelease; semantic coverage remains incomplete. Review coverage warnings and verify behavior in the game."
        );
        return;
    }
    let output = args
        .windows(2)
        .find(|pair| pair[0] == "--output")
        .map(|pair| PathBuf::from(&pair[1]));
    match options(&args).and_then(run) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            if let Some(p) = output {
                let _ = save(
                    &p,
                    &json!({"tool":"Tiger Stellar","status":"infrastructure_error","errors":1,"full_dsl_validated":false,"message":e}),
                );
            }
            eprintln!("{e}");
            std::process::exit(3);
        }
    }
}
