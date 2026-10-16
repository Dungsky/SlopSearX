//! Native, additive CI successors; never publishes reviews or executes target PR code.
use crate::{Result, command};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const HARNESS: &str = "groktopus/codereview";
const REVISION: &str = "04c7fc0b42221872e6c6b611be91c68e462fc679";
const PROTECTED_PINS: [(&str, &str); 4] = [
    (
        ".github/workflows/pr-publish.yml",
        "766b73be90a60adad843b71cf8995dbb8eb9c2c1",
    ),
    (
        ".github/workflows/pr-review-analysis.yml",
        "b2f2519d292e94b291280b323dfa2a366e66c444",
    ),
    (
        ".github/workflows/protected-source-context-probe.yml",
        "768063de420395c234bbbb5b87af47b33485c8d9",
    ),
    (
        ".github/pr-review-publisher-policy.json",
        "c8ecda28988b1bb0681a1ce31448a51ae7606c30",
    ),
];
fn bounded(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    ensure(bytes.len() <= 4 * 1024 * 1024, "CI input exceeds 4 MiB")?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
fn load_json(path: &Path) -> Result<Value> {
    serde_json::from_str(&bounded(path)?).map_err(|e| format!("{}: {e}", path.display()))
}
fn parse_yaml(raw: &str) -> Result<Value> {
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
        budget: serde_saphyr::budget! {max_events: 100_000, max_aliases: 0, max_anchors: 0, flow_nesting_limit: 64},
    };
    serde_saphyr::from_str_with_options(raw, options).map_err(|e| e.to_string())
}
fn ensure(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(reason.into()) }
}
fn yaml(path: &Path) -> Result<Value> {
    parse_yaml(&bounded(path)?).map_err(|e| format!("{}: {e}", path.display()))
}
fn object<'a>(v: &'a Value, label: &str) -> Result<&'a serde_json::Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| format!("{label} must be an object"))
}
pub fn inventory(root: &Path) -> Result<Value> {
    let mut gates = serde_json::Map::new();
    for entry in fs::read_dir(root.join(".github/workflows")).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !matches!(
            path.extension().and_then(|p| p.to_str()),
            Some("yml" | "yaml")
        ) {
            continue;
        }
        let w = yaml(&path)?;
        for (id, job) in object(&w["jobs"], "workflow jobs")? {
            ensure(
                job.get("uses").is_some() || job["steps"].as_array().is_some_and(|s| !s.is_empty()),
                "gate has no executable step",
            )?;
            gates.insert(
                format!(
                    "{}:{id}",
                    path.file_name()
                        .and_then(|p| p.to_str())
                        .ok_or("workflow name")?
                ),
                gate_source(&w, job)?,
            );
        }
    }
    Ok(Value::Object(gates))
}
fn gate_source(workflow: &Value, job: &Value) -> Result<Value> {
    let mut context = object(workflow, "workflow")?.clone();
    context.remove("jobs");
    Ok(json!({"workflow":context,"job":job}))
}
fn validate_gate_source(source: &Value, mapping: &Value) -> Result<()> {
    ensure(
        mapping["source"] == *source,
        "executable workflow/job/step controls differ from successor ledger",
    )
}
fn validate_policy(p: &Value) -> Result<()> {
    ensure(
        p["schema"] == "pr-review-protected-publication-policy.v5"
            && p["repository"]["name"] == "magnus919/SlopSearX"
            && p["repository"]["id"] == 1263452316_u64
            && p["default_branch"] == "main"
            && p["called_harness"]["repository"] == HARNESS
            && p["called_harness"]["sha"] == REVISION
            && p["called_harness"]["path"] == ".github/workflows/pr-analysis.yml"
            && p["credential"]["kind"] == "ACTIONS_TOKEN"
            && p["artifact_trust_mode"] == "API_BOUND_SHA256"
            && p["artifact_download_mode"] == "AUTHENTICATED_API_LOCATION_NO_AUTH",
        "protected policy identity/provenance mismatch",
    )
}
fn permissions(p: &Value, writer: bool) -> Result<()> {
    ensure(
        *p == json!({"actions":"read","contents":"read","pull-requests":if writer {"write"} else {"read"}}),
        "publisher permissions differ from accepted least privilege",
    )
}
fn protected(root: &Path) -> Result<()> {
    validate_policy(&load_json(
        &root.join(".github/pr-review-publisher-policy.json"),
    )?)?;
    let p = yaml(&root.join(".github/workflows/pr-publish.yml"))?;
    permissions(&p["permissions"], false)?;
    permissions(&p["jobs"]["publisher"]["permissions"], true)?;
    ensure(
        p["on"]["workflow_run"]["workflows"] == json!(["PR Review Analysis"])
            && p["on"]["workflow_run"]["types"] == json!(["completed"]),
        "publisher trigger mismatch",
    )?;
    let job = &p["jobs"]["publisher"];
    ensure(
        job["environment"]["name"] == "code-review-publication"
            && job["steps"][0]["with"]["ref"] == "${{ github.sha }}"
            && job["steps"][0]["with"]["persist-credentials"] == false,
        "protected source/environment boundary mismatch",
    )?;
    let pins = load_json(&root.join("docs/rust-rewrite/ci/protected-sources.json"))?;
    let expected: BTreeSet<_> = PROTECTED_PINS.iter().map(|(p, _)| *p).collect();
    ensure(
        object(&pins, "pins")?
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            == expected,
        "protected pin coverage incomplete",
    )?;
    for (path, pin) in object(&pins, "pins")? {
        let hash = command(root, "git", &["hash-object", "--", path], &mut vec![])?;
        let accepted = PROTECTED_PINS
            .iter()
            .find(|(p, _)| p == path)
            .ok_or("unaccepted protected path")?
            .1;
        ensure(
            pin.as_str() == Some(accepted) && hash.trim() == accepted,
            "protected source changed: trust review and repinning required",
        )?;
    }
    Ok(())
}
fn mirror(raw: &[u8]) -> Result<Value> {
    let mut v: Value = if raw.iter().all(u8::is_ascii_whitespace) {
        json!({})
    } else {
        serde_json::from_slice(raw).map_err(|e| e.to_string())?
    };
    let mirrors = v
        .as_object_mut()
        .ok_or("Docker config must be an object")?
        .entry("registry-mirrors")
        .or_insert(json!([]))
        .as_array_mut()
        .ok_or("mirrors must be an array")?;
    ensure(
        mirrors
            .iter()
            .all(|v| v.as_str().is_some_and(|s| !s.is_empty())),
        "invalid mirror entry",
    )?;
    if !mirrors.iter().any(|v| v == "https://mirror.gcr.io") {
        mirrors.push(json!("https://mirror.gcr.io"));
    }
    Ok(v)
}
fn digest(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn manifest(v: &Value) -> Result<()> {
    ensure(
        v["schemaVersion"] == 2
            && matches!(
                v["mediaType"].as_str(),
                Some(
                    "application/vnd.oci.image.index.v1+json"
                        | "application/vnd.docker.distribution.manifest.list.v2+json"
                )
            ),
        "expected OCI index",
    )?;
    let rows = v["manifests"].as_array().ok_or("missing manifests")?;
    for arch in ["amd64", "arm64"] {
        ensure(
            rows.iter()
                .filter(|m| m["platform"]["os"] == "linux" && m["platform"]["architecture"] == arch)
                .count()
                == 1,
            "missing/ambiguous Linux platform",
        )?;
    }
    ensure(
        rows.iter()
            .all(|m| m["digest"].as_str().is_some_and(digest)),
        "invalid immutable manifest digest",
    )
}
fn bootstrap(root: &Path, p: &Value, dest: &Path) -> Result<Value> {
    validate_policy(p)?;
    ensure(
        dest.is_absolute()
            && dest
                .file_name()
                .is_some_and(|s| s == "trusted-review-harness"),
        "destination must be absolute trusted-review-harness",
    )?;
    ensure(
        fs::symlink_metadata(dest).is_err(),
        "destination already exists",
    )?;
    let parent = dest
        .parent()
        .ok_or("destination parent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    ensure(
        !parent.starts_with(root.canonicalize().map_err(|e| e.to_string())?),
        "harness must be outside target source",
    )?;
    Ok(
        json!({"repository":HARNESS,"sha":REVISION,"destination":parent.join("trusted-review-harness"),"execute_target_code":false}),
    )
}
struct GitGroup(u32);
impl Drop for GitGroup {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{}", self.0)])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &self.0.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}
fn git_bounded(program: &Path, cwd: &Path, args: &[&str], seconds: u64) -> Result<String> {
    let mut process = Command::new(program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        process.process_group(0);
    }
    process
        .current_dir(cwd)
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = process.spawn().map_err(|e| e.to_string())?;
    let _group = GitGroup(child.id());
    let stdout = child.stdout.take().ok_or("git stdout")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(8193)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|e| e.to_string());
        let _ = tx.send(result);
    });
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            ensure(status.success(), "trusted git command failed")?;
            let bytes = rx
                .recv_timeout(Duration::from_secs(seconds).saturating_sub(start.elapsed()))
                .map_err(|_| "trusted git output timed out")??;
            ensure(bytes.len() <= 8192, "git output too large")?;
            return String::from_utf8(bytes).map_err(|e| e.to_string());
        }
        if start.elapsed() >= Duration::from_secs(seconds) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("trusted git command timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn checkout_with(
    root: &Path,
    p: &Value,
    dest: &Path,
    mut run: impl FnMut(&Path, &[&str], u64) -> Result<String>,
) -> Result<()> {
    let plan = bootstrap(root, p, dest)?;
    let destination = plan["destination"].as_str().ok_or("destination encoding")?;
    let parent = Path::new(destination)
        .parent()
        .ok_or("destination parent")?;
    run(
        parent,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "credential.helper=",
            "clone",
            "--quiet",
            "--no-checkout",
            "--filter=blob:none",
            &format!("https://github.com/{HARNESS}.git"),
            destination,
        ],
        120,
    )?;
    run(
        Path::new(destination),
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "checkout",
            "--quiet",
            "--detach",
            REVISION,
        ],
        30,
    )?;
    ensure(
        run(Path::new(destination), &["rev-parse", "HEAD"], 10)?.trim() == REVISION,
        "trusted harness revision mismatch",
    )
}
fn hygiene_text(path: &str, raw: &str) -> Result<()> {
    use sha2::{Digest as _, Sha256};
    // Exact durable output intentionally has no text newline. Keep JSON and
    // all other hygiene validation; no other path/bytes receive this exception.
    let durable_fixture = path == "crates/xtask/fixtures/f13/native-record.json"
        && Sha256::digest(raw.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            == "fbf5c2b453e4c4e4fdd54b5c372be642d036e5cf2db788de0df963e24e0ba203";
    ensure(!raw.contains('\r'), "mixed/CR line endings")?;
    ensure(
        durable_fixture || raw.is_empty() || (raw.ends_with('\n') && !raw.ends_with("\n\n")),
        "missing or repeated final newline",
    )?;
    for line in raw.lines() {
        ensure(
            line.trim_end_matches([' ', '\t']) == line,
            "trailing whitespace",
        )?;
        ensure(
            !["<<<<<<< ", "=======", ">>>>>>> "]
                .iter()
                .any(|m| line.starts_with(m)),
            "merge conflict marker",
        )?;
    }
    if path.ends_with(".json") {
        let _: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    }
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        parse_yaml(raw)?;
    }
    Ok(())
}
fn hygiene(root: &Path) -> Result<()> {
    let files = command(
        root,
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        &mut vec![],
    )?;
    for file in files.split('\0').filter(|p| !p.is_empty()).filter(|p| {
        p.starts_with("crates/")
            || p.starts_with(".github/workflows/")
            || p.starts_with("docs/rust-rewrite/ci/")
            || matches!(
                *p,
                "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml" | "deny.toml"
            )
    }) {
        let raw = bounded(&root.join(file))?;
        ensure(
            raw.len() <= 500 * 1024,
            "new native gate source exceeds 500 KiB",
        )?;
        hygiene_text(file, &raw).map_err(|e| format!("{file}: {e}"))?;
    }
    Ok(())
}
pub fn helper(root: &Path, args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("source-hygiene") if args.len()==1 => hygiene(root),
        Some("review-bootstrap") if args.len()==2 => checkout_with(root,&load_json(&root.join(".github/pr-review-publisher-policy.json"))?,Path::new(&args[1]),|cwd,args,seconds|git_bounded(Path::new("git"),cwd,args,seconds)),
        Some("inventory") if args.len()==1 => {println!("{}",serde_json::to_string_pretty(&inventory(root)?).map_err(|e|e.to_string())?);Ok(())}
        Some("mirror-config") if args.len()==2 => {println!("{}",serde_json::to_string_pretty(&mirror(&fs::read(&args[1]).map_err(|e|e.to_string())?)?).map_err(|e|e.to_string())?);Ok(())}
        Some("manifest") if args.len()==2 => manifest(&load_json(Path::new(&args[1]))?),
        Some("review-bootstrap-plan") if args.len()==2 => {println!("{}",bootstrap(root,&load_json(&root.join(".github/pr-review-publisher-policy.json"))?,Path::new(&args[1]))?);Ok(())}
        _ => Err("usage: cargo xtask ci-helper {inventory|source-hygiene|mirror-config FILE|manifest FILE|review-bootstrap-plan ABS_DEST|review-bootstrap ABS_DEST}".into())
    }
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    hygiene(root)?;
    let actual = inventory(root)?;
    let ledger = load_json(&root.join("docs/rust-rewrite/ci/gates.json"))?;
    let gates = object(&ledger["gates"], "gate ledger")?;
    ensure(
        object(&actual, "actual gates")?
            .keys()
            .collect::<BTreeSet<_>>()
            == gates.keys().collect::<BTreeSet<_>>(),
        "workflow gate coverage differs",
    )?;
    for (id, source) in object(&actual, "actual gates")? {
        let g = &gates[id];
        validate_gate_source(source, g).map_err(|e| format!("{id}: {e}"))?;
        ensure(
            g["successor"].as_str().is_some_and(|s| !s.is_empty())
                && g["owner"].as_str().is_some_and(|s| !s.is_empty())
                && matches!(
                    g["state"].as_str(),
                    Some("additive" | "pending-owner" | "retained-external")
                ),
            "missing successor decision",
        )?;
    }
    ensure(
        ledger["precommit_source"] == yaml(&root.join(".pre-commit-config.yaml"))?,
        "precommit drift",
    )?;
    let hooks = yaml(&root.join(".pre-commit-config.yaml"))?;
    let mut actual_hooks = BTreeSet::new();
    for repo in hooks["repos"].as_array().ok_or("missing hook repos")? {
        for hook in repo["hooks"].as_array().ok_or("missing hooks")? {
            actual_hooks.insert(hook["id"].as_str().ok_or("missing hook id")?);
        }
    }
    let mapped = object(&ledger["precommit_successors"], "hook successors")?;
    ensure(
        actual_hooks == mapped.keys().map(String::as_str).collect(),
        "incomplete hook successor coverage",
    )?;
    ensure(
        mapped
            .values()
            .all(|v| v.as_str().is_some_and(|s| !s.is_empty())),
        "empty hook successor",
    )?;
    ensure(
        ledger["pr_template_source"].as_str()
            == Some(
                fs::read_to_string(root.join(".github/PULL_REQUEST_TEMPLATE.md"))
                    .map_err(|e| e.to_string())?
                    .as_str(),
            ),
        "PR template drift",
    )?;
    protected(root)?;
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "xtask", "o06::tests::"],
        records,
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_ledger_rejects_failure_weakening_and_all_execution_control_drift() {
        let original = json!({"on":{"pull_request":{}},"permissions":{"contents":"read"},"concurrency":{"group":"gate"},"jobs":{"test":{"runs-on":"ubuntu-24.04","needs":["build"],"strategy":{"fail-fast":true},"timeout-minutes":10,"environment":"test","steps":[{"run":"cargo test --locked","timeout-minutes":5}]}}});
        let source = gate_source(&original, &original["jobs"]["test"]).unwrap();
        let mapping = json!({"source":source});
        validate_gate_source(&source, &mapping).unwrap();
        for (pointer, value) in [
            ("/jobs/test/continue-on-error", json!(true)),
            ("/jobs/test/steps/0/continue-on-error", json!(true)),
            ("/jobs/test/steps/0/timeout-minutes", json!(1)),
            ("/jobs/test/timeout-minutes", json!(1)),
            ("/jobs/test/needs", json!([])),
            ("/jobs/test/strategy/fail-fast", json!(false)),
            ("/jobs/test/environment", json!("unprotected")),
            ("/permissions/contents", json!("write")),
            ("/on", json!({"workflow_dispatch":{}})),
            ("/concurrency/group", json!("other")),
        ] {
            let mut changed = original.clone();
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            changed
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), value);
            let changed_source = gate_source(&changed, &changed["jobs"]["test"]).unwrap();
            assert!(
                validate_gate_source(&changed_source, &mapping).is_err(),
                "accepted drift at {pointer}"
            );
        }
    }
    #[test]
    #[cfg(unix)]
    fn subprocess_environment_failure_output_and_deadline_are_bounded() {
        let cwd = std::env::temp_dir();
        let output=git_bounded(Path::new("/bin/sh"),&cwd,&["-c","printf '%s/%s/%s' \"${GITHUB_TOKEN-unset}\" \"$GIT_CONFIG_GLOBAL\" \"$GIT_TERMINAL_PROMPT\""],2).unwrap();
        assert_eq!(output, "unset//dev/null/0");
        assert!(git_bounded(Path::new("/bin/sh"), &cwd, &["-c", "exit 3"], 2).is_err());
        let start = Instant::now();
        assert!(git_bounded(Path::new("/bin/sh"), &cwd, &["-c", "exec sleep 30"], 1).is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(
            git_bounded(
                Path::new("/bin/sh"),
                &cwd,
                &["-c", "head -c 9000 /dev/zero"],
                2
            )
            .is_err()
        );
    }
    #[test]
    fn yaml_and_hygiene_fail_closed_on_ambiguous_or_malformed_sources() {
        for bad in [
            "permissions: read\npermissions: write\n",
            "x: &a [1]\ny: *a\n",
            "a: [\n",
        ] {
            assert!(parse_yaml(bad).is_err());
        }
        for bad in [
            "value \n",
            "value\r\n",
            "no final newline",
            "value\n\n",
            "<<<<<<< branch\n",
            "=======\n",
        ] {
            assert!(hygiene_text("source.rs", bad).is_err());
        }
        hygiene_text("source.rs", "fn main() {}\n").unwrap();
        let fixture = include_str!("../fixtures/f13/native-record.json");
        let path = "crates/xtask/fixtures/f13/native-record.json";
        hygiene_text(path, fixture).unwrap();
        assert!(hygiene_text("other.json", fixture).is_err());
        assert!(hygiene_text(path, &fixture.replace("Title0", "changed")).is_err());
        assert!(hygiene_text(path, "{").is_err());
        assert!(hygiene_text("source.json", "{\n").is_err());
    }
    #[test]
    fn bootstrap_executes_only_immutable_checkout_and_checks_actual_head() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let p = load_json(&root.join(".github/pr-review-publisher-policy.json")).unwrap();
        let temp = std::env::temp_dir().join(format!("o06-bootstrap-{}", std::process::id()));
        fs::create_dir_all(&temp).unwrap();
        let dest = temp.join("trusted-review-harness");
        let mut calls = Vec::new();
        checkout_with(&root, &p, &dest, |cwd, args, seconds| {
            calls.push((
                cwd.to_path_buf(),
                args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                seconds,
            ));
            Ok(if args == ["rev-parse", "HEAD"] {
                format!("{REVISION}\n")
            } else {
                String::new()
            })
        })
        .unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].2, 120);
        assert_eq!(calls[1].2, 30);
        assert_eq!(calls[2].2, 10);
        assert!(
            calls[0]
                .1
                .contains(&format!("https://github.com/{HARNESS}.git"))
        );
        assert!(calls[0].1.contains(&"--no-checkout".to_owned()));
        assert!(calls[1].1.contains(&REVISION.to_owned()));
        assert!(checkout_with(&root, &p, &dest, |_, _, _| Ok("attacker revision".into())).is_err());
        assert!(checkout_with(&root, &p, &dest, |_, _, _| Err("clone failed".into())).is_err());
        fs::create_dir_all(&dest).unwrap();
        assert!(bootstrap(&root, &p, &dest).is_err());
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn protected_workflow_mutation_cannot_self_authorize_by_repinning() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp = std::env::temp_dir().join(format!("o06-trust-{}", std::process::id()));
        fs::create_dir_all(temp.join(".github/workflows")).unwrap();
        fs::create_dir_all(temp.join("docs/rust-rewrite/ci")).unwrap();
        for (path, _) in PROTECTED_PINS {
            fs::copy(root.join(path), temp.join(path)).unwrap();
        }
        let pins = load_json(&root.join("docs/rust-rewrite/ci/protected-sources.json")).unwrap();
        fs::write(
            temp.join("docs/rust-rewrite/ci/protected-sources.json"),
            serde_json::to_vec(&pins).unwrap(),
        )
        .unwrap();
        protected(&temp).unwrap();
        let path = ".github/workflows/pr-publish.yml";
        let original = fs::read_to_string(root.join(path)).unwrap();
        for (before, after) in [
            (
                "github.event.workflow_run.event == 'pull_request_target'",
                "true",
            ),
            (
                "ref: ${{ github.sha }}",
                "ref: ${{ github.event.workflow_run.head_sha }}",
            ),
            ("pull-requests: write", "contents: write"),
            (REVISION, "main"),
            ("persist-credentials: false", "persist-credentials: true"),
        ] {
            let changed = original.replace(before, after);
            assert_ne!(changed, original);
            fs::write(temp.join(path), changed).unwrap();
            let new_hash =
                command(&temp, "git", &["hash-object", "--", path], &mut vec![]).unwrap();
            let mut repinned = pins.clone();
            repinned[path] = json!(new_hash.trim());
            fs::write(
                temp.join("docs/rust-rewrite/ci/protected-sources.json"),
                serde_json::to_vec(&repinned).unwrap(),
            )
            .unwrap();
            assert!(protected(&temp).is_err(), "self-repin allowed {before}");
        }
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn mirror_preserves_and_is_idempotent() {
        let v = mirror(br#"{"debug":true,"registry-mirrors":["https://private"]}"#).unwrap();
        assert_eq!(v["debug"], true);
        assert_eq!(
            v["registry-mirrors"],
            json!(["https://private", "https://mirror.gcr.io"])
        );
        assert_eq!(mirror(&serde_json::to_vec(&v).unwrap()).unwrap(), v);
        for bad in [
            "[]",
            "null",
            "{",
            r#"{"registry-mirrors":"url"}"#,
            r#"{"registry-mirrors":[null]}"#,
        ] {
            assert!(mirror(bad.as_bytes()).is_err());
        }
    }
    #[test]
    fn manifest_rejects_missing_duplicate_and_mutable_platforms() {
        let d = format!("sha256:{}", "a".repeat(64));
        let good = json!({"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{"digest":d,"platform":{"os":"linux","architecture":"amd64"}},{"digest":d,"platform":{"os":"linux","architecture":"arm64"}}]});
        manifest(&good).unwrap();
        for pointer in [
            "/manifests/0/digest",
            "/manifests/1/platform/architecture",
            "/mediaType",
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = json!("amd64");
            assert!(manifest(&bad).is_err());
        }
        assert!(!digest("sha256:abc"));
        assert!(!digest(&format!("sha256:{}", "A".repeat(64))));
    }
    #[test]
    fn permissions_cannot_expand() {
        permissions(
            &json!({"actions":"read","contents":"read","pull-requests":"write"}),
            true,
        )
        .unwrap();
        for bad in [
            json!("write-all"),
            json!({"actions":"write","contents":"read","pull-requests":"write"}),
            json!({"actions":"read","contents":"write","pull-requests":"write"}),
            json!({"actions":"read","contents":"read","pull-requests":"write","id-token":"write"}),
        ] {
            assert!(permissions(&bad, true).is_err());
        }
    }
    #[test]
    fn bootstrap_rejects_untrusted_identity_and_target_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let p = load_json(&root.join(".github/pr-review-publisher-policy.json")).unwrap();
        validate_policy(&p).unwrap();
        for (pointer, value) in [
            ("/called_harness/sha", json!("main")),
            ("/called_harness/repository", json!("evil/review")),
            ("/repository/id", json!(1)),
            ("/credential/kind", json!("APP_TOKEN")),
            ("/artifact_trust_mode", json!("UNVERIFIED")),
        ] {
            let mut bad = p.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(validate_policy(&bad).is_err());
        }
        assert!(bootstrap(&root, &p, &root.join("trusted-review-harness")).is_err());
        assert!(bootstrap(&root, &p, Path::new("relative/trusted-review-harness")).is_err());
    }
}
