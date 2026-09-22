#!/usr/bin/env bash
# ------------------------------------------------------------------
# vps-info.sh
# Quick overview of VPS / machine basic details.
#   - System, CPU, memory, disk, network
#   - Docker: installed, running containers (name/image/status/ports)
#   - Web server: nginx/apache status + sites with reverse-proxy info
#   - Dev tools: node/java/python/go/rust/... versions
# Source this from ~/.bashrc to have it run on every new login shell.
# ------------------------------------------------------------------
set -o pipefail

# --- Colors (only when safe: real terminal, sane TERM, NO_COLOR unset) --
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-dumb}" != "dumb" ]; then
  ESC=$(printf '\033')
  C_RESET="${ESC}[0m"; C_BOLD="${ESC}[1m"
  C_CYA="${ESC}[36m"; C_GRN="${ESC}[32m"; C_YLW="${ESC}[33m"; C_RED="${ESC}[31m"; C_BLU="${ESC}[34m"
  C_DIM="${ESC}[2m"
else
  C_RESET=""; C_BOLD=""; C_CYA=""; C_GRN=""; C_YLW=""; C_RED=""; C_BLU=""; C_DIM=""
fi

has() { command -v "$1" >/dev/null 2>&1; }

# --- Header ----------------------------------------------------------
print_row() { printf "${C_DIM}%-16s${C_RESET} %s\n" "$1" "$2"; }

echo
TITLE="VPS / SYSTEM INFO"
STAMP="$(date +'%a %b %d %H:%M')"
INNER_LEN=$(( ${#TITLE} + ${#STAMP} + 3 ))
BOX_LEN=$(( INNER_LEN + 2 ))
bar=$(printf '%*s' "$BOX_LEN" '' | tr ' ' '-')
printf "${C_BOLD}${C_CYA} .${bar}.${C_RESET}\n"
printf "${C_BOLD}${C_CYA} | ${C_RESET}${C_BOLD}%s${C_RESET}${C_DIM} - ${C_RESET}${C_BOLD}%s${C_RESET}${C_CYA} |${C_RESET}\n" "$TITLE" "$STAMP"
printf "${C_BOLD}${C_CYA} '${bar}'${C_RESET}\n"

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
cpu_cores=$(grep -c '^processor' /proc/cpuinfo)
print_row "CPU"         "${cpu_model:-unknown}"
print_row "Cores"       "${cpu_cores} (${C_DIM}$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo $cpu_cores) online${C_RESET})"

# CPU usage (idle delta over ~1s)
cpu_usage() {
  local prev_idle prev_total idle total diff_idle diff_total
  prev_total=0; prev_idle=0
  while read -r _ a b c d e f g h i j k; do
    idle=$((d + e)); total=$((a + b + c + idle + f + g + h + i + j + k))
    prev_idle=$idle; prev_total=$total
    break
  done < /proc/stat
  sleep 1
  while read -r _ a b c d e f g h i j k; do
    idle=$((d + e)); total=$((a + b + c + idle + f + g + h + i + j + k))
    break
  done < /proc/stat
  diff_idle=$((idle - prev_idle)); diff_total=$((total - prev_total))
  echo $((100 * (diff_total - diff_idle) / diff_total))
}
if has bc || true; then CPU_PCT=$(cpu_usage); else CPU_PCT="-"; fi
print_row "CPU usage"   "${CPU_PCT}% ${C_DIM}(1s sample)${C_RESET}"

# --- Memory ----------------------------------------------------------
mem_info() { awk -F': *' -v key="$1" '$0 ~ key {gsub(/kB/,""); print $2}' /proc/meminfo; }
mem_total=$(mem_info '^MemTotal'); mem_avail=$(mem_info '^MemAvailable')
mem_used=$((mem_total - mem_avail))
pct=$((100 * mem_used / mem_total))
color="${C_GRN}"; [ "$pct" -ge 80 ] && color="${C_RED}" || [ "$pct" -ge 60 ] && color="${C_YLW}"
print_row "Memory"      "${color}${pct}%${C_RESET} used  $((mem_used / 1024)) MB / $((mem_total / 1024)) MB"
swap_total=$(mem_info '^SwapTotal'); swap_free=$(mem_info '^SwapFree')
swap_used=$((swap_total - swap_free))
print_row "Swap"        "$((swap_used / 1024)) MB / $((swap_total / 1024)) MB"

# --- Disk ------------------------------------------------------------
printf "${C_DIM}%-16s${C_RESET}\n" "Disk"
df -h -x tmpfs -x devtmpfs -x overlay -x squashfs --output=target,size,used,pcent 2>/dev/null \
  | awk -v hdr="${C_DIM}" -v rst="${C_RESET}" \
      'NR==1{print "  "hdr $0 rst; next} {print "  "$0}' \
  || df -h | head -8

# --- Network ---------------------------------------------------------
lan=$(hostname -I 2>/dev/null | tr ' ' '\n' | grep -v '^$' | head -1)
[ -n "$lan" ] && print_row "LAN IP" "${lan}"
if [ -z "$NO_PUBLIC_IP" ] && has curl; then
  pub=$(curl -4 -s --max-time 3 https://ifconfig.me 2>/dev/null)
  if [ -z "$pub" ]; then pub=$(curl -6 -s --max-time 3 https://ifconfig.me 2>/dev/null); fi
  [ -n "$pub" ] && print_row "Public IP" "$pub" || print_row "Public IP" "${C_DIM}unreachable (offline?)${C_RESET}"
else
  print_row "Public IP" "${C_DIM}skipped (NO_PUBLIC_IP=1)${C_RESET}"
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
  failed=$(systemctl --failed --no-legend 2>/dev/null | wc -l)
  [ "$failed" -gt 0 ] && print_row "Failed srvcs" "${C_RED}$failed unit(s) FAILED${C_RESET}" \
                     || print_row "Failed srvcs" "${C_GRN}none${C_RESET}"
fi

# ======================================================================
#  EXTRA SECTIONS (docker / web server / dev tools)
# ======================================================================
section() { printf "${C_BOLD}${C_CYA}── %s${C_RESET}\n" "$1"; }

# --- Docker ----------------------------------------------------------
section "Docker"
if has docker; then
  print_row "Docker" "client $(docker --version 2>/dev/null | awk '{print $3}' | tr -d ',')"
  if docker info >/dev/null 2>&1; then
    running=$(docker ps -q | wc -l)
    total=$(docker ps -aq | wc -l)

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

    trunc() { local s="$1" n="$2"; [ "${#s}" -gt "$n" ] && s="${s:0:$((n-1))}.."; printf '%s' "$s"; }
    CW1=20; CW2=28; CW3=12
    border="+$(printf '%*s' $((CW1+2)) '' | tr ' ' '-')+$(printf '%*s' $((CW2+2)) '' | tr ' ' '-')+$(printf '%*s' $((CW3+2)) '' | tr ' ' '-')+"
    printf "  ${C_DIM}$border${C_RESET}\n"
    printf "  ${C_DIM}| %-${CW1}s | %-${CW2}s | %-${CW3}s |${C_RESET}\n" "NAME" "IMAGE" "STATUS"
    printf "  ${C_DIM}$border${C_RESET}\n"

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
        printf "  | %-${CW1}s | %-${CW2}s | %s%-${CW3}s%s |\n" \
          "$(trunc "$cname" $CW1)" "$(trunc "$cimage" $CW2)" "$col" "${tag}${dur:+ $dur}" "$C_RESET"
      done < <(docker ps --format '{{.Names}}|{{.Image}}|{{.State}}|{{.Status}}')
    else
      printf "  ${C_DIM}no containers running${C_RESET}\n"
    fi
    printf "  ${C_DIM}$border${C_RESET}\n"
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
  if has "$name"; then
    ver=$($cmd 2>&1 | sed -n '/[^[:space:]]/{p;q}')
    printf "  ${C_GRN}%-10s${C_RESET} %s\n" "$name" "$ver"
  else
    printf "  %-10s ${C_DIM}not installed${C_RESET}\n" "$name"
  fi
done

echo