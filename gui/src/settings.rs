//! Settings model, presets, named profiles, undo/redo history and GUI state persistence.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Expand a leading `~` to the user's home directory.
pub fn expand_tilde(p: &str) -> String {
    let t = p.trim();
    if t == "~" {
        return home_dir()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|| t.to_string());
    }
    if let Some(rest) = t.strip_prefix("~/") {
        if let Some(h) = home_dir() {
            return h.join(rest).to_string_lossy().into_owned();
        }
    }
    t.to_string()
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ColorMode::Auto => "auto",
            ColorMode::Always => "always",
            ColorMode::Never => "never",
        }
    }
    pub fn parse(s: &str) -> ColorMode {
        match s {
            "always" => ColorMode::Always,
            "never" => ColorMode::Never,
            _ => ColorMode::Auto,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
pub enum Tab {
    #[default]
    Settings,
    Preview,
    RcManager,
    About,
}

impl Tab {
    pub fn label(&self) -> &'static str {
        match self {
            Tab::Settings => "Settings",
            Tab::Preview => "Preview",
            Tab::RcManager => "rc Manager",
            Tab::About => "About",
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct Sections {
    pub summary: bool,
    pub banner: bool,
    pub system: bool,
    pub cpu: bool,
    pub memory: bool,
    pub disk: bool,
    pub network: bool,
    pub processes: bool,
    pub docker: bool,
    pub web: bool,
    pub ports: bool,
    pub security: bool,
    pub maint: bool,
    pub tools: bool,
    pub footer: bool,
}

impl Default for Sections {
    fn default() -> Self {
        Self {
            summary: true,
            banner: true,
            system: true,
            cpu: true,
            memory: true,
            disk: true,
            network: true,
            processes: true,
            docker: true,
            web: true,
            ports: true,
            security: true,
            maint: true,
            tools: true,
            footer: true,
        }
    }
}

/// UI metadata for every section toggle: (label, tooltip, field accessor).
pub const SECTION_ENTRIES: &[(&str, &str, fn(&Sections) -> &bool, fn(&mut Sections) -> &mut bool)] = &[
    ("Summary bar", "CPU / memory / disk / containers / load at a glance", |s| &s.summary, |s| &mut s.summary),
    ("Health banner", "Problems flagged: failed units, disk/mem pressure, updates, reboot", |s| &s.banner, |s| &mut s.banner),
    ("System", "Hostname, user, OS, kernel, uptime, load", |s| &s.system, |s| &mut s.system),
    ("CPU", "Model, cores, live CPU usage", |s| &s.cpu, |s| &mut s.cpu),
    ("Memory", "Used memory + swap", |s| &s.memory, |s| &mut s.memory),
    ("Disk", "Boxed table: mount, device, size, used, use%", |s| &s.disk, |s| &mut s.disk),
    ("Network", "LAN IP, DNS, public IP + Geo/ASN", |s| &s.network, |s| &mut s.network),
    ("Processes", "Running processes, logged-in sessions, last login, failed services", |s| &s.processes, |s| &mut s.processes),
    ("Docker", "Client version, health bar, container table", |s| &s.docker, |s| &mut s.docker),
    ("Web server", "nginx/apache status + site configs", |s| &s.web, |s| &mut s.web),
    ("Open ports", "ss -ltunpH table with process names", |s| &s.ports, |s| &mut s.ports),
    ("Security", "fail2ban status + jails, SSH failures (installed only)", |s| &s.security, |s| &mut s.security),
    ("Maintenance", "Pending updates, reboot required, inode usage", |s| &s.maint, |s| &mut s.maint),
    ("Dev tools", "node/python/go/rustc/... versions", |s| &s.tools, |s| &mut s.tools),
    ("Footer", "Total checks, elapsed time, public-IP source", |s| &s.footer, |s| &mut s.footer),
];

impl Sections {
    pub fn desktop_full() -> Self {
        Self::default()
    }
    /// system, disk, network, maintenance only (plus summary/banner/footer)
    pub fn server_minimal() -> Self {
        Self {
            summary: true, banner: true, system: true,
            cpu: false, memory: false, disk: true, network: true, processes: false,
            docker: false, web: false, ports: false, security: false, maint: true,
            tools: false, footer: true,
        }
    }
    /// everything on except web + dev tools (docker host emphasis)
    pub fn docker_host() -> Self {
        Self {
            summary: true, banner: true, system: true, cpu: true, memory: true,
            disk: true, network: true, processes: true, docker: true,
            web: false, ports: true, security: true, maint: true,
            tools: false, footer: true,
        }
    }
    pub fn all_off() -> Self {
        Self {
            summary: false, banner: false, system: false, cpu: false, memory: false,
            disk: false, network: false, processes: false, docker: false, web: false,
            ports: false, security: false, maint: false, tools: false, footer: false,
        }
    }
}

pub const PRESETS: &[(&str, &str, fn() -> Sections)] = &[
    ("Server-minimal", "system, disk, network, maintenance only", Sections::server_minimal),
    ("Desktop-full", "every section on", Sections::desktop_full),
    ("Docker-host", "docker + ports + security emphasized", Sections::docker_host),
    ("All-off", "clean slate (nothing shown)", Sections::all_off),
];

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct Settings {
    pub sections: Sections,
    pub frame: bool,
    pub color: ColorMode,
    pub live_cpu_sample: bool,
    pub update_counts: bool,
    pub public_ip: bool,
    pub geo_asn: bool,
    pub ssh_only: bool,
    pub script_path: String,
    pub config_path: String,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        let script_path = home_dir()
            .map(|h| h.join("vps-info.sh").to_string_lossy().into_owned())
            .unwrap_or_else(|| "~/vps-info.sh".into());
        let config_path = home_dir()
            .map(|h| h.join(".config/vpsinfo/vpsinfo.conf").to_string_lossy().into_owned())
            .unwrap_or_else(|| "~/.config/vpsinfo/vpsinfo.conf".into());
        Self {
            sections: Sections::default(),
            frame: true,
            color: ColorMode::Auto,
            live_cpu_sample: true,
            update_counts: true,
            public_ip: true,
            geo_asn: true,
            ssh_only: true,
            script_path,
            config_path,
            theme: Theme::Dark,
        }
    }
}

impl Settings {
    pub fn section_flags(&self) -> Vec<(&'static str, bool)> {
        let s = &self.sections;
        vec![
            ("VPSINFO_SHOW_SUMMARY", s.summary),
            ("VPSINFO_SHOW_BANNER", s.banner),
            ("VPSINFO_SHOW_SYSTEM", s.system),
            ("VPSINFO_SHOW_CPU", s.cpu),
            ("VPSINFO_SHOW_MEMORY", s.memory),
            ("VPSINFO_SHOW_DISK", s.disk),
            ("VPSINFO_SHOW_NETWORK", s.network),
            ("VPSINFO_SHOW_PROCESSES", s.processes),
            ("VPSINFO_SHOW_DOCKER", s.docker),
            ("VPSINFO_SHOW_WEB", s.web),
            ("VPSINFO_SHOW_PORTS", s.ports),
            ("VPSINFO_SHOW_SECURITY", s.security),
            ("VPSINFO_SHOW_MAINT", s.maint),
            ("VPSINFO_SHOW_TOOLS", s.tools),
            ("VPSINFO_SHOW_FOOTER", s.footer),
        ]
    }

    /// KEY=VALUE lines for the runtime config file (read by the canonical script).
    pub fn config_lines(&self, profile: &str) -> String {
        let mut out = String::new();
        out.push_str(&format!("# vpsinfo config - generated by vpsinfo-gui (profile: {profile})\n"));
        out.push_str("# Used by vps-info.sh when VPSINFO_NO_CONFIG is not set. Env vars win.\n");
        for (k, v) in self.section_flags() {
            out.push_str(&format!("{k}={}\n", v as u8));
        }
        out.push_str(&format!("VPSINFO_FRAME={}\n", self.frame as u8));
        out.push_str(&format!("VPSINFO_COLOR={}\n", self.color.as_str()));
        out.push_str(&format!("VPSINFO_SKIP_CPU={}\n", (!self.live_cpu_sample) as u8));
        out.push_str(&format!("VPSINFO_SKIP_UPDATES={}\n", (!self.update_counts) as u8));
        out.push_str(&format!("NO_PUBLIC_IP={}\n", (!self.public_ip) as u8));
        out.push_str(&format!("VPSINFO_SKIP_GEO={}\n", (!self.geo_asn) as u8));
        out
    }

    /// `export …` header for the baked/standalone script (VPSINFO_NO_CONFIG=1).
    pub fn baked_header(&self, profile: &str) -> String {
        let mut out = String::new();
        out.push_str("#!/usr/bin/env bash\n");
        out.push_str("# ------------------------------------------------------------------\n");
        out.push_str("# vps-info.sh - baked/standalone export generated by vpsinfo-gui\n");
        out.push_str(&format!("#   profile: {profile}   (regenerate from the GUI to update)\n"));
        out.push_str("#   VPSINFO_NO_CONFIG=1: this export ignores ~/.config/vpsinfo/vpsinfo.conf\n");
        out.push_str("# ------------------------------------------------------------------\n");
        for (k, v) in self.section_flags() {
            out.push_str(&format!("export {k}={}\n", v as u8));
        }
        out.push_str(&format!("export VPSINFO_FRAME={}\n", self.frame as u8));
        out.push_str(&format!("export VPSINFO_COLOR={}\n", self.color.as_str()));
        out.push_str(&format!("export VPSINFO_SKIP_CPU={}\n", (!self.live_cpu_sample) as u8));
        out.push_str(&format!("export VPSINFO_SKIP_UPDATES={}\n", (!self.update_counts) as u8));
        out.push_str(&format!("export NO_PUBLIC_IP={}\n", (!self.public_ip) as u8));
        out.push_str(&format!("export VPSINFO_SKIP_GEO={}\n", (!self.geo_asn) as u8));
        out.push_str("export VPSINFO_NO_CONFIG=1\n");
        out
    }
}

/// Undo/redo snapshot history for settings.
pub struct History {
    stack: Vec<Settings>,
    cursor: usize,
}

impl History {
    pub fn new(initial: Settings) -> Self {
        Self { stack: vec![initial], cursor: 0 }
    }
    pub fn push(&mut self, s: Settings) {
        self.stack.truncate(self.cursor + 1);
        self.stack.push(s);
        if self.stack.len() > 50 {
            self.stack.remove(0);
        }
        self.cursor = self.stack.len() - 1;
    }
    pub fn undo(&mut self, cur: &mut Settings) -> bool {
        if self.cursor > 0 {
            self.cursor -= 1;
            *cur = self.stack[self.cursor].clone();
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, cur: &mut Settings) -> bool {
        if self.cursor + 1 < self.stack.len() {
            self.cursor += 1;
            *cur = self.stack[self.cursor].clone();
            true
        } else {
            false
        }
    }
}

/// Everything we persist across launches (profiles + window/tab state).
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GuiState {
    #[serde(default)]
    pub profiles: BTreeMap<String, Settings>,
    #[serde(default)]
    pub current_profile: String,
    #[serde(default)]
    pub window_rect: Option<[f32; 4]>,
    #[serde(default)]
    pub active_tab: Option<Tab>,
}

impl Default for GuiState {
    fn default() -> Self {
        let mut s = Self {
            profiles: BTreeMap::new(),
            current_profile: "Default".into(),
            window_rect: None,
            active_tab: None,
        };
        s.profiles.insert("Default".into(), Settings::default());
        s
    }
}

impl GuiState {
    pub fn path() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/vpsinfo/gui.json")
    }

    pub fn load() -> Self {
        let p = Self::path();
        if let Ok(txt) = std::fs::read_to_string(&p) {
            if let Ok(mut st) = serde_json::from_str::<GuiState>(&txt) {
                if st.profiles.is_empty() {
                    let mut d = Self::default();
                    d.current_profile = st.current_profile.clone();
                    return d;
                }
                if !st.profiles.contains_key(&st.current_profile) {
                    st.current_profile = st.profiles.keys().next().unwrap().clone();
                }
                return st;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(txt) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, txt);
        }
    }

    pub fn current_settings(&self) -> Settings {
        self.profiles
            .get(&self.current_profile)
            .cloned()
            .unwrap_or_default()
    }
}