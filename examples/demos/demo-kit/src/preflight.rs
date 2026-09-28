//! Preflight: every reason a demo must not run, collected before any step.
//!
//! The probe is a trait so the falsifiers can drive it with a fake tool set;
//! [`SystemProbe`] asks the real binaries.

use crate::manifest::DemoManifest;
use crate::pin;
use crate::sha;
use crate::verdict::NotRunReason;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where forjar installs the course tools. A tool's declared location
/// defaults to `<DECLARED_BIN_DIR>/<tool>`; override per host with
/// `RFML5_DECLARED_<TOOL>` (colon-separated absolute paths), e.g.
/// `RFML5_DECLARED_APR` or `RFML5_DECLARED_AGY`.
pub const DECLARED_BIN_DIR: &str = "/opt/course-bin/bin";

/// The env var naming the declared locations of `tool`:
/// `claude` → `RFML5_DECLARED_CLAUDE`, `foo-bar` → `RFML5_DECLARED_FOO_BAR`.
pub fn declared_env_var(tool: &str) -> String {
    let name: String = tool
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("RFML5_DECLARED_{name}")
}

/// Parse a colon-separated declared set, falling back to the forjar default.
pub fn declared_set(tool: &str, env: Option<&str>) -> Vec<PathBuf> {
    match env {
        Some(v) => v
            .split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect(),
        None => vec![Path::new(DECLARED_BIN_DIR).join(tool)],
    }
}

/// Every tool this demo invokes that the kit can name: the pinned tools it
/// uses (apr, agy), then the program word of each `needs` entry (`"claude"`,
/// `"apr serve run"` → apr). Flag-only needs (`--json-schema`) name no tool.
/// Step commands are not scanned: their programs are the build toolchain
/// (`cargo`) or a pinned tool already listed.
pub fn invoked_tools(m: &DemoManifest) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let pinned = [("apr", m.uses_apr()), ("agy", m.uses_agy())];
    let from_needs = m.needs.iter().filter_map(|n| n.split_whitespace().next());
    for t in pinned
        .iter()
        .filter(|(_, used)| *used)
        .map(|(t, _)| *t)
        .chain(from_needs.filter(|w| !w.starts_with('-')))
    {
        if !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    }
    out
}

pub trait Probe {
    /// Absolute path `tool` resolves to on PATH, if any.
    fn which(&self, tool: &str) -> Option<PathBuf>;
    /// `<tool> --version`, parsed.
    fn version(&self, tool: &str) -> Option<String>;
    /// Is this `needs` entry available (not refused) in the installed tools?
    fn available(&self, need: &str) -> bool;
    /// Does any job hold a GPU reservation?
    fn gpu_reserved(&self) -> bool;
    /// The forjar-declared locations `tool` may resolve to.
    fn declared(&self, tool: &str) -> Vec<PathBuf>;
}

pub fn preflight(m: &DemoManifest, base: &Path, probe: &dyn Probe) -> Vec<NotRunReason> {
    let mut out = Vec::new();

    // RD-1: what actually runs is what PATH resolves to, so every invoked
    // tool's resolved path must be forjar-declared — a shim that prints the
    // pinned `--version` still goes red here.
    for tool in invoked_tools(m) {
        if let Some(path) = probe.which(&tool) {
            if !probe.declared(&tool).contains(&path) {
                out.push(NotRunReason::UndeclaredTool {
                    tool: tool.clone(),
                    path: path.display().to_string(),
                });
            }
        }
    }

    for (tool, spec, used) in [("apr", &m.apr, m.uses_apr()), ("agy", &m.agy, m.uses_agy())] {
        if !used {
            continue;
        }
        match pin::parse_exact(spec) {
            Err(bad) => out.push(NotRunReason::PinRefused(format!("{tool} {bad}"))),
            Ok(p) => match probe.which(tool) {
                None => out.push(NotRunReason::MissingTool(tool.into())),
                Some(_) => {
                    let found = probe.version(tool).unwrap_or_else(|| "unknown".into());
                    if found != p.0 {
                        out.push(NotRunReason::VersionMismatch {
                            tool: tool.into(),
                            pinned: p.0,
                            found,
                        });
                    }
                }
            },
        }
    }

    if let Some(model) = &m.model {
        let found = sha::sha256_file(&base.join(&model.path)).unwrap_or_else(|_| "missing".into());
        if found != model.sha256 {
            out.push(NotRunReason::WeightsMismatch {
                expected: model.sha256.clone(),
                found,
            });
        }
    }

    if let Some(fx) = &m.fixtures {
        let found = sha::tree_hash(&base.join(&fx.dir)).unwrap_or_else(|_| "missing".into());
        if found != fx.sha256 {
            out.push(NotRunReason::FixturesMismatch {
                expected: fx.sha256.clone(),
                found,
            });
        }
    }

    for need in &m.needs {
        if !probe.available(need) {
            out.push(NotRunReason::Refused(need.clone()));
        }
    }

    if m.host_class.contains("cuda") && probe.gpu_reserved() {
        out.push(NotRunReason::GpuLockHeld);
    }
    out
}

/// The real tools on this host.
pub struct SystemProbe;

fn run(tool: &str, args: &[&str]) -> Option<(bool, String)> {
    let o = Command::new(tool).args(args).output().ok()?;
    let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&o.stderr));
    Some((o.status.success(), text))
}

impl SystemProbe {
    fn help(&self, tool: &str, subs: &[&str]) -> Option<String> {
        let mut args: Vec<&str> = subs.to_vec();
        args.push("--help");
        match run(tool, &args)? {
            (true, text) if !text.contains("unavailable in this build") => Some(text),
            _ => None,
        }
    }
}

impl Probe for SystemProbe {
    fn which(&self, tool: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|d| d.join(tool))
            .find(|p| p.is_file())
    }

    fn version(&self, tool: &str) -> Option<String> {
        let (ok, text) = run(tool, &["--version"])?;
        ok.then(|| pin::version_from_output(tool, &text)).flatten()
    }

    /// `"apr serve run"` → `apr serve run --help` exits 0.
    /// `"apr run --json"` → and `--json` appears in that help.
    /// `"--json-schema"` → appears in `apr run --help` or `apr serve run --help`.
    /// `"agy"` / `"agy ..."` → `agy --version` exits 0.
    fn available(&self, need: &str) -> bool {
        let words: Vec<&str> = need.split_whitespace().collect();
        match words.first() {
            Some(&"agy") => run("agy", &["--version"]).is_some_and(|(ok, _)| ok),
            Some(&"apr") => {
                let subs: Vec<&str> = words[1..]
                    .iter()
                    .copied()
                    .filter(|w| !w.starts_with('-'))
                    .collect();
                let flags: Vec<&str> = words[1..]
                    .iter()
                    .copied()
                    .filter(|w| w.starts_with('-'))
                    .collect();
                self.help("apr", &subs)
                    .is_some_and(|h| flags.iter().all(|f| h.contains(f)))
            }
            Some(flag) if flag.starts_with("--") => [&["run"][..], &["serve", "run"][..]]
                .iter()
                .filter_map(|subs| self.help("apr", subs))
                .any(|h| h.contains(flag)),
            Some(tool) => run(tool, &["--version"]).is_some_and(|(ok, _)| ok),
            None => false,
        }
    }

    fn gpu_reserved(&self) -> bool {
        let Some((true, text)) = run("apr", &["gpu", "--json"]) else {
            return false;
        };
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("reservations")
                    .and_then(|r| r.as_array())
                    .map(|a| !a.is_empty())
            })
            .unwrap_or(false)
    }

    fn declared(&self, tool: &str) -> Vec<PathBuf> {
        declared_set(tool, std::env::var(declared_env_var(tool)).ok().as_deref())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    /// A tool set the falsifiers control.
    pub struct FakeProbe {
        /// Resolved path and `--version` of apr.
        pub apr: Option<(PathBuf, String)>,
        /// Resolved path and `--version` of agy.
        pub agy: Option<(PathBuf, String)>,
        /// Resolved paths of unpinned tools named in `needs`.
        pub others: BTreeMap<String, PathBuf>,
        pub refused: BTreeSet<String>,
        pub gpu_reserved: bool,
    }

    fn declared_path(tool: &str) -> PathBuf {
        Path::new(DECLARED_BIN_DIR).join(tool)
    }

    impl Default for FakeProbe {
        fn default() -> Self {
            Self {
                apr: Some((declared_path("apr"), "0.69.3".into())),
                agy: Some((declared_path("agy"), "1.2.11".into())),
                others: [("claude".to_string(), declared_path("claude"))].into(),
                refused: BTreeSet::new(),
                gpu_reserved: false,
            }
        }
    }

    impl Probe for FakeProbe {
        fn which(&self, tool: &str) -> Option<PathBuf> {
            match tool {
                "apr" => self.apr.as_ref().map(|(p, _)| p.clone()),
                "agy" => self.agy.as_ref().map(|(p, _)| p.clone()),
                _ => self.others.get(tool).cloned(),
            }
        }
        fn version(&self, tool: &str) -> Option<String> {
            match tool {
                "apr" => self.apr.as_ref().map(|(_, v)| v.clone()),
                "agy" => self.agy.as_ref().map(|(_, v)| v.clone()),
                _ => None,
            }
        }
        fn available(&self, need: &str) -> bool {
            !self.refused.contains(need)
        }
        fn gpu_reserved(&self) -> bool {
            self.gpu_reserved
        }
        fn declared(&self, tool: &str) -> Vec<PathBuf> {
            declared_set(tool, None)
        }
    }

    fn manifest(extra: &str) -> DemoManifest {
        DemoManifest::parse(&format!(
            r#"
id = "d04-json-verdict"
title = "t"
lesson = "rfml5/3.2"
apr = "=0.69.3"
host_class = "gx10-cuda"
needs = ["apr serve run", "--json-schema"]
{extra}
[assert]
exit = 0
[record]
target_duration_s = 300
resolution = "1920x1080"
"#
        ))
        .unwrap()
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn clean_preflight_is_empty() {
        assert!(preflight(&manifest(""), Path::new("."), &FakeProbe::default()).is_empty());
    }

    /// demo-pin-v1 (floor arm): a floor pin never runs.
    #[test]
    fn demo_pin_v1_floor_pin_refused() {
        let m = manifest("").clone();
        let m = DemoManifest {
            apr: ">=0.69.3".into(),
            ..m
        };
        let r = preflight(&m, Path::new("."), &FakeProbe::default());
        assert_eq!(r, vec![NotRunReason::PinRefused("apr >=0.69.3".into())]);
    }

    #[test]
    fn version_mismatch_is_notrun() {
        let probe = FakeProbe {
            apr: Some((declared_path("apr"), "0.69.1".into())),
            ..Default::default()
        };
        let r = preflight(&manifest(""), Path::new("."), &probe);
        assert!(matches!(r[0], NotRunReason::VersionMismatch { .. }));
    }

    /// demo-pin-v1 (sha arm): wrong weights sha → NotRun{WeightsMismatch}.
    #[test]
    fn demo_pin_v1_wrong_sha_is_weights_mismatch() {
        let d = tmp("weights");
        std::fs::write(d.join("model.gguf"), b"not the declared bytes").unwrap();
        let m = manifest(&format!(
            "model = {{ name = \"Qwen3.5-4B-Q4_K_M\", sha256 = \"{}\", path = \"model.gguf\" }}",
            "0".repeat(64)
        ));
        let r = preflight(&m, &d, &FakeProbe::default());
        assert!(
            matches!(r.as_slice(), [NotRunReason::WeightsMismatch { .. }]),
            "{r:?}"
        );
        let good = crate::sha::sha256_file(&d.join("model.gguf")).unwrap();
        let m = manifest(&format!(
            "model = {{ name = \"Qwen3.5-4B-Q4_K_M\", sha256 = \"{good}\", path = \"model.gguf\" }}"
        ));
        assert!(preflight(&m, &d, &FakeProbe::default()).is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn missing_weights_is_weights_mismatch() {
        let m = manifest(&format!(
            "model = {{ name = \"m\", sha256 = \"{}\", path = \"/nonexistent/m.gguf\" }}",
            "0".repeat(64)
        ));
        let r = preflight(&m, Path::new("."), &FakeProbe::default());
        assert!(matches!(&r[0], NotRunReason::WeightsMismatch { found, .. } if found == "missing"));
    }

    #[test]
    fn refused_need_is_named() {
        let probe = FakeProbe {
            refused: ["--json-schema".to_string()].into(),
            ..Default::default()
        };
        let r = preflight(&manifest(""), Path::new("."), &probe);
        assert_eq!(r, vec![NotRunReason::Refused("--json-schema".into())]);
    }

    /// Shaped like d08: apr and agy pinned, plus an unpinned `claude` need.
    fn quorum_manifest() -> DemoManifest {
        DemoManifest::parse(
            r#"
id = "d08-planted-defect"
title = "t"
lesson = "rfml5/4.2"
apr = "=0.69.3"
agy = "=1.2.11"
host_class = "any"
needs = ["agy", "claude", "--json-schema"]
[assert]
exit = 0
[record]
target_duration_s = 300
resolution = "1920x1080"
"#,
        )
        .unwrap()
    }

    const SHIM_DIR: &str = "/home/someone/.local/bin";

    fn shim(tool: &str) -> PathBuf {
        Path::new(SHIM_DIR).join(tool)
    }

    fn undeclared(tool: &str) -> NotRunReason {
        NotRunReason::UndeclaredTool {
            tool: tool.into(),
            path: shim(tool).display().to_string(),
        }
    }

    #[test]
    fn invoked_tools_are_pinned_plus_needs_programs() {
        assert_eq!(invoked_tools(&quorum_manifest()), ["apr", "agy", "claude"]);
        assert_eq!(invoked_tools(&manifest("")), ["apr"]);
    }

    #[test]
    fn declared_set_defaults_to_forjar_bin_and_env_overrides() {
        assert_eq!(declared_env_var("apr"), "RFML5_DECLARED_APR");
        assert_eq!(declared_env_var("foo-bar"), "RFML5_DECLARED_FOO_BAR");
        assert_eq!(
            declared_set("agy", None),
            [PathBuf::from("/opt/course-bin/bin/agy")]
        );
        assert_eq!(
            declared_set("apr", Some("/a/apr::/b/apr")),
            [PathBuf::from("/a/apr"), PathBuf::from("/b/apr")]
        );
    }

    /// RD-1 green arm: every invoked tool at its declared path passes.
    #[test]
    fn rd1_declared_paths_pass() {
        let r = preflight(&quorum_manifest(), Path::new("."), &FakeProbe::default());
        assert!(r.is_empty(), "{r:?}");
    }

    /// RD-1 falsifier (apr): a shim that prints the pinned version is still
    /// NotRun{UndeclaredTool(apr, …)}.
    #[test]
    fn rd1_apr_shim_is_undeclared_tool() {
        let probe = FakeProbe {
            apr: Some((shim("apr"), "0.69.3".into())),
            ..Default::default()
        };
        let r = preflight(&manifest(""), Path::new("."), &probe);
        assert_eq!(r, vec![undeclared("apr")]);
        assert!(!crate::verdict::decide(&r, &[("exit".to_string(), true)].into()).is_green());
    }

    /// RD-1 falsifier (agy): the path check is not apr-only any more.
    #[test]
    fn rd1_agy_shim_is_undeclared_tool() {
        let probe = FakeProbe {
            agy: Some((shim("agy"), "1.2.11".into())),
            ..Default::default()
        };
        let r = preflight(&quorum_manifest(), Path::new("."), &probe);
        assert_eq!(r, vec![undeclared("agy")]);
    }

    /// RD-1 falsifier (unpinned tool from `needs`): `claude` has no pin, so
    /// only the resolved path stands between a shim and a green run.
    #[test]
    fn rd1_needs_tool_shim_is_undeclared_tool() {
        let mut probe = FakeProbe::default();
        probe.others.insert("claude".into(), shim("claude"));
        let r = preflight(&quorum_manifest(), Path::new("."), &probe);
        assert_eq!(r, vec![undeclared("claude")]);
    }

    /// Every checked tool shimmed at once: one reason per tool, none hidden.
    #[test]
    fn rd1_all_shims_each_named() {
        let mut probe = FakeProbe {
            apr: Some((shim("apr"), "0.69.3".into())),
            agy: Some((shim("agy"), "1.2.11".into())),
            ..Default::default()
        };
        probe.others.insert("claude".into(), shim("claude"));
        let r = preflight(&quorum_manifest(), Path::new("."), &probe);
        assert_eq!(
            r,
            vec![undeclared("apr"), undeclared("agy"), undeclared("claude")]
        );
    }

    #[test]
    fn gpu_lock_held_is_notrun_never_wait() {
        let probe = FakeProbe {
            gpu_reserved: true,
            ..Default::default()
        };
        assert_eq!(
            preflight(&manifest(""), Path::new("."), &probe),
            vec![NotRunReason::GpuLockHeld]
        );
    }

    #[test]
    fn fixtures_hash_is_checked() {
        let d = tmp("fixtures");
        std::fs::create_dir_all(d.join("fx")).unwrap();
        std::fs::write(d.join("fx/a.diff"), "x").unwrap();
        let h = crate::sha::tree_hash(&d.join("fx")).unwrap();
        let ok = manifest(&format!("fixtures = {{ dir = \"fx\", sha256 = \"{h}\" }}"));
        assert!(preflight(&ok, &d, &FakeProbe::default()).is_empty());
        std::fs::write(d.join("fx/a.diff"), "y").unwrap();
        assert!(matches!(
            preflight(&ok, &d, &FakeProbe::default()).as_slice(),
            [NotRunReason::FixturesMismatch { .. }]
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
