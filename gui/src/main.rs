//! vpsinfo-gui: single binary, two faces — the egui GUI and a headless CLI
//! for scriptable provisioning.
mod actions;
mod app;
mod preview;
mod rc;
mod settings;

use eframe::egui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gui_state = settings::GuiState::load();
    // V10 (#29): autodetect an existing runtime config — if the Default
    // profile is still untouched, pre-fill it from ~/.config/vpsinfo/vpsinfo.conf.
    let default_untouched = gui_state
        .profiles
        .get("Default")
        .map(|s| *s == settings::Settings::default())
        .unwrap_or(false);
    if default_untouched {
        let cfg = settings::expand_tilde(&settings::Settings::default().config_path);
        if let Ok(txt) = std::fs::read_to_string(&cfg) {
            let parsed = settings::Settings::from_config_text(&txt);
            if parsed != settings::Settings::default() {
                gui_state.profiles.insert("Default".to_string(), parsed);
            }
        }
    }
    let mut cli_settings = gui_state.current_settings();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty()
        && (args[0] == "--generate"
            || args[0] == "--export"
            || args[0] == "--rc-add"
            || args[0] == "--rc-remove"
            || args[0] == "--check-rc"
            || args[0] == "--export-json"
            || args[0] == "--import-json"
            || args[0] == "--install-system"
            || args[0] == "--remove-system")
    {
        return cli_run(&args, &mut cli_settings);
    }

    let mut viewport = egui::ViewportBuilder::default().with_title("vpsinfo-gui");
    if let Some(r) = gui_state.window_rect {
        viewport = viewport.with_inner_size([r[2].max(960.0), r[3].max(620.0)]);
        viewport = viewport.with_position([r[0], r[1]]);
    } else {
        viewport = viewport.with_inner_size([1120.0, 760.0]);
    }
    viewport = viewport.with_min_inner_size([960.0, 620.0]);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "vpsinfo-gui",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, gui_state)))),
    )
    .map_err(|e| e.into())
}

fn usage() -> ! {
    eprintln!(
        "vpsinfo-gui — configure vps-info.sh and manage rc-file sourcing

GUI:            vpsinfo-gui
CLI:
  --generate            write the runtime config file
      --path PATH       target config path (default: ~/.config/vpsinfo/vpsinfo.conf)
      --preset NAME     apply preset first: server-minimal | desktop-full | docker-host |
                        all-off | desktop-only | vps-only | dev-box
      --color MODE      auto | always | never
      --frame 0|1       frame around the banner output
      --ssh-only 0|1    rc hook wrapped in SSH_CONNECTION check
      --dry-run         print what would be written instead of writing
  --export              write a baked standalone script (values embedded, VPSINFO_NO_CONFIG=1)
      --path PATH       target script path (default: settings script_path)
      --dry-run         print the baked script to stdout instead
  --rc-add              append the vpsinfo hook block to discovered rc files
      --ssh-only        wrap in SSH_CONNECTION check (default: on)
      --script PATH     path the hook should run (default: settings script_path)
      --dry-run         print the hook block to stdout instead
  --rc-remove           strip only the vpsinfo marker block (backed up first)
  --check-rc            exit code: 0 = sourced in an rc file, 1 = not sourced, 2 = no rc files
  --export-json         print the current profile as JSON
      --path PATH       write the JSON to this file instead of stdout
  --import-json         load a profile from a JSON file
      --path PATH       JSON file to read (required)
  --install-system      install /usr/local/bin/vpsinfo + /etc/profile.d hook (sudo)
  --remove-system       remove the system-wide install (sudo)"
    );
    std::process::exit(2);
}

fn cli_run(args: &[String], settings: &mut settings::Settings) -> Result<(), Box<dyn std::error::Error>> {
    // parse flags
    let mut path: Option<String> = None;
    let mut preset: Option<String> = None;
    let mut color: Option<String> = None;
    let mut frame: Option<u8> = None;
    let mut ssh_only: Option<bool> = None;
    let mut script_path: Option<String> = None;
    let mut dry_run = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dry-run" => dry_run = true,
            "--path" => {
                i += 1;
                path = Some(args.get(i).cloned().unwrap_or_else(usage_abort));
            }
            "--preset" => {
                i += 1;
                preset = Some(args.get(i).cloned().unwrap_or_else(usage_abort));
            }
            "--color" => {
                i += 1;
                color = Some(args.get(i).cloned().unwrap_or_else(usage_abort));
            }
            "--frame" => {
                i += 1;
                frame = Some(args.get(i).cloned().unwrap_or_else(usage_abort).parse().unwrap_or_else(|_| usage_abort()));
            }
            "--ssh-only" => {
                i += 1;
                let v = args.get(i).cloned().unwrap_or_else(|| "1".into());
                ssh_only = Some(v == "1" || v == "true" || v == "on");
            }
            "--script" => {
                i += 1;
                script_path = Some(args.get(i).cloned().unwrap_or_else(usage_abort));
            }
            other => {
                eprintln!("unknown flag: {other}");
                usage();
            }
        }
        i += 1;
    }

    if let Some(p) = preset {
        let f = match p.as_str() {
            "server-minimal" => settings::Sections::server_minimal,
            "desktop-full" => settings::Sections::desktop_full,
            "docker-host" => settings::Sections::docker_host,
            "all-off" => settings::Sections::all_off,
            "desktop-only" => settings::Sections::desktop_only,
            "vps-only" => settings::Sections::vps_only,
            "dev-box" => settings::Sections::dev_box,
            other => {
                eprintln!("unknown preset: {other}");
                std::process::exit(2);
            }
        };
        settings.sections = f();
    }
    if let Some(c) = color {
        settings.color = settings::ColorMode::parse(&c);
    }
    if let Some(f) = frame {
        settings.frame = f != 0;
    }
    if let Some(s) = ssh_only {
        settings.ssh_only = s;
    }
    if let Some(sp) = script_path {
        settings.script_path = sp;
    }

    let op = args[0].as_str();
    match op {
        "--generate" => {
            if dry_run {
                print!("{}", settings.config_lines("default"));
                return Ok(());
            }
            let target = path.clone().unwrap_or_else(|| settings.config_path.clone());
            match actions::generate_config(settings, "default", &target) {
                Ok(()) => {
                    println!("config written to {}", settings::expand_tilde(&target));
                    Ok(())
                }
                Err(e) => Err(format!("generate config: {e}").into()),
            }
        }
        "--export" => {
            if dry_run {
                print!("{}", actions::export_baked_text(settings, "default"));
                return Ok(());
            }
            let target = path.clone().unwrap_or_else(|| settings.script_path.clone());
            match actions::export_baked(settings, "default", &target) {
                Ok(()) => {
                    println!("baked script written to {}", settings::expand_tilde(&target));
                    Ok(())
                }
                Err(e) => Err(format!("export baked: {e}").into()),
            }
        }
        "--rc-add" => {
            if dry_run {
                print!("{}", rc::build_block(&settings.script_path, settings.ssh_only));
                return Ok(());
            }
            let script = settings.script_path.clone();
            let mut n = 0;
            let mut first_err: Option<String> = None;
            for rf in rc::candidates() {
                if !rf.exists {
                    continue;
                }
                match rc::add_block(&rf.path, &script, settings.ssh_only) {
                    Ok(()) => n += 1,
                    Err(e) => {
                        if first_err.is_none() {
                            first_err = Some(e.to_string());
                        }
                    }
                }
            }
            if let Some(e) = first_err {
                Err(format!("rc-add: {e} ({n} ok)").into())
            } else {
                println!("rc hook added to {n} rc file(s)");
                Ok(())
            }
        }
        "--rc-remove" => {
            let mut n = 0;
            for rf in rc::candidates() {
                if rf.exists && rf.sourced {
                    if rc::remove_block(&rf.path).is_ok() {
                        n += 1;
                    }
                }
            }
            println!("rc block removed from {n} file(s)");
            Ok(())
        }
        "--check-rc" => {
            let found: Vec<_> = rc::candidates().into_iter().filter(|f| f.exists).collect();
            if found.is_empty() {
                eprintln!("no rc files found");
                std::process::exit(2);
            }
            for f in &found {
                let status = if f.sourced { "sourced" } else { "not sourced" };
                println!("{}: {status}", f.path.display());
            }
            if found.iter().any(|f| f.sourced) {
                std::process::exit(0);
            } else {
                std::process::exit(1);
            }
        }
        "--export-json" => {
            let txt = settings.to_json()?;
            match path {
                Some(p) => {
                    let p = settings::expand_tilde(&p);
                    if let Some(dir) = std::path::Path::new(&p).parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    std::fs::write(&p, txt)?;
                    println!("profile JSON written to {p}");
                }
                None => println!("{txt}"),
            }
            Ok(())
        }
        "--import-json" => {
            let p = path.ok_or_else(|| "import-json requires --path FILE".to_string())?;
            let txt = std::fs::read_to_string(settings::expand_tilde(&p))?;
            let s = settings::Settings::from_json(&txt)
                .ok_or_else(|| format!("{p} is not a valid vpsinfo profile JSON"))?;
            *settings = s.clone();
            // Persist so the next CLI/GUI invocation actually sees it.
            let mut gs = settings::GuiState::load();
            let name = gs.current_profile.clone();
            gs.profiles.insert(name.clone(), s);
            gs.save();
            println!("imported profile from {p} into “{name}”");
            Ok(())
        }
        "--install-system" => match actions::install_system(settings) {
            Ok(m) => {
                println!("{m}");
                Ok(())
            }
            Err(e) => Err(format!("install-system: {e}").into()),
        },
        "--remove-system" => match actions::uninstall_system() {
            Ok(m) => {
                println!("{m}");
                Ok(())
            }
            Err(e) => Err(format!("remove-system: {e}").into()),
        },
        _ => unreachable!(),
    }
}

fn usage_abort<T>() -> T {
    eprintln!("missing value for flag");
    usage();
}

#[allow(dead_code)]
fn _unused(_: &str) {}