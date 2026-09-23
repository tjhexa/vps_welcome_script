//! The eframe application: tabs, settings pane, preview, rc manager, about,
//! diff dialogs, keyboard shortcuts and undo/redo.
use eframe::egui;
use egui::{Color32, ComboBox, Context, Key, Modifiers, RichText, ScrollArea};

use crate::actions::{self, PreviewState};
use crate::preview;
use crate::rc::{self, RcEditPlan};
use crate::settings::{self, GuiState, History, Settings, Tab, Theme};

pub struct DiffDialog {
    pub title: String,
    pub path: String,
    pub hunks: Vec<(String, Vec<(char, String)>)>,
    pub kind: DiffKind,
}

pub enum DiffKind {
    WriteConfig,
    WriteBaked,
    RcAdd(Vec<RcEditPlan>),
    RcRemove(Vec<RcEditPlan>),
}

pub struct App {
    pub settings: Settings,
    pub history: History,
    pub gui: GuiState,
    pub active_tab: Tab,
    pub rc_files: Vec<rc::RcFile>,
    pub show_help: bool,
    pub show_about: bool,
    pub diff: Option<DiffDialog>,
    pub status: Option<(Color32, String)>,
    pub real_preview: PreviewState,
    pub last_snapshot: Settings,
    pub suppress_history: bool,
    pub applied_theme: Option<Theme>,
    pub window_rect: Option<[f32; 4]>,
    pub new_profile_name: String,
    pub preset_pending: Option<fn() -> crate::settings::Sections>,
    pub profile_json_path: String,
    pub pending_screenshot: bool,
    pub wizard_step: usize,
    pub wizard_purpose: usize,
    pub wizard_draft: crate::settings::Sections,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, gui: GuiState) -> Self {
        install_fonts(&cc.egui_ctx);
        let settings = gui.current_settings();
        let theme = settings.theme;
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        if theme == Theme::Light {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
        }
        let active_tab = gui.active_tab.unwrap_or(Tab::Settings);
        let history = History::new(settings.clone());
        let rc_files = rc::candidates();
        let last_snapshot = settings.clone();
        Self {
            settings,
            history,
            gui,
            active_tab,
            rc_files,
            show_help: false,
            show_about: false,
            diff: None,
            status: None,
            real_preview: PreviewState::Idle,
            last_snapshot,
            suppress_history: false,
            applied_theme: Some(theme),
            window_rect: None,
            new_profile_name: String::new(),
            preset_pending: None,
            profile_json_path: settings::expand_tilde("~/.config/vpsinfo/profile.json"),
            pending_screenshot: false,
            wizard_step: 0,
            wizard_purpose: 0,
            wizard_draft: settings::Sections::docker_host(),
        }
    }

    // ---------------------------------------------------------------- helpers

    fn set_status(&mut self, color: Color32, text: impl Into<String>) {
        self.status = Some((color, text.into()));
    }

    fn set_visuals_for_theme(&mut self, ctx: &Context) {
        if self.applied_theme != Some(self.settings.theme) {
            match self.settings.theme {
                Theme::Dark => ctx.set_visuals(egui::Visuals::dark()),
                Theme::Light => ctx.set_visuals(egui::Visuals::light()),
            }
            self.applied_theme = Some(self.settings.theme);
        }
    }

    fn switch_profile(&mut self, name: String) {
        if let Some(s) = self.gui.profiles.get(&name).cloned() {
            self.gui.current_profile = name;
            self.settings = s;
            self.history = History::new(self.settings.clone());
            self.last_snapshot = self.settings.clone();
            self.gui.save();
        }
    }

    fn save_current_profile(&mut self) {
        self.gui
            .profiles
            .insert(self.gui.current_profile.clone(), self.settings.clone());
        self.gui.save();
    }

    fn refresh_rc(&mut self) {
        let selected: Vec<bool> = self.rc_files.iter().map(|f| f.selected).collect();
        self.rc_files = rc::candidates();
        for (i, f) in self.rc_files.iter_mut().enumerate() {
            f.selected = selected.get(i).copied().unwrap_or(f.selected);
        }
    }

    fn start_preview(&mut self) {
        self.active_tab = Tab::Preview;
        self.real_preview = actions::start_preview(&self.settings);
    }

    // ---------------------------------------------------------------- actions

    fn request_generate(&mut self) {
        let p = settings::expand_tilde(&self.settings.config_path);
        let old = std::fs::read_to_string(&p).unwrap_or_default();
        let new = self.settings.config_lines(&self.gui.current_profile);
        if std::path::Path::new(&p).exists() && !old.is_empty() {
            self.diff = Some(DiffDialog {
                title: "Overwrite config file?".to_string(),
                path: p,
                hunks: write_hunks(&old, &new),
                kind: DiffKind::WriteConfig,
            });
        } else {
            match actions::generate_config(&self.settings, &self.gui.current_profile, &p) {
                Ok(()) => self.set_status(preview::GREEN, format!("config written to {p}")),
                Err(e) => self.set_status(preview::RED, format!("generate config: {e}")),
            }
        }
    }

    fn request_export(&mut self) {
        let p = settings::expand_tilde(&self.settings.script_path);
        let old = std::fs::read_to_string(&p).unwrap_or_default();
        let new = actions::export_baked_text(&self.settings, &self.gui.current_profile);
        if std::path::Path::new(&p).exists() && !old.is_empty() {
            self.diff = Some(DiffDialog {
                title: "Overwrite existing script?".to_string(),
                path: p,
                hunks: write_hunks(&old, &new),
                kind: DiffKind::WriteBaked,
            });
        } else {
            match actions::export_baked(&self.settings, &self.gui.current_profile, &p) {
                Ok(()) => self.set_status(preview::GREEN, format!("baked script written to {p}")),
                Err(e) => self.set_status(preview::RED, format!("export baked: {e}")),
            }
        }
    }

    fn request_rc_add(&mut self) {
        let mut plans: Vec<RcEditPlan> = Vec::new();
        for rf in &self.rc_files {
            if rf.selected || (rf.create && !rf.exists) {
                plans.push(rc::plan_add(&rf.path, &self.settings.script_path, self.settings.ssh_only));
            }
        }
        if plans.is_empty() {
            self.set_status(preview::YELLOW, "select at least one rc file (or tick “create missing”)");
            return;
        }
        let mut hunks = Vec::new();
        for p in &plans {
            let old = std::fs::read_to_string(&p.path).unwrap_or_default();
            let label = p
                .path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.path.display().to_string());
            hunks.push((label, rc::diff_lines(&old, &p.after, 30)));
        }
        self.diff = Some(DiffDialog {
            title: format!("Add vpsinfo to {} rc file(s) — confirm", plans.len()),
            path: "(multiple files, see below)".into(),
            hunks,
            kind: DiffKind::RcAdd(plans),
        });
    }

    fn request_rc_remove(&mut self) {
        let mut plans: Vec<RcEditPlan> = Vec::new();
        for rf in &self.rc_files {
            if rf.selected && rf.sourced {
                plans.push(rc::plan_remove(&rf.path));
            }
        }
        if plans.is_empty() {
            self.set_status(preview::YELLOW, "no selected rc file has a vpsinfo block to remove");
            return;
        }
        let mut hunks = Vec::new();
        for p in &plans {
            let old = std::fs::read_to_string(&p.path).unwrap_or_default();
            let label = p
                .path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.path.display().to_string());
            hunks.push((label, rc::diff_lines(&old, &p.after, 30)));
        }
        self.diff = Some(DiffDialog {
            title: format!("Remove vpsinfo block from {} rc file(s) — confirm", plans.len()),
            path: "(multiple files, see below)".into(),
            hunks,
            kind: DiffKind::RcRemove(plans),
        });
    }

    fn apply_diff(&mut self) {
        let Some(d) = &self.diff else { return };
        let path = d.path.clone();
        let profile = self.gui.current_profile.clone();
        match &d.kind {
            DiffKind::WriteConfig => match actions::generate_config(&self.settings, &profile, &path) {
                Ok(()) => self.set_status(preview::GREEN, format!("config written to {path}")),
                Err(e) => self.set_status(preview::RED, format!("generate config: {e}")),
            },
            DiffKind::WriteBaked => match actions::export_baked(&self.settings, &profile, &path) {
                Ok(()) => self.set_status(preview::GREEN, format!("baked script written to {path}")),
                Err(e) => self.set_status(preview::RED, format!("export baked: {e}")),
            },
            DiffKind::RcAdd(plans) | DiffKind::RcRemove(plans) => {
                let mut n = 0usize;
                let mut first_err: Option<String> = None;
                for plan in plans {
                    match rc::apply_plan(plan, true) {
                        Ok(()) => n += 1,
                        Err(e) => {
                            if first_err.is_none() {
                                first_err = Some(e.to_string());
                            }
                        }
                    }
                }
                match first_err {
                    Some(e) => self.set_status(preview::RED, format!("rc edit: {e} ({n} ok)")),
                    None => self.set_status(preview::GREEN, format!("rc updated ({n} file(s))")),
                }
                self.refresh_rc();
            }
        }
        self.diff = None;
    }

    // ---------------------------------------------------------------- update

    pub fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.real_preview.poll(ctx);
        self.set_visuals_for_theme(ctx);

        // keyboard shortcuts
        let mut do_undo = false;
        let mut do_redo = false;
        let mut do_generate = false;
        let mut do_preview = false;
        let mut zoom_in = false;
        let mut zoom_out = false;
        let mut zoom_reset = false;
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Z) {
                do_redo = true;
            }
            if i.consume_key(Modifiers::CTRL, Key::Z) {
                do_undo = true;
            }
            if i.consume_key(Modifiers::CTRL, Key::Y) {
                do_redo = true;
            }
            if i.consume_key(Modifiers::CTRL, Key::S) {
                do_generate = true;
            }
            if i.consume_key(Modifiers::CTRL, Key::Enter) {
                do_preview = true;
            }
            if i.modifiers.ctrl && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)) {
                zoom_in = true;
            }
            if i.modifiers.ctrl && i.key_pressed(Key::Minus) {
                zoom_out = true;
            }
            if i.modifiers.ctrl && i.key_pressed(Key::Num0) {
                zoom_reset = true;
            }
            if i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if t == "?")) {
                self.show_help = true;
            }
        });
        if zoom_in || zoom_out || zoom_reset {
            let step = if zoom_in { 1.0 } else if zoom_out { -1.0 } else { 0.0 };
            let base = self.gui.preview_font_size + step;
            let next = if zoom_reset {
                13.0
            } else {
                base.clamp(8.0, 26.0)
            };
            if (next - self.gui.preview_font_size).abs() > f32::EPSILON {
                self.gui.preview_font_size = next;
                self.gui.save();
            }
        }
        if do_undo {
            if self.history.undo(&mut self.settings) {
                self.set_status(preview::DIM, "undone (Ctrl+Z)");
                self.suppress_history = true;
            }
        }
        if do_redo {
            if self.history.redo(&mut self.settings) {
                self.set_status(preview::DIM, "redone (Ctrl+Shift+Z)");
                self.suppress_history = true;
            }
        }
        if do_generate {
            self.request_generate();
        }
        if do_preview {
            self.start_preview();
        }

        // menu bar + theme toggle
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Generate config…").clicked() {
                        ui.close_menu();
                        self.request_generate();
                    }
                    if ui.button("Export baked script…").clicked() {
                        ui.close_menu();
                        self.request_export();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.close_menu();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Shortcuts & help").clicked() {
                        self.show_help = true;
                        ui.close_menu();
                    }
                    if ui.button("About").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let light = self.settings.theme == Theme::Light;
                    let label = if light { "Dark mode" } else { "Light mode" };
                    if ui
                        .small_button(label)
                        .on_hover_text("Toggle dark/light theme")
                        .clicked()
                    {
                        self.settings.theme = if light { Theme::Dark } else { Theme::Light };
                    }
                });
            });
        });

        // tab bar
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                for tab in [Tab::Settings, Tab::Preview, Tab::RcManager, Tab::About, Tab::Wizard] {
                    if ui.selectable_label(self.active_tab == tab, tab.label()).clicked() {
                        self.active_tab = tab;
                    }
                }
            });
            ui.add_space(2.0);
        });

        // bottom status/attribution bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                match &self.status {
                    Some((c, s)) => {
                        let pill = egui::Frame::default()
                            .fill(c.gamma_multiply(0.15))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(8, 2));
                        pill.show(ui, |ui| {
                            ui.colored_label(*c, s);
                        });
                    }
                    None => {
                        ui.colored_label(preview::DIM, "ready — hover any toggle for help");
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("vps-info · github.com/tjhexa/vps_welcome_script · MIT")
                            .small()
                            .weak(),
                    );
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.active_tab {
            Tab::Settings => self.settings_ui(ui),
            Tab::Preview => self.preview_ui(ui),
            Tab::RcManager => self.rc_ui(ui),
            Tab::About => self.about_ui(ui),
            Tab::Wizard => self.wizard_ui(ui),
        });

        if self.show_help {
            self.help_window(ctx);
        }
        if self.show_about {
            self.about_window(ctx);
        }
        self.diff_window(ctx);

        // V14: screenshot-to-PNG (request once, save when the event arrives)
        if self.pending_screenshot {
            self.pending_screenshot = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.input(|i| {
            for ev in &i.events {
                if let egui::Event::Screenshot { image, .. } = ev {
                    if let Some(p) = self.save_screenshot(&image) {
                        self.set_status(preview::GREEN, format!("preview saved to {p} (path copied to clipboard)"));
                        ctx.copy_text(p.clone());
                    } else {
                        self.set_status(preview::RED, "could not write preview PNG");
                    }
                }
            }
        });

        if !self.gui.first_run_dismissed {
            self.first_run_window(ctx);
        }

        // history recording + persistence
        if self.suppress_history {
            self.suppress_history = false;
            self.last_snapshot = self.settings.clone();
        } else if self.settings != self.last_snapshot {
            self.history.push(self.settings.clone());
            self.last_snapshot = self.settings.clone();
            self.save_current_profile();
        }

        // capture window rect for persistence
        let r = ctx.input(|i| i.viewport().outer_rect.or(i.viewport().inner_rect));
        if let Some(r) = r {
            self.window_rect = Some([r.min.x, r.min.y, r.width(), r.height()]);
        }
    }

    // ---------------------------------------------------------------- settings tab

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(2.0);
            self.profiles_row(ui);
            ui.separator();
            self.presets_row(ui);
            ui.separator();

            ui.heading("Settings");
            ui.label(RichText::new("Groups collapse to keep the tab short; state is remembered.").weak());
            ui.add_space(4.0);

            let mut cg = self.gui.collapsed;

            // ---- Sections ----
            let open = !cg.sections;
            let resp = egui::CollapsingHeader::new("Sections — the 15 banner blocks")
                .id_salt("col-sections")
                .open(Some(open))
                .show(ui, |ui| {
                    ui.label(RichText::new("Each maps to a VPSINFO_SHOW_* flag on the generated config / baked script").weak());
                    ui.add_space(4.0);
                    self.sections_grid(ui);
                    ui.add_space(8.0);
                });
            if resp.header_response.clicked() {
                cg.sections = !cg.sections;
            }

            // ---- Appearance ----
            let open = !cg.appearance;
            let resp = egui::CollapsingHeader::new("Appearance — colors, frame, theme")
                .id_salt("col-appearance")
                .open(Some(open))
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Color mode:");
                        self.color_mode_combo(ui);
                        ui.checkbox(&mut self.settings.frame, "Frame around output")
                            .on_hover_text("Draw the screenfetch-style box (VPSINFO_FRAME)");
                    });
                    ui.add_space(8.0);
                });
            if resp.header_response.clicked() {
                cg.appearance = !cg.appearance;
            }

            // ---- Behavior ----
            let open = !cg.behavior;
            let resp = egui::CollapsingHeader::new("Behavior — scan cost & scope")
                .id_salt("col-behavior")
                .open(Some(open))
                .show(ui, |ui| {
                    ui.label(RichText::new("Cost/scope of the scans — maps to VPSINFO_SKIP_* / NO_PUBLIC_IP / VPSINFO_SKIP_GEO").weak());
                    ui.add_space(4.0);
                    self.behavior_ui(ui);
                    ui.add_space(8.0);
                });
            if resp.header_response.clicked() {
                cg.behavior = !cg.behavior;
            }

            // ---- Paths ----
            let open = !cg.paths;
            let resp = egui::CollapsingHeader::new("Paths — where things get written")
                .id_salt("col-paths")
                .open(Some(open))
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.label("Baked export + rc hook target:");
                    ui.add(egui::TextEdit::singleline(&mut self.settings.script_path).hint_text("~/vps-info.sh"));
                    ui.label("Runtime config file (read by the canonical script):");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.settings.config_path)
                            .hint_text("~/.config/vpsinfo/vpsinfo.conf"),
                    );
                    ui.add_space(8.0);
                });
            if resp.header_response.clicked() {
                cg.paths = !cg.paths;
            }

            if cg != self.gui.collapsed {
                self.gui.collapsed = cg;
                self.gui.save();
            }

            ui.separator();
            self.defaults_row(ui);
            ui.add_space(4.0);
            self.output_buttons(ui);
            ui.add_space(4.0);
            ui.label(
                RichText::new("Shortcuts:  Ctrl+S generate · Ctrl+Enter real preview · Ctrl+Z undo / Ctrl+Shift+Z redo · Ctrl +/- preview zoom · ? help")
                    .weak(),
            );
        });
    }

    /// V4 (#19): reset-to-defaults button + “N settings differ from defaults” badge.
    fn defaults_row(&mut self, ui: &mut egui::Ui) {
        let n = self.settings.diff_count();
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(RichText::new("Reset to defaults").strong())
                .on_hover_text("Restore every value to the factory defaults (paths are kept)")
                .clicked()
            {
                let mut d = settings::Settings::default();
                d.script_path = self.settings.script_path.clone();
                d.config_path = self.settings.config_path.clone();
                self.settings = d;
                self.set_status(preview::GREEN, "reset to defaults");
            }
            if n > 0 {
                ui.colored_label(preview::YELLOW, format!("{n} setting(s) differ from defaults"));
            } else {
                ui.colored_label(preview::DIM, "matches defaults");
            }
        });
    }

    fn profiles_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Profile:");
            let names: Vec<String> = self.gui.profiles.keys().cloned().collect();
            let cur = self.gui.current_profile.clone();
            ComboBox::from_id_salt("profile")
                .selected_text(&cur)
                .show_ui(ui, |ui| {
                    for n in &names {
                        let sel = *n == cur;
                        if ui.selectable_label(sel, n).clicked() {
                            self.switch_profile(n.clone());
                        }
                    }
                });
            ui.add(
                egui::TextEdit::singleline(&mut self.new_profile_name)
                    .desired_width(110.0)
                    .hint_text("new profile"),
            );
            let add_ok = !self.new_profile_name.trim().is_empty()
                && !self.gui.profiles.contains_key(self.new_profile_name.trim());
            if ui
                .add_enabled(add_ok, egui::Button::new("Save as"))
                .on_hover_text("Save current settings as a new named profile")
                .clicked()
            {
                let name = self.new_profile_name.trim().to_string();
                self.gui.profiles.insert(name.clone(), self.settings.clone());
                self.gui.current_profile = name;
                self.new_profile_name.clear();
                self.gui.save();
                self.set_status(preview::GREEN, "profile saved");
            }
            if ui
                .add_enabled(self.gui.profiles.len() > 1, egui::Button::new("Delete"))
                .on_hover_text("Delete the current profile")
                .clicked()
            {
                let cur = self.gui.current_profile.clone();
                self.gui.profiles.remove(&cur);
                self.gui.current_profile = self.gui.profiles.keys().next().unwrap().clone();
                self.switch_profile(self.gui.current_profile.clone());
            }
            if ui.button("Save").on_hover_text("Save current settings into this profile").clicked() {
                self.save_current_profile();
                self.set_status(preview::GREEN, "profile saved");
            }
            ui.separator();
            ui.label(RichText::new("Copy to another machine:").weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.profile_json_path)
                    .desired_width(220.0)
                    .hint_text("profile.json path"),
            );
            if ui
                .button("Export JSON")
                .on_hover_text("Write this profile to a JSON file you can copy to another machine")
                .clicked()
            {
                let p = settings::expand_tilde(&self.profile_json_path);
                match self.settings.to_json() {
                    Ok(txt) => {
                        if let Some(dir) = std::path::Path::new(&p).parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        match std::fs::write(&p, txt) {
                            Ok(()) => self.set_status(preview::GREEN, format!("profile JSON written to {p}")),
                            Err(e) => self.set_status(preview::RED, format!("export json: {e}")),
                        }
                    }
                    Err(e) => self.set_status(preview::RED, format!("export json: {e}")),
                }
            }
            if ui
                .button("Import JSON")
                .on_hover_text("Load a profile from a JSON file — added as a new profile")
                .clicked()
            {
                let p = settings::expand_tilde(&self.profile_json_path);
                match std::fs::read_to_string(&p)
                    .ok()
                    .and_then(|t| settings::Settings::from_json(&t))
                {
                    Some(s) => {
                        let base = if self.new_profile_name.trim().is_empty() {
                            "Imported".to_string()
                        } else {
                            self.new_profile_name.trim().to_string()
                        };
                        let mut name = base.clone();
                        let mut i = 1;
                        while self.gui.profiles.contains_key(&name) {
                            name = format!("{base} {}", i);
                            i += 1;
                        }
                        self.gui.profiles.insert(name.clone(), s.clone());
                        self.gui.current_profile = name.clone();
                        self.gui.save();
                        self.switch_profile(name.clone());
                        self.new_profile_name.clear();
                        self.set_status(preview::GREEN, format!("imported profile from {p}"));
                    }
                    None => self.set_status(preview::RED, format!("could not parse {p} as a vpsinfo profile JSON")),
                }
            }
        });
    }

    fn presets_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Preset:");
            ComboBox::from_id_salt("preset")
                .selected_text("choose…")
                .show_ui(ui, |ui| {
                    for (name, desc, f) in settings::PRESETS {
                        if ui
                            .selectable_label(false, format!("{name} — {desc}"))
                            .clicked()
                        {
                            self.preset_pending = Some(*f);
                        }
                    }
                });
            ui.label(RichText::new("one click loads a ready section set").weak());
        });
        self.apply_pending_preset(ui);
    }

    fn apply_pending_preset(&mut self, ui: &mut egui::Ui) {
        let _ = ui;
        if let Some(f) = self.preset_pending.take() {
            self.settings.sections = f();
            self.set_status(preview::GREEN, "preset applied");
            self.save_current_profile();
        }
    }

    /// V14 (#30): save the window screenshot as PNG (path returned for status).
    fn save_screenshot(&self, img: &egui::ColorImage) -> Option<String> {
        let (w, h) = (img.size[0], img.size[1]);
        if w == 0 || h == 0 {
            return None;
        }
        let mut rgb = Vec::with_capacity(w * h * 3);
        for p in img.pixels.iter() {
            rgb.extend_from_slice(&[p.r(), p.g(), p.b()]);
        }
        let buf = image::RgbImage::from_raw(w as u32, h as u32, rgb)?;
        let path = settings::expand_tilde("~/.config/vpsinfo/preview.png");
        if let Some(dir) = std::path::Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        buf.save(&path).ok()?;
        Some(path)
    }

    /// V7 (#26): one-time welcome popup with a “don't show again” checkbox.
    fn first_run_window(&mut self, ctx: &Context) {
        let mut open = true;
        let mut got_started = false;
        let mut dont = false;
        egui::Window::new("Welcome to vpsinfo-gui")
            .id(egui::Id::new("welcome"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -40.0])
            .show(ctx, |ui| {
                ui.heading("Welcome to vpsinfo-gui");
                ui.add_space(4.0);
                ui.label("A one-stop switcher for the vps-info login banner: ");
                ui.label("• toggle the 15 sections, colors and frame on the Settings tab");
                ui.label("• Preview shows a live mock and can run the real script");
                ui.label("• rc Manager installs/removes the SSH-login hook safely");
                ui.label("• Generate config / Export baked script at any time");
                ui.add_space(8.0);
                ui.label(RichText::new("Tip: pick a preset (e.g. Docker-host) to start from a sensible section set.").weak());
                ui.add_space(8.0);
                ui.checkbox(&mut dont, "Don't show this again");
                ui.add_space(4.0);
                if ui.button(RichText::new("Get started").strong()).clicked() {
                    got_started = true;
                }
            });
        if got_started || !open {
            // Shown only while first_run_dismissed == false; persist the
            // user's choice when the window closes (X or Get started).
            self.gui.first_run_dismissed = dont;
            self.gui.save();
        }
    }

    fn color_mode_combo(&mut self, ui: &mut egui::Ui) {
        ComboBox::from_id_salt("color")
            .selected_text(match self.settings.color {
                settings::ColorMode::Auto => "auto (terminal only)",
                settings::ColorMode::Always => "always",
                settings::ColorMode::Never => "never",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.color, settings::ColorMode::Auto, "auto — colors only on a real terminal");
                ui.selectable_value(&mut self.settings.color, settings::ColorMode::Always, "always — force ANSI colors");
                ui.selectable_value(&mut self.settings.color, settings::ColorMode::Never, "never — plain text");
            });
    }

    fn sections_grid(&mut self, ui: &mut egui::Ui) {
        sections_grid(ui, &mut self.settings.sections);
    }

    fn behavior_ui(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        ui.checkbox(&mut s.live_cpu_sample, "Live CPU sample")
            .on_hover_text("Samples /proc/stat for ~0.2s. Off maps to VPSINFO_SKIP_CPU=1.");
        ui.checkbox(&mut s.update_counts, "Update counts")
            .on_hover_text("apt/dnf/dpkg pending-update lookup (can take a moment). Off maps to VPSINFO_SKIP_UPDATES=1.");
        ui.checkbox(&mut s.public_ip, "Public IP + Geo")
            .on_hover_text("Fetches the public IP (cached for 1h). Off maps to NO_PUBLIC_IP=1.");
        ui.horizontal(|ui| {
            let enabled = s.public_ip;
            ui.add_enabled_ui(enabled, |ui| {
                ui.checkbox(&mut s.geo_asn, "Geo / ASN detail")
                    .on_hover_text("City/region/country/ASN lookup for the public IP. Off maps to VPSINFO_SKIP_GEO=1.");
            });
        });
        ui.checkbox(&mut s.ssh_only, "SSH-only rc guard")
            .on_hover_text("Wraps the rc hook in `if [ -n \"${SSH_CONNECTION:-}\" ]` so the banner only prints for SSH logins.");
    }

    fn output_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(RichText::new("Generate config").strong())
                .on_hover_text("Write the runtime config file (KEY=VALUE) read by vps-info.sh · Ctrl+S")
                .clicked()
            {
                self.request_generate();
            }
            if ui
                .button(RichText::new("Export baked script").strong())
                .on_hover_text("Write a standalone script with these values embedded (VPSINFO_NO_CONFIG=1)")
                .clicked()
            {
                self.request_export();
            }
        });
    }

    // ---------------------------------------------------------------- preview tab

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Run real preview")
                .on_hover_text("Execute the embedded script with the current settings (8s timeout, network off) · Ctrl+Enter")
                .clicked()
            {
                self.start_preview();
            }
            if ui.button("Clear").clicked() {
                self.real_preview = PreviewState::Idle;
            }
            if let PreviewState::Running { .. } = self.real_preview {
                ui.spinner();
                ui.label(RichText::new("running…").weak());
            }
            ui.separator();
            let fs = self.gui.preview_font_size;
            if ui.small_button("Zoom -").on_hover_text("Ctrl+-").clicked() {
                self.gui.preview_font_size = (fs - 1.0).clamp(8.0, 26.0);
                self.gui.save();
            }
            if ui.small_button("Zoom +").on_hover_text("Ctrl+=").clicked() {
                self.gui.preview_font_size = (fs + 1.0).clamp(8.0, 26.0);
                self.gui.save();
            }
            if ui.small_button("100%").on_hover_text("Ctrl+0").clicked() {
                self.gui.preview_font_size = 13.0;
                self.gui.save();
            }
            ui.separator();
            if ui
                .button("Save PNG")
                .on_hover_text("Screenshot the window to ~/.config/vpsinfo/preview.png and copy the path")
                .clicked()
            {
                self.pending_screenshot = true;
            }
        });

        // V2 (#5): live colour strip — same thresholds as the real script.
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            swatch(ui, preview::GREEN, "5 <= healthy (<60%)");
            swatch(ui, preview::YELLOW, "6 = caution (60-79%)");
            swatch(ui, preview::RED, "7 = critical (80%+)");
            ui.label(RichText::new("— colours match the real script's thresholds").weak());
        });
        ui.add_space(6.0);

        let (job, width) = match &self.real_preview {
            PreviewState::Idle | PreviewState::Running { .. } => {
                let lines = preview::build_mock(&self.settings.sections, self.settings.frame);
                let w = preview::mock_max_width(&lines);
                let job = preview::lines_to_job(&lines, self.gui.preview_font_size);
                (job, w)
            }
            PreviewState::Done(o) => {
                let w = preview::ansi_max_width(&o.text);
                let job = preview::ansi_to_job(&o.text, self.gui.preview_font_size);
                (job, w)
            }
        };

        // V6 (#23): wrap-aware preview
        let cols: usize = std::env::var("COLUMNS")
            .ok()
            .and_then(|c| c.parse().ok())
            .unwrap_or(80);
        if width > cols {
            ui.colored_label(
                preview::YELLOW,
                format!("banner is {width} columns wide — will soft-wrap on a {cols}-column terminal; reduce sections or widen the terminal"),
            );
        } else {
            ui.colored_label(preview::DIM, format!("{width} columns wide — fits a {cols}-column terminal"));
        }
        ui.add_space(6.0);

        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(egui::Label::new(job));
                if let PreviewState::Running { .. } = self.real_preview {
                    ui.add_space(8.0);
                    ui.label(RichText::new("…processing (max 8s)").weak());
                }
            });
        if let PreviewState::Done(o) = &self.real_preview {
            if o.timed_out {
                ui.colored_label(preview::YELLOW, "preview timed out after 8s — some scans may be missing");
            }
            if let Some(e) = &o.error {
                if !e.trim().is_empty() {
                    ui.colored_label(preview::RED, e);
                }
            }
        }
    }

    // ---------------------------------------------------------------- rc tab

    fn rc_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("rc files");
        ui.label(
            RichText::new("Tick the files to edit. “sourced” means the vpsinfo hook is already installed.")
                .weak(),
        );
        ui.add_space(4.0);

        ScrollArea::vertical()
            .id_salt("rc_files")
            .auto_shrink([false, false])
            .max_height(220.0)
            .show(ui, |ui| {
                for i in 0..self.rc_files.len() {
                    let path = self.rc_files[i].path.clone();
                    let label = self.rc_files[i].label();
                    let exists = self.rc_files[i].exists;
                    let sourced = self.rc_files[i].sourced;
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.rc_files[i].selected, label.clone())
                            .on_hover_text(path.display().to_string());
                        if exists {
                            if sourced {
                                ui.colored_label(preview::GREEN, "sourced");
                            } else {
                                ui.colored_label(preview::YELLOW, "not sourced");
                            }
                        } else {
                            ui.colored_label(preview::DIM, "missing");
                        }
                        let baks = rc::backups(&path);
                        if !baks.is_empty() {
                            ui.menu_button(format!("backups ({})", baks.len()), |ui| {
                                for b in baks {
                                    let fname = b
                                        .file_name()
                                        .map(|s| s.to_string_lossy().into_owned())
                                        .unwrap_or_default();
                                    if ui.button(format!("Restore {fname}")).clicked() {
                                        match rc::restore(&path, &b) {
                                            Ok(()) => self.set_status(preview::GREEN, format!("restored {label} from {fname}")),
                                            Err(e) => self.set_status(preview::RED, format!("restore {label}: {e}")),
                                        }
                                        self.refresh_rc();
                                        ui.close_menu();
                                    }
                                }
                            });
                        }
                    });
                    if !exists {
                        ui.horizontal(|ui| {
                            ui.add_space(18.0);
                            ui.checkbox(&mut self.rc_files[i].create, format!("create {} if missing", label))
                                .on_hover_text("Create the file (e.g. ~/.zshrc) and install the block");
                        });
                    }
                }
            });
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.settings.ssh_only, "SSH-only guard")
                .on_hover_text("Only run the banner for remote SSH logins (wraps the hook in an SSH_CONNECTION check)");
        });

        // rc-block preview (what will be appended)
        let block = rc::build_block(&self.settings.script_path, self.settings.ssh_only);
        ui.add_space(6.0);
        ui.label("rc-block preview (exactly what gets appended):");
        ScrollArea::both()
            .id_salt("rc_block_preview")
            .max_height(150.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::group(ui.style())
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.monospace(block);
                    });
            });

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(RichText::new("Add to rc…").strong())
                .on_hover_text("Preview the diff then append the hook block to the selected files (backup first)")
                .clicked()
            {
                self.request_rc_add();
            }
            if ui
                .button(RichText::new("Remove rc block…").strong())
                .on_hover_text("Strip only our marker block from the selected files (backup first)")
                .clicked()
            {
                self.request_rc_remove();
            }
        });
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Whole machine (needs sudo):").weak());
            if ui
                .button("Install system-wide…")
                .on_hover_text("Install the baked script to /usr/local/bin and hook /etc/profile.d for SSH logins (asks for sudo)")
                .clicked()
            {
                match actions::install_system(&self.settings) {
                    Ok(msg) => self.set_status(preview::GREEN, msg),
                    Err(e) => self.set_status(preview::RED, format!("install system-wide: {e}")),
                }
            }
            if ui
                .button("Remove system-wide")
                .on_hover_text("Remove /usr/local/bin/vpsinfo and the /etc/profile.d hook (asks for sudo)")
                .clicked()
            {
                match actions::uninstall_system() {
                    Ok(msg) => self.set_status(preview::GREEN, msg),
                    Err(e) => self.set_status(preview::RED, format!("remove system-wide: {e}")),
                }
            }
        });
    }

    // ---------------------------------------------------------------- about tab

    fn about_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading("vpsinfo-gui");
        ui.label(RichText::new("Configure, preview and install the vps-info banner — with an rc-file manager.").weak());
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Copy install snippet")
                .on_hover_text("Copy the README hook snippet with your current script path to the clipboard")
                .clicked()
            {
                let path = settings::expand_tilde(&self.settings.script_path);
                let snippet = format!(
                    "echo '{path}' >> ~/.bashrc   # permanent SSH-login banner\n# or: echo 'bash {path}' >> ~/.bashrc\n# remove later with: vpsinfo-gui --rc-remove"
                );
                ui.ctx().copy_text(snippet);
                self.set_status(preview::GREEN, "install snippet copied to clipboard");
            }
            if ui
                .button("Copy config path")
                .on_hover_text("Copy the runtime config file path to the clipboard")
                .clicked()
            {
                let p = settings::expand_tilde(&self.settings.config_path);
                ui.ctx().copy_text(p);
                self.set_status(preview::GREEN, "config path copied to clipboard");
            }
        });
        ui.add_space(8.0);
        about_text(ui);
    }

    fn help_window(&mut self, ctx: &Context) {
        let mut open = self.show_help;
        egui::Window::new("Help & shortcuts")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(430.0)
            .show(ctx, |ui| {
                ui.label("Keyboard");
                egui::Grid::new("help_grid")
                    .num_columns(2)
                    .spacing([16.0, 4.0])
                    .show(ui, |ui| {
                        for (k, d) in [
                            ("Space", "toggles the focused checkbox"),
                            ("?", "this help window"),
                            ("Ctrl+S", "generate the config file"),
                            ("Ctrl+Enter", "run the real preview"),
                            ("Ctrl+= / Ctrl+- / Ctrl+0", "preview zoom in / out / reset"),
                            ("Ctrl+Z", "undo settings change"),
                            ("Ctrl+Shift+Z / Ctrl+Y", "redo settings change"),
                        ] {
                            ui.monospace(k);
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                ui.label("Everything else");
                ui.label("• Hover any toggle for a one-line explanation (tooltips).");
                ui.label("• The monospace preview uses the Hack font bundled inside the binary.");
                ui.label("• rc edits are idempotent (marker block) and backed up before every change.");
            });
        self.show_help = open;
    }

    fn about_window(&mut self, ctx: &Context) {
        let mut open = self.show_about;
        egui::Window::new("About")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(430.0)
            .show(ctx, |ui| {
                about_text(ui);
            });
        self.show_about = open;
    }

    // ---------------------------------------------------------------- diff dialog

    fn diff_window(&mut self, ctx: &Context) {
        let mut want_apply = false;
        let mut want_close = false;
        if let Some(d) = &mut self.diff {
            let mut open = true;
            egui::Window::new(d.title.clone())
                .id(egui::Id::new("diff_dialog"))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(620.0)
                .show(ctx, |ui| {
                    if !d.path.contains("multiple files") {
                        ui.horizontal(|ui| {
                            ui.label("Path:");
                            ui.add(
                                egui::TextEdit::singleline(&mut d.path)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    } else {
                        ui.label(RichText::new(&d.path).weak());
                    }
                    ui.add_space(6.0);
                    ScrollArea::both()
                        .max_height(340.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (label, lines) in &d.hunks {
                                ui.label(RichText::new(label).strong());
                                egui::Frame::group(ui.style()).show(ui, |ui| {
                                    for (sign, l) in lines {
                                        let (c, pfx) = match sign {
                                            '+' => (preview::GREEN, "+ "),
                                            '-' => (preview::RED, "- "),
                                            _ => (egui::Color32::from_gray(0x9a), "  "),
                                        };
                                        ui.horizontal(|ui| {
                                            ui.monospace(RichText::new(pfx).color(c));
                                            ui.monospace(RichText::new(l).color(c));
                                        });
                                    }
                                });
                                ui.add_space(6.0);
                            }
                        });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Apply").strong()).clicked() {
                            want_apply = true;
                        }
                        if ui.button("Cancel").clicked() {
                            want_close = true;
                        }
                    });
                });
            if !open {
                want_close = true;
            }
        }
        if want_apply {
            self.apply_diff();
        }
        if want_close && self.diff.is_some() {
            self.diff = None;
        }
    }

    // ---------------------------------------------------------------- wizard (#24)

    fn wizard_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Setup wizard");
        ui.label(
            RichText::new("Three steps: pick a purpose, trim the sections, confirm — then finish on the Settings tab.")
                .weak(),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (i, name) in ["1 · Purpose", "2 · Sections", "3 · Confirm"].iter().enumerate() {
                let _ = ui.selectable_label(self.wizard_step == i, *name);
            }
        });
        ui.separator();
        match self.wizard_step {
            0 => self.wizard_purpose_ui(ui),
            1 => self.wizard_sections_ui(ui),
            _ => self.wizard_confirm_ui(ui),
        }
        ui.add_space(10.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.wizard_step > 0, egui::Button::new("Back"))
                .clicked()
            {
                self.wizard_step -= 1;
            }
            let last = self.wizard_step == 2;
            let label = if last { "Apply & go to Settings" } else { "Next" };
            if ui
                .add_enabled(
                    last || self.wizard_step == 1,
                    egui::Button::new(RichText::new(label).strong()),
                )
                .clicked()
            {
                if last {
                    self.settings.sections = self.wizard_draft.clone();
                    self.active_tab = Tab::Settings;
                    self.set_status(preview::GREEN, "wizard applied to the current profile");
                } else {
                    self.wizard_step += 1;
                }
            }
        });
    }

    fn wizard_purpose_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("What kind of machine is this?");
        ui.add_space(4.0);
        for (i, (name, desc, f)) in WIZARD_PURPOSES.iter().enumerate() {
            if ui
                .selectable_label(self.wizard_purpose == i, format!("{name} — {desc}"))
                .clicked()
            {
                self.wizard_purpose = i;
                self.wizard_draft = f();
            }
        }
        ui.add_space(4.0);
        ui.label(RichText::new("(every section is adjustable on the next step)").weak());
    }

    fn wizard_sections_ui(&mut self, ui: &mut egui::Ui) {
        let (name, _, _) = WIZARD_PURPOSES[self.wizard_purpose];
        ui.label(RichText::new(format!("Trim the “{name}” preset to taste — the preview below updates live.")).weak());
        ui.add_space(6.0);
        ScrollArea::vertical()
            .id_salt("wizard_sections")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                sections_grid(ui, &mut self.wizard_draft);
            });
        ui.add_space(8.0);
        let lines = preview::build_mock(&self.wizard_draft, false);
        let job = preview::lines_to_job(&lines, 12.0);
        egui::Frame::group(ui.style())
            .inner_margin(6.0)
            .show(ui, |ui| {
                ui.add(egui::Label::new(job));
            });
    }

    fn wizard_confirm_ui(&mut self, ui: &mut egui::Ui) {
        let (name, desc, _) = WIZARD_PURPOSES[self.wizard_purpose];
        ui.label(RichText::new(format!("Purpose: {name}")).strong());
        ui.label(RichText::new(desc).weak());
        ui.add_space(6.0);
        let mut n = 0usize;
        let mut names: Vec<&str> = Vec::new();
        for (label, _, get, _) in settings::SECTION_ENTRIES.iter() {
            if *get(&self.wizard_draft) {
                n += 1;
                names.push(label);
            }
        }
        ui.label(format!("{n} of 15 sections enabled."));
        if !names.is_empty() {
            ui.add(egui::Label::new(RichText::new(names.join(", ")).weak()).wrap());
        }
        ui.add_space(8.0);
        ui.separator();
        ui.label("Result preview:");
        let lines = preview::build_mock(&self.wizard_draft, false);
        let job = preview::lines_to_job(&lines, 12.0);
        egui::Frame::group(ui.style())
            .inner_margin(6.0)
            .show(ui, |ui| {
                ui.add(egui::Label::new(job));
            });
    }
}

const WIZARD_PURPOSES: &[(&str, &str, fn() -> crate::settings::Sections)] = &[
    ("Docker host", "container boxes: docker + ports + security emphasized", crate::settings::Sections::docker_host),
    ("VPS", "production server: ports + security, no dev-tools", crate::settings::Sections::vps_only),
    ("Desktop workstation", "local machine: no docker/web/ports/security", crate::settings::Sections::desktop_only),
    ("Dev box", "tooling + system stats only", crate::settings::Sections::dev_box),
    ("Server (minimal)", "system, disk, network, maintenance", crate::settings::Sections::server_minimal),
    ("Everything on", "all 15 sections", crate::settings::Sections::desktop_full),
];

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        App::update(self, ctx, frame);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_current_profile();
        self.gui.active_tab = Some(self.active_tab);
        if let Some(r) = self.window_rect {
            self.gui.window_rect = Some(r);
        }
        self.gui.save();
    }
}

/// Embed the Hack monospace font (OFL) so the banner preview renders
/// identically on every machine, regardless of installed system fonts.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("hack".to_owned(), egui::FontData::from_owned(include_bytes!("../assets/Hack-Regular.ttf").to_vec()).into());
    // Prepend Hack to the monospace family; egui falls back to its own
    // fonts for any glyph Hack lacks (arrows, box-drawing, checks…).
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        list.insert(0, "hack".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn write_hunks(old: &str, new: &str) -> Vec<(String, Vec<(char, String)>)> {
    let mut old_lines: Vec<String> = old.lines().take(18).map(|l| l.to_string()).collect();
    let old_count = old.lines().count();
    if old_count > 18 {
        old_lines.push(format!("… {} more lines on disk", old_count - 18));
    }
    let mut new_lines: Vec<String> = new.lines().take(18).map(|l| l.to_string()).collect();
    let new_count = new.lines().count();
    if new_count > 18 {
        new_lines.push(format!("… {} more lines to write", new_count - 18));
    }
    vec![
        (
            "on disk (will be replaced)".into(),
            old_lines.iter().map(|l| ('-', l.clone())).collect(),
        ),
        (
            "what will be written".into(),
            new_lines.iter().map(|l| ('+', l.clone())).collect(),
        ),
    ]
}

fn about_text(ui: &mut egui::Ui) {
    ui.label("A one-stop GUI + CLI for vps-info.sh:");
    ui.label("• configure the 15 banner sections and appearance");
    ui.label("• generate a runtime config (~/.config/vpsinfo/vpsinfo.conf)");
    ui.label("• export a standalone baked script with values embedded");
    ui.label("• install/remove the SSH-login hook in your rc files (idempotent, backed up)");
    ui.label("• live preview — static mock or run the real script (8s timeout)");
    ui.add_space(6.0);
    ui.label("CLI mode on this binary:");
    ui.monospace("vpsinfo-gui --generate --preset server-minimal --path ~/vps-info.sh");
    ui.monospace("vpsinfo-gui --rc-add --ssh-only");
    ui.monospace("vpsinfo-gui --rc-remove");
    ui.monospace("vpsinfo-gui --check-rc   # exit 0 = sourced");
    ui.add_space(6.0);
    ui.label("Preview text uses the Hack monospace font, embedded in the binary (OFL license) — identical on every machine.");
    ui.add_space(6.0);
    ui.hyperlink("https://github.com/tjhexa/vps_welcome_script");
    ui.separator();
    ui.label("vps-info.sh and vpsinfo-gui — MIT licensed.");
}

/// A small colour chip + text, used by the preview colour strip.
fn swatch(ui: &mut egui::Ui, color: Color32, text: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 3.0, color);
        ui.label(RichText::new(text).weak());
    });
}

/// The 15-section toggle grid, reusable on any draft (settings or wizard).
fn sections_grid(ui: &mut egui::Ui, s: &mut settings::Sections) {
    egui::Grid::new("sections")
        .num_columns(2)
        .spacing([28.0, 6.0])
        .show(ui, |ui| {
            for (i, (label, tip, _, set)) in settings::SECTION_ENTRIES.iter().enumerate() {
                ui.checkbox((*set)(s), *label).on_hover_text(*tip);
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}