#!/usr/bin/env bash
# v1 regression tests for vps-info.sh (run from repo root: bash tests/run_tests.sh)
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$ROOT/vps-info.sh"
PASS=0; FAIL=0
ok()  { PASS=$((PASS+1)); printf '  ok    %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf '  FAIL  %s\n' "$1"; }
check() { local n="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$n"; else bad "$n"; fi }
has() { command -v "$1" >/dev/null 2>&1; }

TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
OUT="$TMP/out.txt"

echo "== vps-info.sh regression tests =="

# ---- 1. syntax -------------------------------------------------
check "bash -n passes" bash -n "$SCRIPT"

# ---- 2. piped output contains no escape sequences --------------
NO_PUBLIC_IP=1 VPSINFO_SKIP_UPDATES=1 VPSINFO_SKIP_CPU=1 VPSINFO_NO_CONFIG=1 \
  bash "$SCRIPT" > "$OUT" 2>/dev/null
if grep -q $'\x1b' "$OUT"; then bad "piped output is plain"; else ok "piped output is plain"; fi

# ---- 3. defaults show every section -----------------------------
secs=$(sed -n 's/^[[:space:]]*|.*── \([^|]*\)|.*/\1/p' "$OUT" | sed 's/[[:space:]]*$//' | sort -u)
want=(Disk Maintenance "Open ports" "Web server" "Dev tools")
if has docker; then want+=("Docker"); fi
if has fail2ban-client; then want+=("Security"); fi
for s in "${want[@]}"; do
  if printf '%s\n' "$secs" | grep -qx "$s"; then ok "default shows section: $s"; else bad "default shows section: $s"; fi
done

# ---- 4. old vs new structure parity (baseline from git HEAD) ----
git -C "$ROOT" show HEAD:vps-info.sh > "$TMP/old.sh" 2>/dev/null
if [ -s "$TMP/old.sh" ]; then
  NO_PUBLIC_IP=1 VPSINFO_SKIP_UPDATES=1 VPSINFO_SKIP_CPU=1 VPSINFO_NO_CONFIG=1 \
    bash "$TMP/old.sh" > "$TMP/old.txt" 2>/dev/null
  tok() { grep '|' "$1" | grep -v '^+' | sed -E 's/^\|[[:space:]]*([^ ]+).*/\1/' | sort -u; }
  tok "$OUT" > "$TMP/new.tok"; tok "$TMP/old.txt" > "$TMP/old.tok"
  if diff -q "$TMP/new.tok" "$TMP/old.tok" >/dev/null; then
    ok "structure parity vs git HEAD (same row labels/headers)"
  else
    bad "structure parity vs git HEAD"; echo "--- only in NEW:"; comm -13 "$TMP/old.tok" "$TMP/new.tok"; echo "--- only in OLD:"; comm -23 "$TMP/old.tok" "$TMP/new.tok"
  fi
else
  bad "could not load git HEAD baseline"
fi

# ---- 5. all-off except System: only system rows -----------------
NO_PUBLIC_IP=1 VPSINFO_SKIP_UPDATES=1 VPSINFO_SKIP_CPU=1 VPSINFO_NO_CONFIG=1 \
  VPSINFO_SHOW_SUMMARY=0 VPSINFO_SHOW_BANNER=0 VPSINFO_SHOW_SYSTEM=1 \
  VPSINFO_SHOW_CPU=0 VPSINFO_SHOW_MEMORY=0 VPSINFO_SHOW_DISK=0 VPSINFO_SHOW_NETWORK=0 \
  VPSINFO_SHOW_PROCESSES=0 VPSINFO_SHOW_DOCKER=0 VPSINFO_SHOW_WEB=0 VPSINFO_SHOW_PORTS=0 \
  VPSINFO_SHOW_SECURITY=0 VPSINFO_SHOW_MAINT=0 VPSINFO_SHOW_TOOLS=0 VPSINFO_SHOW_FOOTER=0 \
  bash "$SCRIPT" > "$OUT" 2>/dev/null
if grep -q '──' "$OUT"; then bad "all-off hides all sections"; else ok "all-off hides all sections"; fi
grep -q 'Hostname' "$OUT" && ok "system rows present when System=1" || bad "system rows present when System=1"

# ---- 6. frame off: first char is not '+' ------------------------
VPSINFO_FRAME=0 VPSINFO_NO_CONFIG=1 VPSINFO_SHOW_SUMMARY=0 VPSINFO_SHOW_BANNER=0 \
  VPSINFO_SKIP_UPDATES=1 VPSINFO_SKIP_CPU=1 NO_PUBLIC_IP=1 \
  bash "$SCRIPT" > "$OUT" 2>/dev/null
if head -c1 "$OUT" | grep -q '+'; then bad "VPSINFO_FRAME=0 removes frame"; else ok "VPSINFO_FRAME=0 removes frame"; fi

# ---- 7. config file drives sections ------------------------------
printf 'VPSINFO_SHOW_DOCKER=0\nVPSINFO_SHOW_SUMMARY=0\nVPSINFO_SHOW_BANNER=0\nNO_PUBLIC_IP=1\nVPSINFO_SKIP_UPDATES=1\nVPSINFO_SKIP_CPU=1\n' > "$TMP/c.conf"
VPSINFO_CONF="$TMP/c.conf" VPSINFO_COLOR=never bash "$SCRIPT" > "$OUT" 2>/dev/null
grep -q '── Docker' "$OUT" && bad "config disables Docker" || ok "config disables Docker"
# env wins over config
VPSINFO_CONF="$TMP/c.conf" VPSINFO_SHOW_DOCKER=1 VPSINFO_COLOR=never bash "$SCRIPT" > "$OUT" 2>/dev/null
grep -q '── Docker' "$OUT" && ok "env wins over config (Docker back on)" || bad "env wins over config (Docker back on)"

# ---- 8. VPSINFO_NO_CONFIG ignores the file -----------------------
VPSINFO_CONF="$TMP/c.conf" VPSINFO_NO_CONFIG=1 VPSINFO_COLOR=never bash "$SCRIPT" > "$OUT" 2>/dev/null
grep -q '── Docker' "$OUT" && ok "VPSINFO_NO_CONFIG ignores config (Docker on by default)" \
  || bad "VPSINFO_NO_CONFIG ignores config (Docker on by default)"

# ---- 9. color modes ----------------------------------------------
VPSINFO_COLOR=always VPSINFO_NO_CONFIG=1 VPSINFO_SKIP_UPDATES=1 NO_PUBLIC_IP=1 VPSINFO_SKIP_CPU=1 \
  bash "$SCRIPT" 2>/dev/null | grep -q $'\x1b\[' && ok "VPSINFO_COLOR=always forces colors when piped" \
  || bad "VPSINFO_COLOR=always forces colors when piped"
VPSINFO_COLOR=never VPSINFO_NO_CONFIG=1 VPSINFO_SKIP_UPDATES=1 NO_PUBLIC_IP=1 VPSINFO_SKIP_CPU=1 \
  bash "$SCRIPT" 2>/dev/null | grep -q $'\x1b\[' && bad "VPSINFO_COLOR=never disables colors" \
  || ok "VPSINFO_COLOR=never disables colors"

# ---- 10. rc marker-block add/remove round-trip --------------------
printf '%s\n' '#!/usr/bin/env bash' 'echo hi' > "$TMP/.bashrc"
RC="$TMP/.bashrc"
BLOCK_START='# ---- vpsinfo:start ----'
BLOCK_END='# ---- vpsinfo:end ----'
if ! grep -q 'vpsinfo:start' "$RC"; then
  printf '\n%s\nif [ -n "${SSH_CONNECTION:-}" ]; then\n  %s\nfi\n%s\n' \
    "$BLOCK_START" "$TMP/vps-info.sh" "$BLOCK_END" >> "$RC"
fi
[ "$(grep -c 'vpsinfo:start' "$RC")" = "1" ] \
  && ok "rc block idempotent (single marker after one add)" \
  || bad "rc block idempotent (single marker after one add)"
# remove only our block
awk -v s="$BLOCK_START" -v e="$BLOCK_END" '
  $0 ~ s {skip=1} !skip {print} $0 ~ e {skip=0}
' "$RC" > "$TMP/.bashrc.new" && mv "$TMP/.bashrc.new" "$RC"
grep -q 'vpsinfo:start' "$RC" && bad "rc remove strips only our block" \
  || { ok "rc remove strips only our block"; grep -q '^echo hi$' "$RC" && ok "rc remove preserves user content" || bad "rc remove preserves user content"; }

echo
echo "PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ]