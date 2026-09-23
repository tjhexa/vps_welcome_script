//! Dummy preview: a static mock banner that reacts to the section toggles,
//! colorised with the same green/yellow/red thresholds as the real script.
//! Also renders ANSI-coloured output from the real-preview run.
use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, FontId, TextFormat};
use crate::settings::Sections;

pub const GREEN: Color32 = Color32::from_rgb(0x4c, 0xaf, 0x50);
pub const YELLOW: Color32 = Color32::from_rgb(0xff, 0xc1, 0x07);
pub const RED: Color32 = Color32::from_rgb(0xf4, 0x43, 0x36);
pub const CYAN: Color32 = Color32::from_rgb(0x00, 0xbc, 0xd4);
pub const BLUE: Color32 = Color32::from_rgb(0x21, 0x96, 0xf3);
pub const DIM: Color32 = Color32::from_gray(0x8c);
pub const FG: Color32 = Color32::from_gray(0xe1);

/// a (text, color, bold, dim) run
pub type Run = (String, Color32, bool, bool);
pub type Line = Vec<Run>;

fn sec(t: &str) -> Line {
    vec![(format!("── {t}"), CYAN, true, false)]
}
fn row(label: &str, value: &str) -> Line {
    vec![
        (format!("{label:<16}"), DIM, false, true),
        (value.to_string(), FG, false, false),
    ]
}
fn table(headers: &[&str], rows: &[Vec<(String, Color32)>], widths: &[usize]) -> Vec<Line> {
    let cells_to_line = |cells: &[(String, Color32)], widths: &[usize]| -> Line {
        let mut line: Line = Vec::new();
        line.push(("  |".to_string(), FG, false, false));
        for (i, (cell, c)) in cells.iter().enumerate() {
            let pad = widths[i].saturating_sub(cell.chars().count());
            line.push((format!(" {cell}{}", " ".repeat(pad)), *c, false, false));
            line.push((" |".to_string(), FG, false, false));
        }
        line
    };
    let border = format!(
        "  +{}",
        widths.iter().map(|w| "-".repeat(w + 2)).collect::<Vec<_>>().join("+")
    ) + "+";
    let mut lines = Vec::new();
    lines.push(vec![(border.clone(), DIM, false, true)]);
    let header: Vec<(String, Color32)> = headers.iter().map(|h| (h.to_string(), FG)).collect();
    lines.push(cells_to_line(&header, widths));
    lines.push(vec![(border.clone(), DIM, false, true)]);
    for r in rows {
        lines.push(cells_to_line(r, widths));
    }
    lines.push(vec![(border, DIM, false, true)]);
    lines
}

/// Build the mock banner lines for the given sections.
pub fn build_mock(s: &Sections, frame: bool) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut push = |l: Line| lines.push(l);

    push(vec![("VPS / SYSTEM INFO - Mon Jan 13 09:41".to_string(), CYAN, true, false)]);
    push(vec![]);

    if s.summary {
        push(vec![
            ("  CPU 12%".to_string(), GREEN, false, false),
            (" | MEM 34%".to_string(), GREEN, false, false),
            (" | DISK 22%".to_string(), GREEN, false, false),
            (" | containers 3/3".to_string(), GREEN, false, false),
            (" | load 0.45".to_string(), DIM, false, true),
        ]);
        push(vec![]);
    }
    if s.banner {
        push(vec![("  [+] all systems nominal".to_string(), GREEN, true, false)]);
        push(vec![]);
    }
    if s.system {
        push(row("Hostname", "my-vps (my-vps)"));
        push(row("User", "root @ root"));
        push(row("OS", "Ubuntu 24.04.4 LTS (x86_64)"));
        push(row("Kernel", "6.8.0-41-generic"));
        push(row("Uptime", "12d 4h 31m"));
        push(row("Load (1/5/15)", "0.45 0.51 0.60"));
    }
    if s.cpu {
        push(row("CPU", "Intel(R) Xeon(R) Gold 6230"));
        push(row("Cores", "8 (8 online)"));
        push(row("CPU usage", "12% (0.2s sample)"));
    }
    if s.memory {
        push(row("Memory", "34% used  1024 MB / 4096 MB"));
        push(row("Swap", "0 MB / 2048 MB"));
    }
    if s.disk {
        push(sec("Disk"));
        for l in table(
            &["MOUNT", "DEVICE", "SIZE", "USED", "USE%"],
            &[
                vec![("/".into(), FG), ("/dev/sda1".into(), FG), ("40G".into(), FG), ("9G".into(), FG), ("22%".into(), GREEN)],
                vec![("/home".into(), FG), ("/dev/sda2".into(), FG), ("100G".into(), FG), ("55G".into(), FG), ("55%".into(), YELLOW)],
                vec![("/data".into(), FG), ("/dev/sdb1".into(), FG), ("500G".into(), FG), ("420G".into(), FG), ("84%".into(), RED)],
            ],
            &[10, 10, 6, 6, 6],
        ) {
            push(l);
        }
    }
    if s.network {
        push(row("LAN IP", "10.0.0.5"));
        push(row("DNS", "1.1.1.1 9.9.9.9"));
        push(row("Public IP", "203.0.113.7"));
        push(row("Geo / ASN", "Berlin, DE (AS-Demo)"));
    }
    if s.processes {
        push(row("Processes", "128 running"));
        push(row("Logged-in", "2 session(s)"));
        push(row("Last login", "Mon Jan 12 22:14 from 10.0.0.1"));
        push(row("Failed srvcs", "none"));
    }
    if s.docker {
        push(sec("Docker"));
        push(row("Docker", "client 24.0.7"));
        push(vec![
            ("Containers [##########] ".to_string(), GREEN, true, false),
            ("3 up / 3 total".to_string(), FG, false, false),
        ]);
        for l in table(
            &["NAME", "IMAGE", "STATUS"],
            &[
                vec![("web".into(), FG), ("nginx:latest".into(), FG), ("Up 2h".into(), GREEN)],
                vec![("db".into(), FG), ("postgres:16".into(), FG), ("Up 2h".into(), GREEN)],
                vec![("cache".into(), FG), ("redis:7".into(), FG), ("Up 2h".into(), GREEN)],
            ],
            &[8, 14, 12],
        ) {
            push(l);
        }
    }
    if s.web {
        push(sec("Web server"));
        push(row("nginx", "active (enabled)"));
        push(vec![("  example.com  (listen: 80, 443)  ".to_string(), GREEN, false, false),
                  ("→ proxy: 127.0.0.1:8080".to_string(), YELLOW, false, false)]);
    }
    if s.ports {
        push(sec("Open ports"));
        for l in table(
            &["PROTO", "ADDRESS:PORT", "STATE", "PROCESS"],
            &[
                vec![("tcp".into(), FG), ("0.0.0.0:22".into(), FG), ("LISTEN".into(), FG), ("sshd".into(), FG)],
                vec![("tcp".into(), FG), ("0.0.0.0:80".into(), FG), ("LISTEN".into(), FG), ("nginx".into(), FG)],
                vec![("tcp".into(), FG), ("0.0.0.0:443".into(), FG), ("LISTEN".into(), FG), ("nginx".into(), FG)],
            ],
            &[6, 14, 8, 10],
        ) {
            push(l);
        }
    }
    if s.security {
        push(sec("Security"));
        push(row("fail2ban", "active (v1.0.2)"));
        push(row("Jails", "2 configured"));
        for l in table(
            &["JAIL", "BANNED NOW", "BANNED TOT"],
            &[
                vec![("sshd".into(), FG), ("3".into(), RED), ("45".into(), FG)],
                vec![("nginx-http".into(), FG), ("0".into(), GREEN), ("12".into(), FG)],
            ],
            &[12, 10, 12],
        ) {
            push(l);
        }
        push(row("SSH fails", "0 (last 24h)"));
    }
    if s.maint {
        push(sec("Maintenance"));
        push(row("Updates", "up to date"));
        push(row("Reboot", "not needed"));
        push(row("Inodes", "ok (max 41%)"));
    }
    if s.tools {
        push(sec("Dev tools"));
        for t in ["node  v20.11.0", "python3  3.12.3", "go  1.22.1", "rustc  1.77.0", "gcc  13.2.0"] {
            push(vec![
                (format!("  {t:<26}"), GREEN, false, false),
                // matches the real script's installed-tool line
            ]);
        }
    }
    if s.footer {
        push(vec![]);
        push(vec![("[+] 42 checks retrieved - 1.2s - public IP: cached".to_string(), GREEN, false, false)]);
    }

    if frame {
        apply_frame(&mut lines);
    }
    lines
}

/// Wrap lines in a screenfetch-style box (like the real script's frame()).
fn apply_frame(lines: &mut Vec<Line>) {
    let width = lines
        .iter()
        .map(|l| l.iter().map(|(t, _, _, _)| t.chars().count()).sum())
        .max()
        .unwrap_or(0);
    let bar = format!(
        "+{}+",
        "-".repeat(width + 2)
    );
    let mut framed: Vec<Line> = Vec::new();
    let mut first = true;
    framed.push(vec![(bar.clone(), CYAN, true, false)]);
    for l in lines.iter() {
        let len: usize = l.iter().map(|(t, _, _, _)| t.chars().count()).sum();
        let mut out: Line = Vec::new();
        out.push(("| ".to_string(), CYAN, false, false));
        for (t, c, b, d) in l {
            out.push((t.clone(), *c, *b, *d));
        }
        if len < width {
            out.push((" ".repeat(width - len), FG, false, false));
        }
        out.push((" |".to_string(), CYAN, false, false));
        framed.push(out);
        if first {
            let dashes = format!("| {}+ |", "-".repeat(width));
            framed.push(vec![(dashes, CYAN, false, true)]);
            first = false;
        }
    }
    framed.push(vec![(bar, CYAN, true, false)]);
    *lines = framed;
}

/// Turn mock lines into an egui LayoutJob (monospace, wrapped).
pub fn lines_to_job(lines: &[Line], size: f32) -> LayoutJob {
    let font = FontId::monospace(size);
    let mut job = LayoutJob::default();
    for (li, line) in lines.iter().enumerate() {
        if li > 0 {
            job.append("\n", 0.0, TextFormat::default());
        }
        for (t, c, bold, dim) in line {
            let color = if *dim { DIM } else if *bold { Color32::WHITE } else { *c };
            job.append(
                t,
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color,
                    ..Default::default()
                },
            );
        }
    }
    let _ = &job;
    job
}

/// Maximum character width of mock lines (for wrap-aware preview, V2 #23).
pub fn mock_max_width(lines: &[Line]) -> usize {
    lines
        .iter()
        .map(|l| l.iter().map(|(t, _, _, _)| t.chars().count()).sum())
        .max()
        .unwrap_or(0)
}

/// Maximum character width of ANSI text after stripping SGR escapes.
pub fn ansi_max_width(text: &str) -> usize {
    let stripped: String = strip_ansi(text);
    stripped.lines().map(|l| l.chars().count()).max().unwrap_or(0)
}

pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for n in chars.by_ref() {
                if ('@'..='~').contains(&n) {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse ANSI SGR escapes from real-script output into a LayoutJob.
pub fn ansi_to_job(text: &str, size: f32) -> LayoutJob {
    let font = FontId::monospace(size);
    let mut job = LayoutJob::default();
    let mut cur = FG;
    let mut bold = false;
    let mut dim = false;
    let base = FG;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut run = String::new();
    let flush = |run: &mut String, cur: Color32, bold: bool, dim: bool, job: &mut LayoutJob| {
        if !run.is_empty() {
            let color = if dim { DIM } else if bold { Color32::WHITE } else { cur };
            let text = std::mem::take(run);
            job.append(
                &text,
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color,
                    ..Default::default()
                },
            );
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '\x1b' && i + 1 < chars.len() && chars[i + 1] == '[' {
            flush(&mut run, cur, bold, dim, &mut job);
            // find terminating 'm'
            let mut j = i + 2;
            while j < chars.len() && chars[j] != 'm' {
                j += 1;
            }
            if j < chars.len() {
                let code_str: String = chars[i + 2..j].iter().collect();
                let codes: Vec<&str> = code_str.split(';').collect();
                for code in codes {
                    match code {
                        "0" => {
                            cur = base;
                            bold = false;
                            dim = false;
                        }
                        "1" => bold = true,
                        "2" => dim = true,
                        "22" => {
                            bold = false;
                            dim = false;
                        }
                        "30" => cur = Color32::from_gray(0x30),
                        "31" => cur = RED,
                        "32" => cur = GREEN,
                        "33" => cur = YELLOW,
                        "34" => cur = BLUE,
                        "35" => cur = Color32::from_rgb(0xe0, 0x40, 0x9e),
                        "36" => cur = CYAN,
                        "37" => cur = FG,
                        "39" => cur = base,
                        _ => {}
                    }
                }
                i = j + 1;
                continue;
            }
        }
        run.push(c);
        i += 1;
    }
    flush(&mut run, cur, bold, dim, &mut job);
    job
}