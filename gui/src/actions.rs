//! File actions: generate config, export baked script, run the real script as
//! a preview subprocess (with an 8s kill timeout).
use crate::settings::{expand_tilde, Settings};
use std::io;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const EMBEDDED_SCRIPT: &str = include_str!("../../vps-info.sh");

pub const PREVIEW_TIMEOUT: Duration = Duration::from_secs(8);

/// Write the runtime config file (KEY=VALUE, read by the canonical script).
pub fn generate_config(settings: &Settings, profile: &str, path: &str) -> io::Result<()> {
    let p = expand_tilde(path);
    if let Some(dir) = std::path::Path::new(&p).parent() {
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(&p, settings.config_lines(profile))
}

/// Write the baked standalone script (export header + canonical script).
pub fn export_baked(settings: &Settings, profile: &str, path: &str) -> io::Result<()> {
    let p = expand_tilde(path);
    if let Some(dir) = std::path::Path::new(&p).parent() {
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(&p, export_baked_text(settings, profile))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

/// The full text of a baked export (used for the overwrite diff too).
pub fn export_baked_text(settings: &Settings, profile: &str) -> String {
    let mut body = settings.baked_header(profile);
    body.push('\n');
    body.push_str(EMBEDDED_SCRIPT);
    body
}

pub struct PreviewOutcome {
    pub text: String,
    pub error: Option<String>,
    pub timed_out: bool,
}

pub enum PreviewState {
    Idle,
    Running { rx: mpsc::Receiver<PreviewOutcome> },
    Done(PreviewOutcome),
}

impl PreviewState {
    pub fn poll(&mut self, ctx: &eframe::egui::Context) {
        let (state, repaint) = match self {
            PreviewState::Running { rx } => match rx.try_recv() {
                Ok(outcome) => (Some(PreviewState::Done(outcome)), false),
                Err(mpsc::TryRecvError::Empty) => (None, true),
                Err(mpsc::TryRecvError::Disconnected) => (
                    Some(PreviewState::Done(PreviewOutcome {
                        text: String::new(),
                        error: Some("preview process died unexpectedly".into()),
                        timed_out: false,
                    })),
                    false,
                ),
            },
            _ => (None, false),
        };
        if let Some(s) = state {
            *self = s;
        } else if repaint {
            ctx.request_repaint();
        }
    }
}

/// Environment for a preview run: mirrors the current settings, forces
/// colors on (so we can render them) and keeps the network off for speed.
fn preview_env(settings: &Settings) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for (k, on) in settings.section_flags() {
        v.push((k.to_string(), if on { "1".into() } else { "0".into() }));
    }
    v.push(("VPSINFO_FRAME".into(), if settings.frame { "1" } else { "0" }.into()));
    v.push(("VPSINFO_COLOR".into(), "always".into()));
    v.push(("VPSINFO_SKIP_CPU".into(), if settings.live_cpu_sample { "0" } else { "1" }.into()));
    v.push(("VPSINFO_SKIP_UPDATES".into(), if settings.update_counts { "0" } else { "1" }.into()));
    v.push(("NO_PUBLIC_IP".into(), "1".into())); // public IP stays off in preview
    v.push(("VPSINFO_SKIP_GEO".into(), "1".into()));
    v.push(("TERM".into(), "xterm-256color".into()));
    v
}

/// Spawn `bash -s` fed with the embedded script; the worker thread enforces
/// the 8s timeout and kills the child.
pub fn start_preview(settings: &Settings) -> PreviewState {
    let (tx, rx) = mpsc::channel();
    let envs = preview_env(settings);
    std::thread::spawn(move || {
        let mut cmd = Command::new("bash");
        cmd.arg("-s");
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        for (k, val) in &envs {
            cmd.env(k, val);
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(PreviewOutcome {
                    text: String::new(),
                    error: Some(format!("could not start bash: {e}")),
                    timed_out: false,
                });
                return;
            }
        };
        if let Some(mut si) = child.stdin.take() {
            let _ = si.write_all(EMBEDDED_SCRIPT.as_bytes());
            drop(si);
        }
        let start = Instant::now();
        let mut timed_out = false;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {}
                Err(e) => {
                    let _ = tx.send(PreviewOutcome {
                        text: String::new(),
                        error: Some(format!("preview failed: {e}")),
                        timed_out: false,
                    });
                    return;
                }
            }
            if start.elapsed() > PREVIEW_TIMEOUT {
                let _ = child.kill();
                timed_out = true;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        let _ = child.wait();
        let mut out = String::new();
        let mut err = String::new();
        if let Some(mut so) = child.stdout.take() {
            let _ = io::Read::read_to_string(&mut so, &mut out);
        }
        if let Some(mut se) = child.stderr.take() {
            let _ = io::Read::read_to_string(&mut se, &mut err);
        }
        let error = if err.trim().is_empty() { None } else { Some(err) };
        let _ = tx.send(PreviewOutcome { text: out, error, timed_out });
    });
    PreviewState::Running { rx }
}