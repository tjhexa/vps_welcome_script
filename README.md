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

---

## Caching

The public IP and its Geo/ASN info are cached in `~/.cache/vpsinfo-net`
(1-hour TTL). Logins within the hour use the cache and make **zero** network
calls; if you're offline the last-known value is shown as `stale` rather than
erroring out. Delete that file any time to force a fresh lookup.

---

## Configuration

Colors are enabled only when output is a real terminal, `NO_COLOR` is unset,
and `TERM` is not `dumb`. Environments:

| Variable | Effect |
| --- | --- |
| `NO_COLOR=1` | disable all colors (frame stays, plain) |
| `TERM=dumb` | disables colors |
| `VPSINFO_SKIP_CPU=1` | skip the live CPU-usage sample (saves ~0.2s) |
| `VPSINFO_SKIP_UPDATES=1` | skip apt/dnf/dpkg update counts |
| `NO_PUBLIC_IP=1` | skip public IP + Geo/ASN lookups entirely |
| `VPSINFO_SKIP_GEO=1` | show public IP but skip the Geo/ASN lookup |

---

## Dependencies

The script adapts to whatever it finds — nothing is required, everything helps:

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