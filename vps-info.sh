#!/usr/bin/env bash
# ------------------------------------------------------------------
# vps-info.sh
# Quick overview of VPS / machine basic details.
#   - System, CPU, memory, disk, network (+DNS, geo details, cached IP)
#   - Summary bar + health banner (issues at a glance)
#   - Docker: containers table with status + health bar
#   - Web server: nginx/apache sites with reverse-proxy info
#   - Open ports + maintenance (updates, reboot, inodes)
#   - Dev tools versions, timing footer
#   - Whole output wrapped in a screenfetch-style frame
# Source this from ~/.bashrc to have it run on every new login shell.
# ------------------------------------------------------------------
set -o pipefail

TTY_OK=0; [ -t 1 ] && TTY_OK=1
checks_total=0
# ms-resolution clock (GNU date); plain seconds fallback (BSD/macOS)
if date -d @0 +%3N >/dev/null 2>&1; then now_ms() { date +%s%3N; }; else now_ms() { date +%s; }; fi
_start_ms=$(now_ms)

# --- Colors (only when safe: real terminal, sane TERM, NO_COLOR unset) --
if [ "$TTY_OK" = "1" ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-dumb}" != "dumb" ]; then
  ESC=$(printf '\033')
  C_RESET="${ESC}[0m"; C_BOLD="${ESC}[1m"
  C_CYA="${ESC}[36m"; C_GRN="${ESC}[32m"; C_YLW="${ESC}[33m"; C_RED="${ESC}[31m"; C_BLU="${ESC}[34m"
  C_DIM="${ESC}[2m"
else
  C_RESET=""; C_BOLD=""; C_CYA=""; C_GRN=""; C_YLW=""; C_RED=""; C_BLU=""; C_DIM=""
fi

has() { command -v "$1" >/dev/null 2>&1; }
# section + table helpers
section() { checks_total=$((checks_total + 1)); printf "${C_BOLD}${C_CYA}── %s${C_RESET}\n" "$1"; }
trunc() { local s="$1" n="$2"; [ "${#s}" -gt "$n" ] && s="${s:0:$((n-1))}.."; printf '%s' "$s"; }
tborder() { local bar="+" w; for w in "$@"; do bar+="$(printf '%*s' $((w+2)) '' | tr ' ' '-')+"; done; printf "  ${C_DIM}%s${C_RESET}\n" "$bar"; }

# --- Pre-scan (cheap metrics, computed once, reused everywhere) -------
cpu_usage() {
  local prev_idle=0 prev_total=0 idle total diff_idle diff_total
  while read -r _ a b c d e f g h i j k; do
    idle=$((d + e)); total=$((a + b + c + idle + f + g + h + i + j + k))
    prev_idle=$idle; prev_total=$total
    break
  done < /proc/stat
  [ -n "${VPSINFO_SKIP_CPU:-}" ] && { echo "-"; return; }
  sleep 0.2
  while read -r _ a b c d e f g h i j k; do
    idle=$((d + e)); total=$((a + b + c + idle + f + g + h + i + j + k))
    break
  done < /proc/stat
  diff_idle=$((idle - prev_idle)); diff_total=$((total - prev_total))
  echo $((100 * (diff_total - diff_idle) / diff_total))
}
pre_scan() {
  read -r load1 load5 load15 _ < /proc/loadavg
  nproc=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)
  cpu_cores=$(grep -c '^processor' /proc/cpuinfo)

  CPU_PCT=$(cpu_usage)

  mem_total=$(awk '/^MemTotal:/{print $2}' /proc/meminfo)
  mem_avail=$(awk '/^MemAvailable:/{print $2}' /proc/meminfo)
  mem_used=$((mem_total - mem_avail))
  MEM_PCT=$((100 * mem_used / mem_total))

  failed_raw=""
  if has systemctl; then failed_raw=$(systemctl --failed --no-legend --plain 2>/dev/null); fi
  failed_count=0; failed_list=""
  if [ -n "$failed_raw" ]; then
    failed_list=$(printf '%s' "$failed_raw" | awk '{print $1}' | paste -sd, -)
    failed_count=$(printf '%s\n' "$failed_raw" | grep -c . || true)
  fi

  reboot_req="no"; [ -f /var/run/reboot-required ] && reboot_req="yes"

  updates_pending=0
  if [ -z "${VPSINFO_SKIP_UPDATES:-}" ]; then
    if [ -r /var/lib/update-notifier/updates-available ]; then
      updates_pending=$(grep -m1 -E '^[0-9]+ updates? can be applied' /var/lib/update-notifier/updates-available 2>/dev/null | awk '{print $1}')
      [[ "$updates_pending" =~ ^[0-9]+$ ]] || updates_pending=0
    elif has apt; then
      updates_pending=$(apt list --upgradable 2>/dev/null | sed -n '2,$p' | grep -c . || true)
    elif has dnf; then
      updates_pending=$(dnf -q check-update 2>/dev/null | grep -vc '^$' || true)
    fi
  fi

  docker_ok=0; docker_running_ct=0; docker_total_ct=0
  if has docker && docker info >/dev/null 2>&1; then
    docker_ok=1
    docker_running_ct=$(docker ps -q | wc -l)
    docker_total_ct=$(docker ps -aq | wc -l)
  fi

  disk_worst=0; disk_warn_n=0; disk_warns=""
  if df -B1 -x tmpfs -x devtmpfs -x overlay -x squashfs --output=target,size,pcent >/dev/null 2>&1; then
    while read -r m sz p; do
      p=${p%\%}
      [ "$sz" -lt 1073741824 ] && continue          # skip tiny pseudo-filesystems
      [ "$p" -gt "$disk_worst" ] && disk_worst=$p
      if [ "$p" -ge 80 ]; then
        disk_warn_n=$((disk_warn_n + 1))
        disk_warns="${disk_warns:+$disk_warns, }${m} at ${p}%"
      fi
    done < <(df -B1 -x tmpfs -x devtmpfs -x overlay -x squashfs --output=target,size,pcent 2>/dev/null | tail -n +2)
  fi
}
pre_scan

# --- Header ----------------------------------------------------------
print_row() { checks_total=$((checks_total + 1)); printf "${C_DIM}%-16s${C_RESET} %s\n" "$1" "$2"; }

# --- framing: capture body, then draw a screenfetch-style box ---------
OUTTMP=$(mktemp "${TMPDIR:-/tmp}/vpsinfo.XXXXXX")
strip_ansi() { sed -E 's/\x1B\[[0-9;]*[A-Za-z]//g'; }
frame() {
  local file="$1" maxw=0 line vis pad first=1
  while IFS= read -r line; do
    vis=$(printf '%s' "$line" | strip_ansi)
    [ "${#vis}" -gt "$maxw" ] && maxw=${#vis}
  done < "$file"
  local top
  top=$(printf '%*s' $((maxw + 2)) '' | tr ' ' '-')
  printf "${C_BOLD}${C_CYA}+%s+${C_RESET}\n" "$top"
  while IFS= read -r line; do
    vis=$(printf '%s' "$line" | strip_ansi)
    pad=$((maxw - ${#vis})); [ "$pad" -lt 0 ] && pad=0
    printf "${C_CYA}|${C_RESET} %s%*s ${C_CYA}|${C_RESET}\n" "$line" "$pad" ''
    if [ "$first" = "1" ]; then
      dashes=$(printf '%*s' "$maxw" '' | tr ' ' '-')
      printf "${C_CYA}|${C_RESET} ${C_DIM}%s${C_RESET} ${C_CYA}|${C_RESET}\n" "$dashes"
      first=
    fi
  done < "$file"
  printf "${C_BOLD}${C_CYA}+%s+${C_RESET}\n" "$top"
}

{  # ---------- framed body begins ----------
printf "${C_BOLD}%s${C_RESET}\n" "VPS / SYSTEM INFO - $(date +'%a %b %d %H:%M')"
echo

# --- summary bar -----------------------------------------------------
if [[ "$CPU_PCT" =~ ^[0-9]+$ ]]; then
  [ "$CPU_PCT" -ge 80 ] && cpuc="${C_RED}" || { [ "$CPU_PCT" -ge 60 ] && cpuc="${C_YLW}" || cpuc="${C_GRN}"; }
else cpuc="${C_DIM}"; fi
[ "$MEM_PCT" -ge 80 ] && memc="${C_RED}" || { [ "$MEM_PCT" -ge 60 ] && memc="${C_YLW}" || memc="${C_GRN}"; }
[ "$disk_worst" -ge 80 ] && disc="${C_RED}" || { [ "$disk_worst" -ge 60 ] && disc="${C_YLW}" || disc="${C_GRN}"; }
sum="CPU ${cpuc}${CPU_PCT}%${C_RESET} | MEM ${memc}${MEM_PCT}%${C_RESET} | DISK ${disc}${disk_worst}%${C_RESET}"
if [ "$docker_ok" = "1" ]; then
  [ "$docker_running_ct" -lt "$docker_total_ct" ] && dcol="${C_YLW}" || dcol="${C_GRN}"
  sum="$sum | containers ${dcol}${docker_running_ct}/${docker_total_ct}${C_RESET}"
fi
sum="$sum | load ${C_DIM}${load1}${C_RESET}"
printf "  ${C_BOLD}%s${C_RESET}\n" "$sum"
echo

# --- health banner ---------------------------------------------------
issues=()
add_issue() { issues+=("$1"); }
[ "$MEM_PCT" -ge 80 ] && add_issue "memory ${MEM_PCT}%"
[ "$disk_warn_n" -gt 0 ] && add_issue "disk ${disk_warns}"
[ "$failed_count" -gt 0 ] && add_issue "${failed_count} failed unit(s)"
awk -v l="$load1" -v c="$nproc" 'BEGIN{exit !(l>=c)}' && add_issue "load ${load1} >= ${nproc} cores"
[ "$reboot_req" = "yes" ] && add_issue "reboot required"
[ "$updates_pending" -gt 0 ] && add_issue "${updates_pending} updates pending"
if [ "$docker_ok" = "1" ] && [ "$docker_running_ct" -lt "$docker_total_ct" ]; then
  add_issue "${docker_running_ct}/${docker_total_ct} containers down"
fi
if [ "${#issues[@]}" -gt 0 ]; then
  msg=""
  for it in "${issues[@]:0:3}"; do msg="${msg:+$msg | }$it"; done
  extra=$(( ${#issues[@]} - 3 )); [ "$extra" -le 0 ] && extra=0
  [ "$extra" -gt 0 ] && msg="$msg | +${extra} more"
  printf "  ${C_RED}[!] ${#issues[@]} issue(s): ${msg}${C_RESET}\n"
else
  printf "  ${C_GRN}[+] all systems nominal${C_RESET}\n"
fi
echo

# --- System ----------------------------------------------------------
HOST=$(hostname -f 2>/dev/null || hostname)
print_row "Hostname"    "$(hostname) ${C_DIM}($HOST)${C_RESET}"
print_row "User"        "$USER @ $(logname 2>/dev/null || echo '-')"

# Distro
if has lsb_release; then
  OS=$(lsb_release -ds 2>/dev/null)
else
  OS=$(sed -n 's/^PRETTY_NAME="\?\([^"]*\)"\?/\1/p' /etc/os-release 2>/dev/null)
fi
print_row "OS"          "${OS:-unknown} (${C_DIM}$(uname -m)${C_RESET})"

# Kernel
print_row "Kernel"      "$(uname -r)"

# Uptime + load
read -r up _ < /proc/uptime
up=$(printf "%.0f" "${up%.*}")
days=$((up / 86400)); hrs=$(((up % 86400) / 3600)); mins=$(((up % 3600) / 60))
print_row "Uptime"      "${days}d ${hrs}h ${mins}m"
print_row "Load (1/5/15)" "$(cut -d' ' -f1-3 /proc/loadavg)"

# --- CPU -------------------------------------------------------------
cpu_model=$(grep -m1 "model name" /proc/cpuinfo | sed 's/.*: *//')
print_row "CPU"         "${cpu_model:-unknown}"
print_row "Cores"       "${cpu_cores} (${C_DIM}${nproc} online${C_RESET})"
if [ "${CPU_PCT}" = "-" ]; then
  print_row "CPU usage" "${C_DIM}skipped (VPSINFO_SKIP_CPU=1)${C_RESET}"
else
  cpuc="${C_GRN}"; [ "$CPU_PCT" -ge 80 ] && cpuc="${C_RED}" || [ "$CPU_PCT" -ge 60 ] && cpuc="${C_YLW}"
  print_row "CPU usage" "${cpuc}${CPU_PCT}%${C_RESET} ${C_DIM}(0.2s sample)${C_RESET}"
fi

# --- Memory ----------------------------------------------------------
mem_info() { awk -F': *' -v key="$1" '$0 ~ key {gsub(/kB/,""); print $2}' /proc/meminfo; }
[ "$MEM_PCT" -ge 80 ] && mcol="${C_RED}" || { [ "$MEM_PCT" -ge 60 ] && mcol="${C_YLW}" || mcol="${C_GRN}"; }
print_row "Memory"      "${mcol}${MEM_PCT}%${C_RESET} used  $((mem_used / 1024)) MB / $((mem_total / 1024)) MB"
swap_total=$(mem_info '^SwapTotal'); swap_free=$(mem_info '^SwapFree')
swap_used=$((swap_total - swap_free))
print_row "Swap"        "$((swap_used / 1024)) MB / $((swap_total / 1024)) MB"

# --- Disk ------------------------------------------------------------
section "Disk"
if df -h -x tmpfs -x devtmpfs -x overlay -x squashfs --output=source,target,size,used,pcent >/dev/null 2>&1; then
  DW1=25; DW2=14; DW3=6; DW4=6; DW5=6
  tborder $DW1 $DW2 $DW3 $DW4 $DW5
  printf "  ${C_DIM}| %-${DW1}s | %-${DW2}s | %-${DW3}s | %-${DW4}s | %-${DW5}s |${C_RESET}\n" "MOUNT" "DEVICE" "SIZE" "USED" "USE%"
  tborder $DW1 $DW2 $DW3 $DW4 $DW5
  df -h -x tmpfs -x devtmpfs -x overlay -x squashfs --output=source,target,size,used,pcent 2>/dev/null | tail -n +2 | \
  while read -r src mnt size used pct; do
    p=${pct%\%}
    if   [ "$p" -ge 80 ]; then pcol="${C_RED}"
    elif [ "$p" -ge 60 ]; then pcol="${C_YLW}"
    else pcol="${C_GRN}"; fi
    checks_total=$((checks_total + 1))
    printf "  | %-${DW1}s | %-${DW2}s | %-${DW3}s | %-${DW4}s | %s%-${DW5}s%s |\n" \
      "$(trunc "$mnt" $DW1)" "$(trunc "$src" $DW2)" "$size" "$used" "$pcol" "$pct" "$C_RESET"
  done
  tborder $DW1 $DW2 $DW3 $DW4 $DW5
else
  df -h -x tmpfs -x devtmpfs -x overlay -x squashfs | head -8
fi

# --- Network ---------------------------------------------------------
lan=$(hostname -I 2>/dev/null | tr ' ' '\n' | grep -v '^$' | head -1)
[ -n "$lan" ] && print_row "LAN IP" "${lan}"

dns=$(grep -hE '^nameserver[[:space:]]+' /etc/resolv.conf 2>/dev/null | awk '{print $2}' | sort -u | paste -sd' ' -)
[ -n "$dns" ] && print_row "DNS" "$dns"

NET_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/vpsinfo-net"
NET_SRC="n/a"
if [ -n "${NO_PUBLIC_IP:-}" ]; then
  print_row "Public IP" "${C_DIM}skipped (NO_PUBLIC_IP=1)${C_RESET}"
elif has curl; then
  now=$(date +%s)
  cached_age=999999
  [ -f "$NET_CACHE" ] && cached_age=$(( now - $(stat -c %Y "$NET_CACHE" 2>/dev/null || echo 0) ))
  if [ -f "$NET_CACHE" ] && [ "$cached_age" -lt 3600 ]; then
    IFS=$'\t' read -r pub geo _ < "$NET_CACHE"
    NET_SRC="cached"
  else
    pub=$(curl -4 -s --max-time 3 https://ifconfig.me 2>/dev/null)
    [ -z "$pub" ] && pub=$(curl -6 -s --max-time 3 https://ifconfig.me 2>/dev/null)
    if [ -n "$pub" ]; then
      if [ -z "${VPSINFO_SKIP_GEO:-}" ]; then
        gj=$(curl -s --max-time 4 "https://ipinfo.io/${pub}/json" 2>/dev/null)
        city=$(printf '%s' "$gj" | sed -n 's/.*"city":[[:space:]]*"\([^"]*\)".*/\1/p')
        region=$(printf '%s' "$gj" | sed -n 's/.*"region":[[:space:]]*"\([^"]*\)".*/\1/p')
        country=$(printf '%s' "$gj" | sed -n 's/.*"country":[[:space:]]*"\([^"]*\)".*/\1/p')
        org=$(printf '%s' "$gj" | sed -n 's/.*"org":[[:space:]]*"\([^"]*\)".*/\1/p')
        geo="$city"
        [ -n "$region" ] && geo="$geo, $region"
        [ -n "$country" ] && geo="$geo, $country"
        [ -n "$org" ] && geo="$geo ($org)"
      fi
      mkdir -p "$(dirname "$NET_CACHE")"
      printf '%s\t%s\n' "$pub" "$geo" > "$NET_CACHE"
      NET_SRC="fetched"
    else
      if [ -f "$NET_CACHE" ]; then
        IFS=$'\t' read -r pub geo _ < "$NET_CACHE"
        NET_SRC="stale"
      fi
    fi
  fi
  [ -n "$pub" ] && print_row "Public IP" "$pub" || print_row "Public IP" "${C_DIM}unreachable (offline?)${C_RESET}"
  [ -n "$geo" ] && print_row "Geo / ASN" "${C_DIM}$geo${C_RESET}"
else
  print_row "Public IP" "${C_DIM}curl not installed${C_RESET}"
fi

# --- Processes / users ----------------------------------------------
print_row "Processes"   "$(ps -e --no-headers 2>/dev/null | wc -l) running"
who_qty=$(who 2>/dev/null | wc -l)
print_row "Logged-in"   "$who_qty session(s)"

# --- Last login for this user ---------------------------------------
lastlogin=$(last -n 1 "$USER" 2>/dev/null | head -1 | awk '{$1=""; sub(/^ +/,""); print}')
[ -n "$lastlogin" ] && print_row "Last login" "$lastlogin"

# --- Services quick check -------------------------------------------
if has systemctl; then
  if [ "$failed_count" -gt 0 ]; then
    print_row "Failed srvcs" "${C_RED}${failed_list}${C_RESET} ${C_DIM}($failed_count unit(s))${C_RESET}"
  else
    print_row "Failed srvcs" "${C_GRN}none${C_RESET}"
  fi
fi

# ======================================================================
#  EXTRA SECTIONS (docker / web server / dev tools)
# ======================================================================

# --- Docker ----------------------------------------------------------
section "Docker"
if has docker; then
  print_row "Docker" "client $(docker --version 2>/dev/null | awk '{print $3}' | tr -d ',')"
  if [ "$docker_ok" = "1" ]; then
    running=$docker_running_ct
    total=$docker_total_ct

    # gamified health bar
    if [ "$total" -gt 0 ]; then
      filled=$((running * 10 / total)); [ "$filled" -gt 10 ] && filled=10
      bfill=$(printf '%*s' "$filled" '' | tr ' ' '#')
      brest=$(printf '%*s' $((10 - filled)) '' | tr ' ' '-')
      if [ "$running" -eq "$total" ]; then hcol="${C_GRN}"
      elif [ "$running" -eq 0 ]; then hcol="${C_RED}"
      else hcol="${C_YLW}"; fi
      print_row "Containers" "${hcol}[${bfill}${brest}]${C_RESET} ${C_BOLD}${running} up / ${total} total${C_RESET}"
    fi

    CW1=20; CW2=28; CW3=12
    tborder $CW1 $CW2 $CW3
    printf "  ${C_DIM}| %-${CW1}s | %-${CW2}s | %-${CW3}s |${C_RESET}\n" "NAME" "IMAGE" "STATUS"
    tborder $CW1 $CW2 $CW3

    if [ "$running" -gt 0 ]; then
      while IFS='|' read -r cname cimage cstate cstatus; do
        case "$cstate" in
          running)     tag="UP";       col="${C_GRN}"; raw="${cstatus#Up }" ;;
          paused)      tag="PAUSED";   col="${C_YLW}"; raw="" ;;
          restarting)  tag="RESTART";  col="${C_YLW}"; raw="" ;;
          removing)    tag="STOPPING"; col="${C_YLW}"; raw="" ;;
          dead)        tag="DEAD";     col="${C_RED}"; raw="" ;;
          exited)      tag="DOWN";     col="${C_RED}"; raw="" ;;
          created)     tag="CREATED";  col="${C_BLU}"; raw="" ;;
          *)           tag="$cstate";  col="${C_DIM}"; raw="" ;;
        esac
        dur=$(printf '%s' "$raw" | awk '{for(i=1;i<=NF;i++){if($i~/^[0-9]+$/){n=$i}else if($i~/^hour/){if(n=="")n="~1";printf "%sh",n}else if($i~/^minute/){if(n=="")n="~1";printf "%sm",n}else if($i~/^day/){if(n=="")n="~1";printf "%sd",n}else if($i~/^second/){if(n=="")n="~1";printf "%ss",n}}}')
        checks_total=$((checks_total + 1))
        printf "  | %-${CW1}s | %-${CW2}s | %s%-${CW3}s%s |\n" \
          "$(trunc "$cname" $CW1)" "$(trunc "$cimage" $CW2)" "$col" "${tag}${dur:+ $dur}" "$C_RESET"
      done < <(docker ps --format '{{.Names}}|{{.Image}}|{{.State}}|{{.Status}}')
    else
      printf "  ${C_DIM}no containers running${C_RESET}\n"
    fi
    tborder $CW1 $CW2 $CW3
  else
    print_row "Engine" "${C_RED}daemon not running or no permission${C_RESET}"
  fi
else
  print_row "Docker" "${C_DIM}not installed${C_RESET}"
fi

# --- Web server (nginx / apache) -------------------------------------
section "Web server"
nginx_sites() {
  local files=() cfg
  for cfg in /etc/nginx/sites-enabled/* /etc/nginx/conf.d/*.conf; do
    [ -f "$cfg" ] && files+=("$cfg")
  done
  if [ "${#files[@]}" -eq 0 ]; then
    printf "  ${C_DIM}no site configs found${C_RESET}\n"
    return
  fi
  for cfg in "${files[@]}"; do
    local name ports proxies out
    name=$(grep -m1 -E '^[[:space:]]*server_name[[:space:]]' "$cfg" \
           | sed -E 's/^[[:space:]]*server_name[[:space:]]+//; s/[;[:space:]]+.*$//')
    ports=$(grep -E '^[[:space:]]*listen[[:space:]]' "$cfg" \
            | sed -E 's/^[[:space:]]*listen[[:space:]]+//; s/[;[:space:]]+.*$//' | sort -u | paste -sd, -)
    proxies=$(grep -E 'proxy_pass[[:space:]]' "$cfg" \
              | sed -E 's/^[[:space:]]*proxy_pass[[:space:]]+//; s/[;[:space:]]+.*$//' | sort -u | paste -sd, -)
    out="${C_GRN}${name:-NO server_name}${C_RESET}  ${C_DIM}(listen: ${ports:-?})${C_RESET}"
    [ -n "$proxies" ] && out="$out  ${C_YLW}→ proxy: ${proxies}${C_RESET}"
    printf "  %s\n" "$out"
  done
}
apache_sites() {
  local files=() cfg
  for cfg in /etc/apache2/sites-enabled/*; do
    [ -f "$cfg" ] && files+=("$cfg")
  done
  if [ "${#files[@]}" -eq 0 ]; then
    printf "  ${C_DIM}no sites enabled${C_RESET}\n"
    return
  fi
  for cfg in "${files[@]}"; do
    local sname docroot proxy out
    sname=$(grep -m1 -E '^[[:space:]]*ServerName' "$cfg" \
            | sed -E 's/^[[:space:]]*ServerName[[:space:]]+//; s/[[:space:]]+.*$//')
    docroot=$(grep -m1 -E '^[[:space:]]*DocumentRoot' "$cfg" \
              | sed -E 's/^[[:space:]]*DocumentRoot[[:space:]]+//; s/[[:space:]]+.*$//')
    proxy=$(grep -E '^[[:space:]]*ProxyPass[[:space:]]' "$cfg" \
            | sed -E 's/^[[:space:]]*ProxyPass[[:space:]]+//; s/[[:space:]]+.*$//' | head -1)
    out="${C_GRN}${sname:-NO ServerName}${C_RESET}  ${C_DIM}${docroot:-no DocumentRoot}${C_RESET}"
    [ -n "$proxy" ] && out="$out  ${C_YLW}→ proxy: ${proxy}${C_RESET}"
    printf "  %s\n" "$out"
  done
}

if has nginx; then
  st=$(systemctl is-active nginx 2>/dev/null || echo "unknown")
  if systemctl is-enabled nginx >/dev/null 2>&1; then en="enabled"; else en="disabled"; fi
  print_row "nginx" "${C_GRN}$st${C_RESET} (${C_DIM}$en${C_RESET})"
  nginx_sites
fi
if has apache2 || has httpd; then
  [ -x /usr/sbin/httpd ] && apache_bin=httpd || apache_bin=apache2
  st=$(systemctl is-active "$apache_bin" 2>/dev/null || echo "unknown")
  if systemctl is-enabled "$apache_bin" >/dev/null 2>&1; then en="enabled"; else en="disabled"; fi
  print_row "apache" "${C_GRN}$st${C_RESET} (${C_DIM}$en${C_RESET})"
  apache_sites
fi
if ! has nginx && ! has apache2 && ! has httpd; then
  print_row "Web server" "${C_DIM}none installed${C_RESET}"
fi

# --- Open ports ------------------------------------------------------
section "Open ports"
if has ss; then
  PW1=7; PW2=22; PW3=10; PW4=26
  tborder $PW1 $PW2 $PW3 $PW4
  printf "  ${C_DIM}| %-${PW1}s | %-${PW2}s | %-${PW3}s | %-${PW4}s |${C_RESET}\n" "PROTO" "ADDRESS:PORT" "STATE" "PROCESS"
  tborder $PW1 $PW2 $PW3 $PW4
  ss_cmd=(ss -ltunpH)
  if has sudo && sudo -n true 2>/dev/null; then ss_cmd=(sudo -n ss -ltunpH); fi
  "${ss_cmd[@]}" 2>/dev/null | \
  while read -r netid state _ _ local _ proc; do
    prog=$(printf '%s' "$proc" | sed -E 's/.*users:\(\("([^"]+)".*/\1/')
    [ -n "$prog" ] && prog=$(trunc "$prog" $PW4) || prog="-"
    checks_total=$((checks_total + 1))
    printf "  | %-${PW1}s | %-${PW2}s | %-${PW3}s | %-${PW4}s |\n" \
      "${netid:-?}" "$(trunc "$local" $PW2)" "${state:-?}" "$prog"
  done
  tborder $PW1 $PW2 $PW3 $PW4
else
  print_row "Open ports" "${C_DIM}ss unavailable${C_RESET}"
fi

# --- Maintenance -----------------------------------------------------
section "Maintenance"
if [ "$updates_pending" -gt 0 ]; then
  upcol="${C_YLW}"; [ "$updates_pending" -ge 10 ] && upcol="${C_RED}"
  print_row "Updates"  "${upcol}${updates_pending} pending${C_RESET}"
else
  print_row "Updates"  "${C_GRN}up to date${C_RESET}"
fi
if [ "$reboot_req" = "yes" ]; then
  print_row "Reboot" "${C_RED}required${C_RESET}"
else
  print_row "Reboot" "${C_GRN}not needed${C_RESET}"
fi
if df -x tmpfs -x devtmpfs -x overlay -x squashfs --output=target,ipcent >/dev/null 2>&1; then
  inode_max=0; inode_top=""
  while read -r m ip; do
    ip=${ip%\%}
    [[ "$ip" =~ ^[0-9]+$ ]] || continue
    if [ "$ip" -gt "$inode_max" ]; then inode_max=$ip; inode_top=$m; fi
  done < <(df -x tmpfs -x devtmpfs -x overlay -x squashfs --output=target,ipcent 2>/dev/null | tail -n +2)
  if [ "$inode_max" -ge 90 ]; then
    print_row "Inodes" "${C_RED}${inode_top} at ${inode_max}%${C_RESET}"
  else
    print_row "Inodes" "${C_GRN}ok (max ${inode_max}%)${C_RESET}"
  fi
fi

# --- Development tools ----------------------------------------------
section "Dev tools"
tools=(
  "node:node --version"
  "npm:npm --version"
  "yarn:yarn --version"
  "pnpm:pnpm --version"
  "bun:bun --version"
  "java:java -version"
  "python3:python3 --version"
  "pip3:pip3 --version"
  "go:go version"
  "rustc:rustc --version"
  "cargo:cargo --version"
  "gcc:gcc --version"
  "g++:g++ --version"
  "make:make --version"
  "cmake:cmake --version"
  "php:php --version"
  "composer:composer --version"
  "ruby:ruby --version"
  "perl:perl -v"
)
for entry in "${tools[@]}"; do
  IFS=':' read -r name cmd <<< "$entry"
  checks_total=$((checks_total + 1))
  if has "$name"; then
    ver=$($cmd 2>&1 | sed -n '/[^[:space:]]/{p;q}')
    printf "  ${C_GRN}%-10s${C_RESET} %s\n" "$name" "$ver"
  else
    printf "  %-10s ${C_DIM}not installed${C_RESET}\n" "$name"
  fi
done

# --- footer ----------------------------------------------------------
el_ms=$(( $(now_ms) - _start_ms ))
if [ "$_start_ms" -lt 1000000000 ]; then el_txt="${el_ms}s"
else el_txt=$(awk -v ms="$el_ms" 'BEGIN{printf "%.1fs", ms/1000}'); fi
printf "  ${C_GRN}[+]${C_RESET} %s checks retrieved - %s - public IP: ${C_DIM}%s${C_RESET}\n" \
  "$checks_total" "$el_txt" "${NET_SRC:-n/a}"
echo
} > "$OUTTMP" 2>/dev/null   # ---------- framed body ends ----------

frame "$OUTTMP"
rm -f "$OUTTMP"