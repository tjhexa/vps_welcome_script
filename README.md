# vps-info — VPS / Machine Info banner

A single, dependency-light **bash script** that prints a screenfetch-style welcome
banner with everything worth knowing about a VPS (or any Linux machine) at a
glance: system stats, a summary bar, a health banner, Docker containers, open
ports, security (fail2ban), maintenance status and dev-tool versions.

It is designed to run on every **SSH login** (see below) and to degrade
gracefully — every tool it inspects is optional, and missing utilities just
produce sensible `-` / "not installed" entries instead of errors.

![preview](https://img.shields.io/badge/works%20on-bash%203%2B-4c1)

---

## Preview

```
+-------------------------------------------------------------------------------------------------------+
| VPS / SYSTEM INFO - Wed Sep 23 00:43                                                                  |
| ----------------------------------------------------------------------------------------------------- |
|                                                                                                       |
|   CPU 1% | MEM 22% | DISK 22% | containers 3/3 | load 1.15                                            |
|                                                                                                       |
|   [!] 2 issue(s): 2 failed unit(s) | 180 updates pending                                              |
|                                                                                                       |
| Hostname         tjhexa-ThinkPad-E14-Gen-6 (tjhexa-ThinkPad-E14-Gen-6)                                |
| User             tjhexa @ tjhexa                                                                      |
| OS               Ubuntu 24.04.4 LTS (x86_64)                                                          |
| Kernel           6.8.0-41-generic                                                                     |
| Uptime           0d 3h 48m                                                                            |
| Load (1/5/15)    1.15 0.84 0.81                                                                       |
| CPU              Intel(R) Core(TM) Ultra 7 155H                                                       |
| Cores            22 (22 online)                                                                       |
| CPU usage        1% (0.2s sample)                                                                     |
| Memory           22% used  7069 MB / 31561 MB                                                         |
| Swap             0 MB / 8191 MB                                                                       |
| ── Disk                                                                                               |
|   +---------------------------+----------------+--------+--------+--------+                           |
|   | MOUNT                     | DEVICE         | SIZE   | USED   | USE%   |                           |
|   +---------------------------+----------------+--------+--------+--------+                           |
|   | /                         | /dev/nvme0n1p2 | 468G   | 95G    | 22%    |                           |
|   ...                                                                                                 |
| ── Security (shown only when fail2ban is installed)                                                    |
| fail2ban         active (v1.0.2)                                                                      |
| Jails            2 configured                                                                         |
|   +----------------+------------+--------------+                                                      |
|   | JAIL           | BANNED NOW | BANNED TOT   |                                                      |
|   +----------------+------------+--------------+                                                      |
|   | sshd           | 3          | 45           |                                                      |
|   | nginx-http     | 0          | 12           |                                                      |
|   +----------------+------------+--------------+                                                      |
| SSH fails        0 (last 24h)                                                                         |
|   ...                                                                                                 |
|   [+] 53 checks retrieved - 1.7s - public IP: cached                                                  |
+-------------------------------------------------------------------------------------------------------+
```

(Frame is truncated; the top border is drawn to the width of the longest line.
Colors are shown only on a real terminal — plain when piped.)

---

## Features

| Section | Shows |
| --- | --- |
| **Summary bar** | CPU / memory / disk / containers / load at a glance |
| **Health banner** | problems flagged: failed units, disk<1GB, mem<1GB, high load, pending updates, reboot-required, containers down |
| **System** | hostname, user, OS, kernel, uptime, load |
| **CPU / Memory** | model, cores, live CPU %, memory + swap usage |
| **Disk** | boxed table: mount, device, size, used, use% (color-coded ≥60% / ≥80%) |
| **Network** | LAN IP, DNS resolvers, public IP + Geo/ASN (cached) |
| **Processes** | running processes, logged-in sessions, last login, failed services |
| **Docker** | client version, gamified health bar `[##########]`, name/image/status table |
| **Web server** | nginx/apache status + site configs with reverse-proxy targets |
| **Open ports** | `ss -ltunpH` table with process names (best-effort without root) |
| **Security** | fail2ban status, jail table (banned now / total), SSH failures last 24h |
| **Maintenance** | pending updates, reboot required, inode usage |
| **Dev tools** | node/npm/yarn/java/python/go/rustc/gcc/... versions and absence |
| **Footer** | total checks, elapsed time, public-IP source (cached/fetched/stale) |

Everything is optional: skip CPU probing with `VPSINFO_SKIP_CPU=1`, apt/dnf
update lookups with `VPSINFO_SKIP_UPDATES=1`, and public-IP/geo lookups with
`NO_PUBLIC_IP=1` / `VPSINFO_SKIP_GEO=1`.

---

## Install & run

```bash
git clone git@github.com:tjhexa/vps_welcome_script.git
cd vps_welcome_script
chmod +x vps-info.sh

./vps-info.sh            # run once by hand
bash vps-info.sh         # same thing, if exec bit is off
```

No root required. Nothing is installed system-wide — it just reads.

### vpsinfo-gui (configure & install the banner)

`vpsinfo-gui` is a small **Rust/egui** app in `gui/` — same repo, same script. It
lets you toggle the sections visually, preview the banner live, manage which
shell rc-files source it, and write your config.

```bash
# debug build (fast compile, unoptimized — good for development)
cd gui
cargo build                 # binary: gui/target/debug/vpsinfo-gui
./target/debug/vpsinfo-gui

# release build (optimized, stripped — what you ship)
cargo build --release       # binary: gui/target/release/vpsinfo-gui
./target/release/vpsinfo-gui

# or skip the explicit build and just run
cargo run --release
```

The GUI has four tabs:

| Tab | What it does |
| --- | --- |
| **Settings** | 15 section toggles, color mode (auto/always/never), frame, CPU sample / updates / public IP / geo, ssh-only guard, theme, presets + named profiles. Undo/redo (`Ctrl+Z` / `Ctrl+Shift+Z`). |
| **Preview** | Live mock banner that reacts to every toggle, plus **▶ Run real preview** (`Ctrl+Enter`) which actually executes the script with your settings (8s timeout, network off). |
| **rc Manager** | Tick `~/.bashrc` / `~/.zshrc` / `~/.profile` … to add or remove the hook. Shows live `sourced ✓` badges, an exact rc-block preview, a green/red diff before applying, and a one-click restore list of the rotating backups (`*.vpsinfo.bak.1..4`). |
| **About** | Attribution, shortcuts, theme/config locations. |

**Shortcuts:** `Ctrl+S` write config · `Ctrl+Enter` real preview · `Ctrl+Z` / `Ctrl+Shift+Z` undo/redo · `?` shortcuts & help.

### Headless CLI (same binary)

Everything the GUI can write is available from the command line, which makes it
scriptable for first-time server setup:

```bash
vpsinfo-gui --generate                       # write ~/.config/vpsinfo/vpsinfo.conf
vpsinfo-gui --generate --preset server-minimal --color never --frame 0
vpsinfo-gui --generate --path /etc/vpsinfo.conf
vpsinfo-gui --export --path ~/vps-info-baked.sh    # standalone script, values embedded
vpsinfo-gui --rc-add --ssh-only              # append hook to rc files (SSH-only guard)
vpsinfo-gui --rc-remove                      # strip only the vpsinfo block (backed up)
vpsinfo-gui --check-rc                       # exit 0 = sourced, 1 = not, 2 = no rc files
```

Config keys can also be passed as flags — `--color auto|always|never`,
`--frame 0|1`, `--ssh-only 0|1`, `--preset server-minimal|desktop-full|docker-host|all-off`,
`--script PATH` (which script the rc hook should run).

### Share / distribute the binary

`vpsinfo-gui` is a **single self-contained ELF binary** — `vps-info.sh` and the
monospace preview font are embedded inside it, so there's nothing to install
alongside it. The release build is ~12 MB and already stripped.

What it needs at runtime:

| Face | Runtime requirements |
| --- | --- |
| **CLI** (`--generate`, `--export`, `--rc-add`, `--rc-remove`, `--check-rc`) | Nothing but a standard `glibc` (libc/libm/libgcc — every desktop/server has these). Works headless, over SSH, in containers. |
| **GUI** | A display with X11 or Wayland and an OpenGL driver (mesa on most desktops). GL is loaded at startup via `dlopen`, so a machine without *any* GL stack will still run the CLI fine. GUI runs acceptably on a 960×600 window or bigger. |

To hand the binary to someone:

```bash
cd gui
cargo build --release
strip -s target/release/vpsinfo-gui          # already stripped, but belt & braces
tar -czf vpsinfo-gui-linux-x86_64.tar.gz -C target/release vpsinfo-gui
```

Then they just `chmod +x vpsinfo-gui && ./vpsinfo-gui` — no Rust, no build tools,
no extra libraries to fetch. The baked `--export` script it produces is even more
portable: a single bash file that runs anywhere bash 3+ exists.

Notes on portability:

- The binary is built for the architecture of the build machine (`x86_64` here;
  other targets via `cargo build --release --target aarch64-unknown-linux-gnu`,
  etc. — install the target with `rustup target add …`).
- It links against **glibc**, so build near the *oldest* distro you need to
  support (or in a container/Docker with an older base image, e.g. Debian 11)
  to get the widest compatibility.
- For a fully static binary: `cargo build --release --target x86_64-unknown-linux-musl`
  (needs `rustup target add x86_64-unknown-linux-musl`). The CLI then runs on
  any Linux regardless of glibc version; the GUI still needs an OpenGL driver,
  which is standard on desktops.

### Run on every SSH login

Append to your `~/.bashrc` so the banner prints for **remote logins only**
(not every local shell you open):

```bash
if [ -n "$SSH_CONNECTION" ]; then
  ~/path/to/vps_welcome_script/vps-info.sh
fi
```

> Tip: it only takes ~1–2 seconds; the public IP/geo and update counts are
> cached, so subsequent logins are fast even on a slow connection.
>
> The GUI's rc Manager (or `--rc-add`) does this for you — it wraps the hook
> in the same `SSH_CONNECTION` check and marks it with a `# ---- vpsinfo:start/end ----`
> block so it can be removed cleanly later.

---

## Caching

The public IP and its Geo/ASN info are cached in `~/.cache/vpsinfo-net`
(1-hour TTL). Logins within the hour use the cache and make **zero** network
calls; if you're offline the last-known value is shown as `stale` rather than
erroring out. Delete that file any time to force a fresh lookup.

---

## Configuration

The script reads `$VPSINFO_CONF` (or `~/.config/vpsinfo/vpsinfo.conf`) at launch
when `VPSINFO_NO_CONFIG` is unset. The file is plain `KEY=VALUE` lines; every key
is also a valid environment variable — and **environment variables always win**
over the file, so you can override per-invocation without touching the file:

```conf
# ~/.config/vpsinfo/vpsinfo.conf  (generated by: vpsinfo-gui --generate)
VPSINFO_SHOW_SUMMARY=1
VPSINFO_SHOW_BANNER=1
VPSINFO_SHOW_SYSTEM=1
VPSINFO_SHOW_CPU=1
VPSINFO_SHOW_MEMORY=1
VPSINFO_SHOW_DISK=1
VPSINFO_SHOW_NETWORK=1
VPSINFO_SHOW_PROCESSES=1
VPSINFO_SHOW_DOCKER=1
VPSINFO_SHOW_WEB=1
VPSINFO_SHOW_PORTS=1
VPSINFO_SHOW_SECURITY=1
VPSINFO_SHOW_MAINT=1
VPSINFO_SHOW_TOOLS=1
VPSINFO_SHOW_FOOTER=1
VPSINFO_FRAME=1
VPSINFO_COLOR=auto
VPSINFO_SKIP_CPU=0
VPSINFO_SKIP_UPDATES=0
NO_PUBLIC_IP=0
VPSINFO_SKIP_GEO=0
```

Colors are enabled only when output is a real terminal, `NO_COLOR` is unset,
and `TERM` is not `dumb`. Environments:

| Variable | Effect |
| --- | --- |
| `NO_COLOR=1` | disable all colors (frame stays, plain) |
| `TERM=dumb` | disables colors |
| `VPSINFO_COLOR=never` | force colors off (`auto`/`always` also accepted) |
| `VPSINFO_FRAME=0` | drop the outer box frame |
| `VPSINFO_SHOW_*` | per-section switch (`VPSINFO_SHOW_DOCKER=0` hides Docker) |
| `VPSINFO_SKIP_CPU=1` | skip the live CPU-usage sample (saves ~0.2s) |
| `VPSINFO_SKIP_UPDATES=1` | skip apt/dnf/dpkg update counts |
| `NO_PUBLIC_IP=1` | skip public IP + Geo/ASN lookups entirely |
| `VPSINFO_SKIP_GEO=1` | show public IP but skip the Geo/ASN lookup |
| `VPSINFO_NO_CONFIG=1` | ignore `vpsinfo.conf` entirely (used by baked exports) |

### Baked standalone export

`vpsinfo-gui --export` (or the GUI's **Export baked script**) writes a single
self-contained script: `vps-info.sh` prefixed with an `export VPSINFO_*` header
that **embeds your current settings and sets `VPSINFO_NO_CONFIG=1`**. The result
is portable — copy it to a server and it always shows exactly the sections you
chose, regardless of any local config file. Re-run the export to update it.

---

## Dependencies

The script adapts to whatever it finds — nothing is required, everything helps:

> `vpsinfo-gui` itself needs **Rust/cargo** to build (eframe/egui, serde, dirs —
> no system C libraries beyond what the display stack needs; the GUI works on
> X11/Wayland). The script it manages still runs anywhere bash 3+ exists.

| Tool | Used for |
| --- | --- |
| `curl` | public IP + Geo/ASN lookup |
| `systemctl` | service states, failed units, reboot-required |
| `ss` | open-port table |
| `docker` | container overview |
| `ss` + `sudo -n` | port process names (needs passwordless sudo, else `-`) |
| `journalctl` / `/var/log/auth.log` | failed SSH login count |
| `fail2ban-client` | the Security section (shown only when present) |
| `df --output` | disk + inode tables (GNU coreutils) |

None exist? The section simply stays hidden or shows `not installed`.

---

## Troubleshooting

- **No colors?** You're piping the output, `NO_COLOR` is set, or `TERM=dumb`.
  They're deliberately disabled in those cases — this also keeps scripts/hooks
  safe from leaking escape codes.
- **Port processes show `-`?** `ss -p` hides processes owned by other users
  unless run as root (or with passwordless `sudo`).
- **Security section missing?** It only renders when `fail2ban-client` is
  installed; if it shows `no socket access`, run as root or add your user to
  the `fail2ban` group.
- **`[+] N checks retrieved - 3.2s - public IP: fetched`?** That's the footer —
  the first run fetches everything; later runs say `cached` and are faster.

---

## License

MIT — do whatever you like, keep the attribution line if you fork it.