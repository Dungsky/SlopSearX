//! Fail-closed native evidence runner. Owned check implementations are deliberately finite.
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
type Result<T> = std::result::Result<T, String>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: u32,
    plan_id: String,
    sources: Vec<Source>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    path: String,
    git_blob: String,
}
fn command(root: &Path, program: &str, args: &[&str], records: &mut Vec<Value>) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    records.push(json!({"program": program, "args": args, "exit_code": output.status.code(), "stdout": stdout, "stderr": String::from_utf8_lossy(&output.stderr)}));
    if !output.status.success() {
        return Err(format!("command failed: {program} {}", args.join(" ")));
    }
    Ok(stdout)
}
fn load_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))
}
fn validate_fixture(f: &Fixture, id: &str) -> Result<()> {
    if f.schema_version != 1 || f.plan_id != id || f.sources.is_empty() {
        return Err("invalid or empty fixture".into());
    }
    let mut seen = BTreeSet::new();
    for s in &f.sources {
        if s.path.is_empty()
            || Path::new(&s.path).is_absolute()
            || s.path.split('/').any(|p| p == "..")
            || !seen.insert(&s.path)
            || s.git_blob.len() != 40
            || !s.git_blob.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid/duplicate source fixture".into());
        }
    }
    Ok(())
}
fn source_snapshot(root: &Path, records: &mut Vec<Value>) -> Result<Vec<Value>> {
    let paths = command(
        root,
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        records,
    )?;
    let mut sources = Vec::new();
    for path in paths.split('\0').filter(|p| !p.is_empty()).filter(|p| {
        p.starts_with("crates/")
            || p.starts_with(".cargo/")
            || matches!(
                *p,
                "Cargo.toml"
                    | "Cargo.lock"
                    | "rust-toolchain.toml"
                    | "deny.toml"
                    | ".github/workflows/rust.yml"
                    | "scripts/rust-foundation-smoke.sh"
            )
            || p.starts_with("docs/rust-rewrite/")
            || p.starts_with("docs/adr/")
    }) {
        let hash = command(root, "git", &["hash-object", "--", path], records)?;
        sources.push(json!({"path":path,"git_blob":hash.trim()}));
    }
    if sources.is_empty() {
        return Err("empty source snapshot".into());
    }
    Ok(sources)
}
fn source_check(root: &Path, id: &str, records: &mut Vec<Value>) -> Result<()> {
    let path = root.join(format!("crates/xtask/fixtures/{id}.json"));
    let f: Fixture = serde_json::from_slice(
        &fs::read(&path).map_err(|e| format!("missing fixture {}: {e}", path.display()))?,
    )
    .map_err(|e| format!("malformed fixture: {e}"))?;
    validate_fixture(&f, id)?;
    let tracked = command(root, "git", &["ls-files", "-z"], records)?;
    let required: BTreeSet<_> = tracked
        .split('\0')
        .filter(|p| {
            if id == "F01" {
                p.starts_with("docs/rust-rewrite/inventory/")
                    || p.ends_with(".py")
                    || p.ends_with(".pyw")
                    || p.ends_with(".pyi")
                    || *p == "ssx"
            } else {
                p.starts_with("docs/rust-rewrite/architecture/")
                    || p.starts_with("docs/adr/003-")
                    || p.starts_with("docs/adr/004-")
                    || p.starts_with("docs/adr/005-")
            }
        })
        .map(str::to_string)
        .collect();
    let provided: BTreeSet<_> = f.sources.iter().map(|s| s.path.clone()).collect();
    if required.is_empty() || required != provided {
        return Err("source fixture coverage is incomplete or unexpected".into());
    }

    for s in &f.sources {
        let actual = command(root, "git", &["hash-object", "--", &s.path], records)?;
        if actual.trim() != s.git_blob {
            return Err(format!("stale source fixture: {}", s.path));
        }
    }
    if id == "F01" {
        let manifest =
            load_json(&root.join("docs/rust-rewrite/inventory/python-disposition.json"))?;
        let rows = manifest["files"]
            .as_array()
            .ok_or("missing Python dispositions")?;
        let mut paths = BTreeSet::new();
        for row in rows {
            let p = row["path"].as_str().ok_or("missing disposition path")?;
            if !paths.insert(p.to_string())
                || row["disposition"].as_str().is_none_or(str::is_empty)
                || row["classification"].as_str().is_none_or(str::is_empty)
            {
                return Err("duplicate or unowned disposition".into());
            }
        }
        let tracked = command(root, "git", &["ls-files", "-z"], records)?;
        let expected: BTreeSet<String> = tracked
            .split('\0')
            .filter(|p| {
                p.ends_with(".py") || p.ends_with(".pyi") || p.ends_with(".pyw") || *p == "ssx"
            })
            .map(str::to_string)
            .collect();
        if paths.is_empty() || paths != expected {
            return Err("Python source inventory differs from tracked source set".into());
        }
        let pinned: BTreeSet<_> = f.sources.iter().map(|s| s.path.clone()).collect();
        if !expected.is_subset(&pinned) {
            return Err("fixture omits inventoried Python source".into());
        }
    }
    if id == "F02" {
        // ADR acceptance is defined by merged F02, not its proposal-heading wording.
        command(
            root,
            "git",
            &[
                "merge-base",
                "--is-ancestor",
                "562b6ce1ce9f798104311e8f3a213daefeddc795",
                "HEAD",
            ],
            records,
        )?;
        let pins = load_json(&root.join("docs/rust-rewrite/architecture/pins.json"))?;
        if pins["toolchain"]["rust"] != "1.99.0"
            || pins["direct_dependencies"]
                .as_object()
                .is_none_or(|o| o.is_empty())
        {
            return Err("architecture pins absent or changed".into());
        }
    }
    Ok(())
}
fn readiness(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let binary = root.join("target/debug/ssx-server");
    let binary_path = binary.to_str().ok_or("non-UTF8 binary path")?;
    let binary_hash = command(root, "git", &["hash-object", "--", binary_path], records)?;
    let mut child = Command::new(&binary)
        .args(["--listen", "127.0.0.1:0"])
        .env("PATH", "")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("native readiness spawn: {e}"))?;
    // Read startup on another thread; the timeout also catches a binary that never becomes ready.
    let stdout = child.stdout.take().ok_or("server stdout missing")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
    });
    let result: Result<()> = (|| {
        let line = rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "readiness startup timed out")?
            .map_err(|e| e.to_string())?;
        let startup: Value =
            serde_json::from_str(&line).map_err(|e| format!("invalid startup: {e}"))?;
        if startup["mode"] != "foundation" {
            return Err("wrong server mode".into());
        }
        let address = startup["address"]
            .as_str()
            .ok_or("missing listener address")?;
        let addr: std::net::SocketAddr = address.parse().map_err(|_| "invalid listener address")?;
        if !addr.ip().is_loopback() || addr.port() == 0 {
            return Err("listener is not bound loopback".into());
        }
        let mut stream =
            TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .map_err(|e| e.to_string())?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|e| e.to_string())?;
        let (headers, body) = response
            .split_once("\r\n\r\n")
            .ok_or("invalid HTTP readiness response")?;
        let body: Value =
            serde_json::from_str(body).map_err(|e| format!("invalid readiness JSON: {e}"))?;
        if !headers.starts_with("HTTP/1.1 200 ")
            || body != json!({"status":"ok","mode":"foundation"})
        {
            return Err("readiness contract mismatch".into());
        }
        records.push(json!({"program":binary,"git_blob":binary_hash.trim(),"args":["--listen","127.0.0.1:0"],"environment":{"PATH":""},"startup":startup,"request":"GET /healthz","response":body,"status":"passed"}));
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    result?;
    let after_hash = command(root, "git", &["hash-object", "--", binary_path], records)?;
    if binary_hash != after_hash {
        return Err("readiness binary changed during smoke".into());
    }
    Ok(())
}
fn execute(root: &Path, id: &str, records: &mut Vec<Value>) -> Result<()> {
    let plan = load_json(&root.join("docs/rust-rewrite/plan.json"))?;
    let items = plan["items"].as_array().ok_or("malformed plan items")?;
    let mut ids = BTreeSet::new();
    for item in items {
        let item_id = item["id"].as_str().ok_or("malformed plan id")?;
        if item_id.is_empty() || !ids.insert(item_id) {
            return Err("empty/duplicate plan id".into());
        }
    }
    if !ids.contains(id) {
        return Err(format!("unknown plan id: {id}"));
    }
    match id {
        "F01" | "F02" => source_check(root, id, records),
        "F03" => {
            source_check(root, "F01", records)?;
            source_check(root, "F02", records)?;
            command(root, "cargo", &["fmt", "--all", "--check"], records)?;
            command(
                root,
                "cargo",
                &[
                    "clippy",
                    "--locked",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
                records,
            )?;
            // xtask unit tests are run by the outer workspace gate; excluding this package prevents recursive invocation.
            command(
                root,
                "cargo",
                &["test", "--locked", "--workspace", "--exclude", "xtask"],
                records,
            )?;
            command(
                root,
                "cargo",
                &["build", "--locked", "--workspace"],
                records,
            )?;
            readiness(root, records)
        }
        _ => Err(format!(
            "unimplemented acceptance checks for {id}; no owned executable fixtures"
        )),
    }
}
fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let started_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_millis();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut records = Vec::new();
    let id = args.get(1).map(String::as_str).unwrap_or("");
    let mut head_records = Vec::new();
    let head = command(&root, "git", &["rev-parse", "HEAD"], &mut head_records).unwrap_or_default();
    let tree = command(
        &root,
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
        &mut head_records,
    )
    .unwrap_or_default();
    let toolchain = command(
        &root,
        "rustc",
        &["--version", "--verbose"],
        &mut head_records,
    )
    .unwrap_or_default();
    let diff = command(
        &root,
        "git",
        &["diff", "--binary", "HEAD"],
        &mut head_records,
    )
    .unwrap_or_default();
    let snapshot = source_snapshot(&root, &mut head_records);
    let mut result = if args.len() != 2 && !(args.len() == 4 && args[2] == "--output")
        || args.first().map(String::as_str) != Some("check")
    {
        Err("usage: cargo xtask check <plan-id> [--output PATH]".into())
    } else {
        if head.is_empty() || toolchain.is_empty() || snapshot.is_err() {
            Err("source/toolchain identity unavailable".into())
        } else {
            execute(&root, id, &mut records)
        }
    };
    if result.is_ok() {
        match source_snapshot(&root, &mut head_records) {
            Ok(after) if snapshot.as_ref().is_ok_and(|before| *before == after) => {}
            _ => {
                result = Err(
                    "source changed while acceptance checks ran; rerun against stable source"
                        .into(),
                )
            }
        }
    }
    let mut evidence = json!
({"schema_version":1,"started_unix_ms":started_unix_ms,"finished_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("wall clock after epoch").as_millis(),"plan_id":id,"status":if result.is_ok(){"passed"}else{"failed"},"scope":"foundation checks only; no feature parity certification","source":{"head":head.trim(),"working_tree":tree,"diff":diff,"files":snapshot.unwrap_or_default()},"toolchain":toolchain,"identity_commands":head_records,"commands":records,"output_path":args.get(3),"error":result.err()});
    if args.len() == 4 && args[2] == "--output" {
        let path = PathBuf::from(&args[3]);
        let write = (|| -> std::io::Result<()> {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, serde_json::to_vec_pretty(&evidence)?)
        })();
        if let Err(e) = write {
            evidence["status"] = json!("failed");
            evidence["error"] = json!(format!("evidence output {}: {e}", path.display()));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).expect("serializable evidence")
    );
    if evidence["status"] != "passed" {
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_empty_wrong_and_duplicate_fixtures() {
        let mut f = Fixture {
            schema_version: 1,
            plan_id: "F01".into(),
            sources: vec![],
        };
        assert!(validate_fixture(&f, "F01").is_err());
        f.sources.push(Source {
            path: "foo".into(),
            git_blob: "0".repeat(40),
        });
        assert!(validate_fixture(&f, "F02").is_err());
        f.sources.push(Source {
            path: "foo".into(),
            git_blob: "0".repeat(40),
        });
        assert!(validate_fixture(&f, "F01").is_err());
    }
    #[test]
    fn rejects_escape_and_malformed() {
        let f = Fixture {
            schema_version: 1,
            plan_id: "F01".into(),
            sources: vec![Source {
                path: "../escape".into(),
                git_blob: "0".repeat(40),
            }],
        };
        assert!(validate_fixture(&f, "F01").is_err());
        assert!(serde_json::from_str::<Fixture>("{}").is_err());
    }
    #[test]
    fn unknown_and_unimplemented_fail() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(
            execute(&root, "F04", &mut vec![])
                .unwrap_err()
                .contains("unimplemented")
        );
        assert!(
            execute(&root, "INVALID", &mut vec![])
                .unwrap_err()
                .contains("unknown")
        );
    }
    #[test]
    fn missing_empty_malformed_stale_and_incomplete_fail() {
        let root = std::env::temp_dir().join(format!("ssx-xtask-negative-{}", std::process::id()));
        fs::create_dir_all(root.join("crates/xtask/fixtures")).unwrap();
        let fixture = root.join("crates/xtask/fixtures/F01.json");
        let mut records = vec![];
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("missing fixture")
        );
        fs::write(&fixture, "{}").unwrap();
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("malformed fixture")
        );
        fs::write(
            &fixture,
            r#"{"schema_version":1,"plan_id":"F01","sources":[]}"#,
        )
        .unwrap();
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("empty fixture")
        );
        command(&root, "git", &["init", "--quiet"], &mut records).unwrap();
        fs::write(root.join("example.py"), "# real source\n").unwrap();
        command(&root, "git", &["add", "example.py"], &mut records).unwrap();
        fs::write(&fixture,serde_json::to_vec(&json!({"schema_version":1,"plan_id":"F01","sources":[{"path":"example.py","git_blob":"0".repeat(40)}]})).unwrap()).unwrap();
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("stale source")
        );
        fs::write(&fixture,serde_json::to_vec(&json!({"schema_version":1,"plan_id":"F01","sources":[{"path":"other.py","git_blob":"0".repeat(40)}]})).unwrap()).unwrap();
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("coverage")
        );
        fs::create_dir_all(root.join("docs/rust-rewrite")).unwrap();
        fs::write(
            root.join("docs/rust-rewrite/plan.json"),
            r#"{"items":[{"id":"F01"},{"id":"F01"}]}"#,
        )
        .unwrap();
        assert!(
            execute(&root, "F01", &mut records)
                .unwrap_err()
                .contains("duplicate")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
