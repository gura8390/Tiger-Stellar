use crate::ast::Parsed;
use serde_json::json;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct ParserClient {
    worker: Option<(Child, BufWriter<ChildStdin>, BufReader<ChildStdout>)>,
    count: usize,
    direct: bool,
    cache: Option<std::path::PathBuf>,
    identity: String,
    pub cache_hits: u64,
    pub parses: u64,
    pub worker_starts: u64,
}
impl Default for ParserClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserClient {
    pub fn new() -> Self {
        Self {
            worker: None,
            count: 0,
            direct: false,
            cache: None,
            identity: String::new(),
            cache_hits: 0,
            parses: 0,
            worker_starts: 0,
        }
    }
    pub fn with_cache(mut self, path: Option<std::path::PathBuf>) -> Result<Self, String> {
        if let Some(p) = &path {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            self.identity = crate::hash::sha256(
                &std::fs::read(std::env::current_exe().map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?,
            );
        }
        self.cache = path;
        Ok(self)
    }
    #[cfg(test)]
    pub fn direct() -> Self {
        let mut p = Self::new();
        p.direct = true;
        p
    }
    pub fn parse(&mut self, path: &str, text: &str, globals: &str) -> Result<Parsed, String> {
        let cache = self.cache.as_ref().map(|dir| {
            dir.join(format!(
                "{}.json",
                crate::hash::sha256(
                    format!("{}\0{path}\0{text}\0{globals}", self.identity).as_bytes()
                )
            ))
        });
        if let Some(cp) = &cache
            && let Ok(bytes) = std::fs::read(cp)
            && let Ok(envelope) = serde_json::from_slice::<serde_json::Value>(&bytes)
            && let (Some(payload), Some(hash)) =
                (envelope["payload"].as_str(), envelope["sha256"].as_str())
            && crate::hash::sha256(payload.as_bytes()) == hash
            && let Ok(p) = serde_json::from_str::<Parsed>(payload)
            && p.fatal.is_none()
        {
            self.cache_hits += 1;
            return Ok(p);
        }
        self.parses += 1;
        let parsed: Parsed = if self.direct {
            serde_json::from_value(stellaris_parser::stellaris_bridge::parse(
                path,
                text.into(),
                globals,
            ))
            .map_err(|e| e.to_string())?
        } else {
            if self.count >= 100 {
                self.stop()?;
            }
            if self.worker.is_none() {
                let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
                command
                    .arg("--parse-worker")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x08000000);
                }
                let mut child = command.spawn().map_err(|e| e.to_string())?;
                self.worker_starts += 1;
                let input = child.stdin.take().ok_or("missing parser input")?;
                let output = BufReader::new(child.stdout.take().ok_or("missing parser output")?);
                self.worker = Some((child, BufWriter::with_capacity(65536, input), output));
                self.count = 0;
            }
            let (_, input, output) = self.worker.as_mut().unwrap();
            serde_json::to_writer(
                &mut *input,
                &json!({"path":path,"content":text,"globals":globals}),
            )
            .map_err(|e| e.to_string())?;
            input
                .write_all(b"\n")
                .and_then(|_| input.flush())
                .map_err(|e| e.to_string())?;
            let mut line = String::new();
            if output.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("parser worker lost response".into());
            }
            self.count += 1;
            serde_json::from_str(&line).map_err(|e| format!("invalid parser response: {e}"))?
        };
        if let Some(e) = &parsed.fatal {
            return Err(e.clone());
        }
        if let Some(cp) = cache {
            let temp = cp.with_extension(format!("{}-{}.tmp", std::process::id(), self.parses));
            let payload = serde_json::to_string(&parsed).map_err(|e| e.to_string())?;
            let envelope =
                json!({"sha256":crate::hash::sha256(payload.as_bytes()),"payload":payload});
            std::fs::write(
                &temp,
                serde_json::to_vec(&envelope).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            if let Err(e) = std::fs::rename(&temp, &cp) {
                let _ = std::fs::remove_file(&temp);
                if !cp.exists() {
                    return Err(e.to_string());
                }
            }
        }
        Ok(parsed)
    }
    fn stop(&mut self) -> Result<(), String> {
        if let Some((mut child, input, output)) = self.worker.take() {
            drop(input);
            drop(output);
            let status = child.wait().map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!("parser worker failed: {status}"));
            }
        }
        Ok(())
    }
}
impl Drop for ParserClient {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
pub fn worker() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    // This cache dies with the existing 100-request worker. The free bridge
    // parser used by direct/oracle calls remains uncached.
    let mut session = stellaris_parser::stellaris_bridge::ParserSession::default();
    for line in stdin.lock().lines() {
        let result = (|| -> Result<serde_json::Value, String> {
            let req: serde_json::Value = serde_json::from_str(&line.map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            let path = req["path"].as_str().ok_or("missing path")?;
            let text = req["content"].as_str().ok_or("missing content")?;
            Ok(session.parse(path, text.into(), req["globals"].as_str().unwrap_or("")))
        })();
        let value = result.unwrap_or_else(|e| json!({"fatal":e}));
        serde_json::to_writer(&mut out, &value).map_err(|e| e.to_string())?;
        writeln!(out)
            .and_then(|_| out.flush())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
