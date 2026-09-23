//! rc-file integration: detect shells, sourced status, idempotent marker-block
//! edits, backups and one-click restore.
use std::io;
use std::path::{Path, PathBuf};

pub const BLOCK_START: &str = "# ---- vpsinfo:start ----";
pub const BLOCK_END: &str = "# ---- vpsinfo:end ----";

#[derive(Clone, Debug)]
pub struct RcFile {
    pub path: PathBuf,
    pub exists: bool,
    pub selected: bool,
    pub sourced: bool,
    pub create: bool,
}

impl RcFile {
    pub fn label(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

/// Candidate rc files in the user's home (~/.bashrc, ~/.zshrc, ~/.bash_profile, ~/.profile).
pub fn candidates() -> Vec<RcFile> {
    let home = crate::settings::home_dir().unwrap_or_else(|| PathBuf::from("."));
    [".bashrc", ".zshrc", ".bash_profile", ".profile"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let path = home.join(name);
            let exists = path.exists();
            let sourced = exists && is_sourced(&path);
            RcFile {
                path,
                exists,
                selected: exists && i == 0 && !sourced,
                sourced,
                create: false,
            }
        })
        .collect()
}

pub fn is_sourced(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|t| t.contains("vpsinfo:start"))
        .unwrap_or(false)
}

/// The exact marker block that will be appended.
pub fn build_block(script_path: &str, ssh_only: bool) -> String {
    let script = script_path.trim();
    let mut b = String::new();
    b.push('\n');
    b.push_str(BLOCK_START);
    b.push('\n');
    if ssh_only {
        b.push_str("if [ -n \"${SSH_CONNECTION:-}\" ]; then\n  ");
    }
    b.push_str(script);
    b.push('\n');
    if ssh_only {
        b.push_str("fi\n");
    }
    b.push_str(BLOCK_END);
    b.push('\n');
    b
}

/// `.bashrc` -> `.bashrc.vpsinfo.bak.N` (N = 1 newest .. 3 oldest).
fn backup_path(p: &Path, n: u32) -> PathBuf {
    PathBuf::from(format!("{}.vpsinfo.bak.{}", p.display(), n))
}

fn rotate_backups(p: &Path) -> io::Result<()> {
    for i in (1..4).rev() {
        let src = backup_path(p, i);
        let dst = backup_path(p, i + 1);
        if src.exists() {
            let _ = std::fs::rename(&src, &dst);
        }
    }
    std::fs::copy(p, backup_path(p, 1))?;
    Ok(())
}

/// List existing backups (newest first) for an rc file.
pub fn backups(p: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = (1..=4)
        .filter_map(|n| {
            let b = backup_path(p, n);
            b.exists().then_some(b)
        })
        .collect();
    out.sort();
    out.reverse();
    out
}

pub fn restore(p: &Path, backup: &Path) -> io::Result<()> {
    std::fs::copy(backup, p).map(|_| ())
}

/// Append the marker block (idempotent: refuses if already present). Backs up first.
pub fn add_block(p: &Path, script_path: &str, ssh_only: bool) -> io::Result<()> {
    if is_sourced(p) {
        return Ok(());
    }
    rotate_backups(p)?;
    let mut content = std::fs::read_to_string(p).unwrap_or_default();
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&build_block(script_path, ssh_only));
    std::fs::write(p, content)
}

/// Strip only our marker block. Backs up first.
pub fn remove_block(p: &Path) -> io::Result<()> {
    if !is_sourced(p) {
        return Ok(());
    }
    rotate_backups(p)?;
    let content = std::fs::read_to_string(p).unwrap_or_default();
    let mut out = String::new();
    let mut in_block = false;
    for line in content.lines() {
        if line.contains("vpsinfo:start") {
            in_block = true;
            continue;
        }
        if line.contains("vpsinfo:end") {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }
    // trim trailing blank lines (the block usually ends the file)
    while out.ends_with("\n\n") {
        out.pop();
    }
    std::fs::write(p, out.trim_end_matches('\n').to_string() + "\n")
}

/// A planned edit to one rc file (content it will have after the operation).
#[derive(Clone, Debug)]
pub struct RcEditPlan {
    pub path: PathBuf,
    pub after: String,
}

/// What a diff dialog needs: per-file label + +/- lines.
pub fn plan_add(p: &Path, script_path: &str, ssh_only: bool) -> RcEditPlan {
    let mut content = std::fs::read_to_string(p).unwrap_or_default();
    if is_sourced(p) {
        return RcEditPlan { path: p.to_path_buf(), after: content };
    }
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&build_block(script_path, ssh_only));
    RcEditPlan { path: p.to_path_buf(), after: content }
}

pub fn plan_remove(p: &Path) -> RcEditPlan {
    let content = std::fs::read_to_string(p).unwrap_or_default();
    if !is_sourced(p) {
        return RcEditPlan { path: p.to_path_buf(), after: content };
    }
    let mut out = String::new();
    let mut in_block = false;
    for line in content.lines() {
        if line.contains("vpsinfo:start") {
            in_block = true;
            continue;
        }
        if line.contains("vpsinfo:end") {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    RcEditPlan { path: p.to_path_buf(), after: out.trim_end_matches('\n').to_string() + "\n" }
}

/// Application of a plan (with backup + write).
pub fn apply_plan(plan: &RcEditPlan, create_if_missing: bool) -> io::Result<()> {
    let p = &plan.path;
    if !p.exists() && !create_if_missing {
        return Ok(());
    }
    if p.exists() {
        rotate_backups(p)?;
    }
    if let Some(dir) = p.parent() {
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(p, &plan.after)
}

/// Simple +/- diff of `before` vs `after` (line based, shows tail of before when large).
pub fn diff_lines(before: &str, after: &str, max_tail: usize) -> Vec<(char, String)> {
    let mut out: Vec<(char, String)> = Vec::new();
    let old_lines: Vec<&str> = before.lines().collect();
    let new_lines: Vec<&str> = after.lines().collect();

    // Keep the diff readable for big files: show trailing old lines + all new lines
    // when the sizes differ a lot (typical for append-block edits).
    let show_old = if old_lines.len() > max_tail {
        old_lines.len().saturating_sub(max_tail)
    } else {
        0
    };
    for (idx, l) in old_lines.iter().enumerate() {
        if new_lines.get(idx).map(|n| n == l).unwrap_or(false) && idx >= show_old {
            out.push((' ', l.to_string()));
        } else if idx >= show_old {
            out.push(('-', l.to_string()));
        }
    }
    for (idx, l) in new_lines.iter().enumerate() {
        if old_lines.get(idx).map(|o| o == l).unwrap_or(false) && idx >= show_old {
            continue; // unchanged, already shown as ' '
        }
        if idx >= show_old {
            out.push(('+', l.to_string()));
        }
    }
    out
}