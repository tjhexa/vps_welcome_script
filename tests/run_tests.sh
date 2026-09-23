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

# ---- 11. vpsinfo-gui end-to-end (release binary; skip if not buildable) ----
GUI="$ROOT/gui/target/release/vpsinfo-gui"
if [ ! -x "$GUI" ]; then
  if has cargo; then
    echo "  note: building vpsinfo-gui release binary for e2e tests (first run)"
    (cd "$ROOT/gui" && cargo build --release) >/dev/null 2>&1 || true
  fi
fi
if [ -x "$GUI" ]; then
  # baked export vs config-driven output parity (structure tokens, volatile values stripped)
  "$GUI" --generate --path "$TMP/e2e.conf" --preset docker-host --frame 1 --color always >/dev/null 2>&1
  "$GUI" --export --path "$TMP/e2e-baked.sh" --preset docker-host --frame 1 --color always >/dev/null 2>&1
  if [ -f "$TMP/e2e-baked.sh" ] && [ -f "$TMP/e2e.conf" ]; then
    HOME="$TMP/home2" bash "$TMP/e2e-baked.sh" > "$TMP/parity-baked.txt" 2>/dev/null || true
    HOME="$TMP/home2" VPSINFO_CONF="$TMP/e2e.conf" VPSINFO_COLOR=always bash "$SCRIPT" > "$TMP/parity-cfg.txt" 2>/dev/null || true
    strip_ansi() { sed 's/\x1b\[[0-9;]*m//g' "$1"; }
    tok() { strip_ansi "$1" | grep '|' | grep -v '^+' | sed -E 's/^\|[[:space:]]*([^ ]+).*/\1/' | sort -u; }
    strip_ansi "$TMP/parity-baked.txt" > "$TMP/pb.txt"; strip_ansi "$TMP/parity-cfg.txt" > "$TMP/pc.txt"
    tok "$TMP/parity-baked.txt" > "$TMP/pb.tok"; tok "$TMP/parity-cfg.txt" > "$TMP/pc.tok"
    if diff -q "$TMP/pb.tok" "$TMP/pc.tok" >/dev/null; then
      ok "baked export == config-driven output (structure parity)"
    else
      bad "baked export == config-driven output"  # shellcheck disable=SC2154
    fi
  else
    bad "e2e generate/export produced expected files"
  fi

  # rc round-trip on a temp HOME
  mkdir -p "$TMP/home2"
  printf '%s\n' '#!/usr/bin/env bash' 'echo hi' > "$TMP/home2/.bashrc"
  HOME="$TMP/home2" "$GUI" --rc-add >/dev/null 2>&1
  HOME="$TMP/home2" "$GUI" --check-rc >/dev/null 2>&1
  [ $? -eq 0 ] && ok "cli --rc-add + --check-rc exit 0" || bad "cli --rc-add + --check-rc exit 0"
  grep -q 'vpsinfo:start' "$TMP/home2/.bashrc" && ok "cli rc-add installs marker block" || bad "cli rc-add installs marker block"
  HOME="$TMP/home2" "$GUI" --rc-remove >/dev/null 2>&1
  HOME="$TMP/home2" "$GUI" --check-rc >/dev/null 2>&1
  [ $? -eq 1 ] && ok "cli --rc-remove + --check-rc exit 1" || bad "cli --rc-remove + --check-rc exit 1"
  grep -q '^echo hi$' "$TMP/home2/.bashrc" && ok "cli rc-remove preserves user content" || bad "cli rc-remove preserves user content"
  ls "$TMP/home2"/.bashrc.vpsinfo.bak.* >/dev/null 2>&1 && ok "cli rc edits leave rotating backups" || bad "cli rc edits leave rotating backups"

  # v2: --dry-run writes nothing to disk
  mkdir -p "$TMP/home3"
  HOME="$TMP/home3" "$GUI" --generate --dry-run --path "$TMP/home3/never.sh" > "$TMP/dr-gen.txt" 2>/dev/null
  [ ! -f "$TMP/home3/never.sh" ] && grep -q 'VPSINFO_SHOW_SUMMARY=' "$TMP/dr-gen.txt" \
    && ok "cli --generate --dry-run prints, writes nothing" || bad "cli --generate --dry-run prints, writes nothing"
  HOME="$TMP/home3" "$GUI" --export --dry-run > "$TMP/dr-exp.txt" 2>/dev/null
  grep -q 'VPSINFO_NO_CONFIG=1' "$TMP/dr-exp.txt" && grep -q '#!/usr/bin/env bash' "$TMP/dr-exp.txt" \
    && ok "cli --export --dry-run prints the baked script" || bad "cli --export --dry-run prints the baked script"
  HOME="$TMP/home3" "$GUI" --rc-add --dry-run > "$TMP/dr-rc.txt" 2>/dev/null
  grep -q 'vpsinfo:start' "$TMP/dr-rc.txt" && ! grep -q 'vpsinfo:start' "$TMP/home3/.bashrc" 2>/dev/null \
    && ok "cli --rc-add --dry-run prints the block, edits nothing" || bad "cli --rc-add --dry-run prints the block, edits nothing"

  # v2: new presets -> expected section flags
  HOME="$TMP/home3" "$GUI" --generate --dry-run --preset vps-only | grep -q 'VPSINFO_SHOW_PORTS=1' \
    && HOME="$TMP/home3" "$GUI" --generate --dry-run --preset vps-only | grep -q 'VPSINFO_SHOW_TOOLS=0' \
    && ok "preset vps-only (ports=1, tools=0)" || bad "preset vps-only (ports=1, tools=0)"
  HOME="$TMP/home3" "$GUI" --generate --dry-run --preset dev-box | grep -q 'VPSINFO_SHOW_NETWORK=0' \
    && HOME="$TMP/home3" "$GUI" --generate --dry-run --preset dev-box | grep -q 'VPSINFO_SHOW_TOOLS=1' \
    && ok "preset dev-box (network=0, tools=1)" || bad "preset dev-box (network=0, tools=1)"

  # v2: profile JSON export/import round-trip
  mkdir -p "$TMP/home4"
  HOME="$TMP/home4" "$GUI" --export-json --preset desktop-only --path "$TMP/home4/p.json" >/dev/null 2>&1
  grep -q '"script_path"' "$TMP/home4/p.json" && ok "cli --export-json writes a JSON profile" || bad "cli --export-json writes a JSON profile"
  HOME="$TMP/home4" "$GUI" --import-json --path "$TMP/home4/p.json" >/dev/null 2>&1
  HOME="$TMP/home4" "$GUI" --generate --dry-run > "$TMP/home4/after.txt" 2>/dev/null
  grep -q 'VPSINFO_SHOW_DOCKER=0' "$TMP/home4/after.txt" && grep -q 'VPSINFO_SHOW_TOOLS=1' "$TMP/home4/after.txt" \
    && ok "cli --import-json persists and drives the next generate" || bad "cli --import-json persists and drives the next generate"

  # v2: autodetect existing config on a fresh HOME
  mkdir -p "$TMP/home5/.config/vpsinfo"
  printf 'VPSINFO_SHOW_DOCKER=1\nVPSINFO_SHOW_WEB=0\nVPSINFO_FRAME=1\n' > "$TMP/home5/.config/vpsinfo/vpsinfo.conf"
  HOME="$TMP/home5" "$GUI" --generate --dry-run > "$TMP/home5/auto.txt" 2>/dev/null
  grep -q 'VPSINFO_SHOW_DOCKER=1' "$TMP/home5/auto.txt" && grep -q 'VPSINFO_FRAME=1' "$TMP/home5/auto.txt" \
    && ok "fresh launch autodetects an existing vpsinfo.conf" || bad "fresh launch autodetects an existing vpsinfo.conf"
else
  echo "  SKIP  e2e gui tests (release binary not buildable locally)"
fi

echo
echo "PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ]