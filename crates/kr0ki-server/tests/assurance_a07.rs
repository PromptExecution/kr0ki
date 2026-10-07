//! KR-A07 acceptance case (VC-A07).
//!
//! "The workload sandbox shall deny writes outside its declared writable mounts."
//!
//! Acceptance: permit temporary output; deny a write to the mounted requirement baseline.
//!
//! The sandbox is a *deployment component* (`deploy/sandbox/run-sandboxed.sh`); kr0ki does not
//! enforce it. This case is kr0ki's evidence that the component does what the requirement says,
//! exercised for real: the script runs commands under bubblewrap and the host's baseline file is
//! digested before and after. Needs `bwrap`; without it the case fails rather than passing on
//! nothing, because a case that could not run is not evidence.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn baseline_dir() -> PathBuf {
    repo_root().join("docs/assurance")
}

fn baseline_file() -> PathBuf {
    baseline_dir().join("kr0ki.assurance.toml")
}

fn digest(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

/// Run `command` in the sandbox with the repository's baseline mounted; extra args go before `--`.
fn sandboxed(extra: &[&str], command: &[&str]) -> Output {
    assert!(
        Command::new("bwrap").arg("--version").output().is_ok(),
        "bubblewrap (bwrap) is required to verify KR-A07: this case cannot be verified without it"
    );
    let mut cmd = Command::new(repo_root().join("deploy/sandbox/run-sandboxed.sh"));
    cmd.arg("--baseline")
        .arg(baseline_dir())
        .args(extra)
        .arg("--")
        .args(command);
    cmd.env("KR0KI_SECRET_PROBE", "leaked-from-host");
    cmd.output().expect("the sandbox script must run")
}

fn text(o: &[u8]) -> String {
    String::from_utf8_lossy(o).into_owned()
}

#[test]
fn temporary_output_is_permitted() {
    let o = sandboxed(
        &[],
        &[
            "sh",
            "-c",
            "echo scratch-ok > /scratch/out.txt && cat /scratch/out.txt",
        ],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(text(&o.stdout).trim(), "scratch-ok");
}

#[test]
fn a_write_to_the_mounted_requirement_baseline_is_denied_and_the_host_file_is_untouched() {
    let before = digest(&baseline_file());
    for cmd in [
        "echo tampered >> /baseline/kr0ki.assurance.toml",
        "echo tampered > /baseline/kr0ki.assurance.toml",
        "echo x > /baseline/new-file",
        "rm /baseline/kr0ki.assurance.toml",
        "mv /baseline/kr0ki.assurance.toml /scratch/stolen",
        "echo x > /baseline/../escaped",
    ] {
        let o = sandboxed(&[], &["sh", "-c", cmd]);
        assert!(
            !o.status.success(),
            "`{cmd}` must fail; stdout={}",
            text(&o.stdout)
        );
        let err = text(&o.stderr);
        assert!(
            err.contains("Read-only file system") || err.contains("Operation not permitted"),
            "`{cmd}` failed for an unexpected reason: {err}"
        );
    }
    assert_eq!(
        digest(&baseline_file()),
        before,
        "the host's baseline must be byte-identical"
    );
    assert!(!baseline_dir().join("new-file").exists());
    assert!(!baseline_dir().parent().unwrap().join("escaped").exists());
}

#[test]
fn writes_anywhere_else_are_denied() {
    for path in ["/x", "/usr/x", "/etc-x", "/dev/x", "/proc/x", "/baseline"] {
        let o = sandboxed(&[], &["sh", "-c", &format!("echo x > {path}")]);
        assert!(!o.status.success(), "a write to {path} must be denied");
    }
    // Device nodes remain usable.
    let o = sandboxed(&[], &["sh", "-c", "echo discarded > /dev/null"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
}

#[test]
fn scratch_starts_empty_and_nothing_survives_the_run() {
    let o = sandboxed(
        &[],
        &[
            "sh",
            "-c",
            "ls -A /scratch | wc -l; echo persist > /scratch/leftover",
        ],
    );
    assert_eq!(text(&o.stdout).trim(), "0");
    let again = sandboxed(&[], &["sh", "-c", "ls -A /scratch | wc -l"]);
    assert_eq!(
        text(&again.stdout).trim(),
        "0",
        "scratch must not persist between runs"
    );
}

#[test]
fn the_host_environment_and_network_are_not_visible() {
    let o = sandboxed(&[], &["sh", "-c", "env"]);
    assert!(
        !text(&o.stdout).contains("leaked-from-host"),
        "{}",
        text(&o.stdout)
    );
    assert!(text(&o.stdout).contains("HOME=/scratch"));
    let o = sandboxed(&[], &["sh", "-c", "cat /proc/net/dev"]);
    let interfaces: Vec<String> = text(&o.stdout)
        .lines()
        .skip(2)
        .filter_map(|l| l.split(':').next().map(|s| s.trim().to_string()))
        .collect();
    assert_eq!(
        interfaces,
        vec!["lo".to_string()],
        "only loopback: no network"
    );
}

#[test]
fn a_declared_writable_mount_is_writable_and_only_that() {
    let out = std::env::temp_dir().join(format!("kr0ki-a07-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let mount = format!("{}:/out", out.display());
    let o = sandboxed(
        &["--writable", &mount],
        &["sh", "-c", "echo declared > /out/result.txt"],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(
        std::fs::read_to_string(out.join("result.txt"))
            .unwrap()
            .trim(),
        "declared"
    );
    // ...while everything not declared is still read-only.
    let o = sandboxed(
        &["--writable", &mount],
        &["sh", "-c", "echo x > /baseline/x"],
    );
    assert!(!o.status.success());
}

#[test]
fn the_baseline_and_system_paths_cannot_be_declared_writable() {
    let tmp = std::env::temp_dir();
    for dest in ["/baseline", "/baseline/sub", "/usr", "/proc", "/dev", ""] {
        let mount = format!("{}:{dest}", tmp.display());
        let o = sandboxed(&["--writable", &mount], &["true"]);
        assert_eq!(
            o.status.code(),
            Some(2),
            "`{dest}` must be refused: {}",
            text(&o.stderr)
        );
        assert!(text(&o.stderr).contains("refusing"), "{}", text(&o.stderr));
    }
}

#[test]
fn bad_invocations_are_refused_before_anything_runs() {
    let script = repo_root().join("deploy/sandbox/run-sandboxed.sh");
    let run = |args: &[&str]| Command::new(&script).args(args).output().unwrap();
    assert_eq!(run(&[]).status.code(), Some(2), "no --baseline");
    assert_eq!(
        run(&["--baseline", "/definitely/not/a/dir", "--", "true"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        run(&["--baseline", &baseline_dir().to_string_lossy()])
            .status
            .code(),
        Some(2),
        "no command"
    );
    assert_eq!(run(&["--surprise"]).status.code(), Some(2));
}
