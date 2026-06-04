#!/usr/bin/env bash
#
# gfit-cli — one-line installer for Claude Desktop (macOS).
#
#   curl -fsSL https://raw.githubusercontent.com/dickwu/gfit-cli/main/install.sh | bash
#
# What it does (Claude Desktop only — it never touches ~/.claude/skills / Claude Code):
#   1. downloads the gfit-cli binary for your Mac (latest release)
#   2. downloads + unpacks the Claude Desktop MCP extension (.mcpb, latest mcpb-v* release)
#   3. saves the "Project instructions" usage guide (and copies it to your clipboard)
#   4. registers the extension in claude_desktop_config.json — backing the file up first
#   5. walks you through the GFit browser sign-in
#   6. prints how to enable it (restart Desktop) + how to load the guide
#
# macOS only. On Linux/Windows, install manually — see the README.
#
# Every non-default tool it needs (curl, unzip, node) is checked first; if one is
# missing the script prints the exact Homebrew command (installing Homebrew itself
# first if it's absent) and stops.
#
# Env overrides (mostly for testing):
#   GFIT_PREFIX          binary install dir            (default: ~/.local/bin)
#   GFIT_MCPB_DIR        extension unpack dir          (default: ~/.local/share/gfit-cli-mcpb)
#   GFIT_DESKTOP_CONFIG  Claude Desktop config path    (default: ~/Library/Application Support/Claude/…)
#   GFIT_MCPB_TAG        pin a specific mcpb-v* release (default: auto = latest)
#   GFIT_RAW_BASE        raw base URL for the guide    (default: github raw, main)
#   GFIT_SKIP_LOGIN=1    skip the sign-in step

set -euo pipefail

REPO="dickwu/gfit-cli"
DEFAULT_MCPB_TAG="mcpb-v0.2.0"     # fallback if the GitHub API can't be reached
RAW_BASE="${GFIT_RAW_BASE:-https://raw.githubusercontent.com/${REPO}/main}"
PREFIX="${GFIT_PREFIX:-$HOME/.local/bin}"
MCPB_DIR="${GFIT_MCPB_DIR:-$HOME/.local/share/gfit-cli-mcpb}"

# ---------- pretty output ----------
if [ -t 1 ]; then B=$'\033[1m'; G=$'\033[32m'; Y=$'\033[33m'; R=$'\033[31m'; D=$'\033[2m'; N=$'\033[0m'
else B=""; G=""; Y=""; R=""; D=""; N=""; fi
say()  { printf '%s\n' "${B}==>${N} $*"; }
ok()   { printf '%s\n' "${G}  ok${N} $*"; }
warn() { printf '%s\n' "${Y}  ! ${N} $*"; }
die()  { printf '%s\n' "${R}error:${N} $*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# ---------- platform (macOS only) ----------
[ "$(uname -s)" = "Darwin" ] || die "this installer is macOS-only — on Linux/Windows install manually (see the README)"
case "$(uname -m)" in
  arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
  x86_64)        TARGET="x86_64-apple-darwin" ;;
  *) die "unsupported CPU arch '$(uname -m)'" ;;
esac
CFG="${GFIT_DESKTOP_CONFIG:-$HOME/Library/Application Support/Claude/claude_desktop_config.json}"

# ---------- dependency checks: verify a tool, else print how to install it (Homebrew) ----------
install_hint() {
  local tool="$1"
  if have brew; then
    echo "brew install $tool"
  else
    echo "install Homebrew, then the tool:"
    echo "         /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
    echo "         brew install $tool"
  fi
}
require() {
  if ! have "$1"; then
    warn "required tool '$1' is not installed."
    warn "install it with:"
    warn "    $(install_hint "$1")"
    die "missing dependency: $1"
  fi
}

say "Installing gfit-cli for Claude Desktop  ${D}(target: ${TARGET})${N}"
require curl

# ---------- 1. binary ----------
say "Downloading the gfit-cli binary…"
mkdir -p "$PREFIX"
BIN="$PREFIX/gfit-cli"
BIN_URL="https://github.com/${REPO}/releases/latest/download/gfit-cli-${TARGET}"
curl -fsSL -o "$BIN" "$BIN_URL" || die "download failed: $BIN_URL"
chmod +x "$BIN"
VER="$("$BIN" --version 2>/dev/null || true)"
[ -n "$VER" ] || die "the downloaded binary did not run (no --version output) — wrong target?"
ok "installed ${VER} → $BIN"
case ":$PATH:" in
  *":$PREFIX:"*) : ;;
  *) warn "to run 'gfit-cli' directly in a terminal, add:  export PATH=\"$PREFIX:\$PATH\"" ;;
esac

# ---------- 2. extension (.mcpb): resolve newest release, download, unpack+register if node exists ----------
if [ -n "${GFIT_MCPB_TAG:-}" ]; then
  MCPB_URL="https://github.com/${REPO}/releases/download/${GFIT_MCPB_TAG}/gfit-cli.mcpb"
else
  say "Finding the latest extension release…"
  MCPB_URL="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases" 2>/dev/null \
    | grep '"browser_download_url"' | grep 'gfit-cli\.mcpb"' \
    | sed -E 's/.*"(https[^"]+)".*/\1/' | head -1 || true)"
  [ -n "$MCPB_URL" ] || MCPB_URL="https://github.com/${REPO}/releases/download/${DEFAULT_MCPB_TAG}/gfit-cli.mcpb"
fi
MCPB_TAG="$(printf '%s' "$MCPB_URL" | sed -E 's#.*/download/([^/]+)/.*#\1#')"

say "Downloading the Claude Desktop extension (${MCPB_TAG})…"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
curl -fsSL -o "$TMP/gfit-cli.mcpb" "$MCPB_URL" || die "download failed: $MCPB_URL"
mkdir -p "$MCPB_DIR"
cp "$TMP/gfit-cli.mcpb" "$MCPB_DIR/gfit-cli.mcpb"   # always keep the file (needed for UI import)

# Find a Node runtime (the registered MCP server runs on `node`).
NODE=""
for c in node /opt/homebrew/bin/node /usr/local/bin/node; do
  if command -v "$c" >/dev/null 2>&1; then NODE="$(command -v "$c")"; break; fi
done

WIRED=0
if [ -n "$NODE" ]; then
  require unzip
  rm -rf "$MCPB_DIR/server" "$MCPB_DIR/node_modules"
  unzip -oq "$TMP/gfit-cli.mcpb" -d "$MCPB_DIR"
  [ -f "$MCPB_DIR/server/index.js" ] || die "extension is missing server/index.js after unpack"
  ok "extension unpacked → $MCPB_DIR"

  say "Registering the extension in Claude Desktop…"
  mkdir -p "$(dirname "$CFG")"
  if [ -f "$CFG" ]; then
    BAK="${CFG}.bak.$(date +%Y%m%d%H%M%S)"
    cp "$CFG" "$BAK"
    ok "backed up your existing config → $BAK"
  else
    printf '{}\n' > "$CFG"
  fi
  CFG_PATH="$CFG" NODE_ABS="$NODE" BIN_ABS="$BIN" SERVER="$MCPB_DIR/server/index.js" \
    "$NODE" <<'NODEEOF'
const fs = require("fs");
const p = process.env.CFG_PATH;
let cfg = {};
try { cfg = JSON.parse(fs.readFileSync(p, "utf8") || "{}"); } catch (e) { cfg = {}; }
if (typeof cfg !== "object" || cfg === null || Array.isArray(cfg)) cfg = {};
cfg.mcpServers = cfg.mcpServers || {};
cfg.mcpServers["gfit-cli"] = {
  command: process.env.NODE_ABS,
  args: [process.env.SERVER],
  env: {
    GFIT_CLI_PATH: process.env.BIN_ABS,
    GFIT_API_URL: "https://api.gfitwellness.ca",
  },
};
fs.writeFileSync(p, JSON.stringify(cfg, null, 2) + "\n");
process.stderr.write("registered mcpServers.gfit-cli\n");
NODEEOF
  ok "registered in $CFG"
  WIRED=1
else
  # No Node on the host. Claude Desktop ships its own Node for UI-installed
  # extensions, so the supported path here is a manual import of the .mcpb file.
  warn "Node.js not found — needed to auto-register the server on your machine."
  warn "Install it with:"
  warn "    $(install_hint node)"
  warn "…then re-run this script. Or skip Node and import the extension via the UI"
  warn "(Claude Desktop bundles its own Node for that):"
  warn "    Claude Desktop → Settings → Extensions → add $MCPB_DIR/gfit-cli.mcpb"
fi

# ---------- 3. usage guide ----------
say "Saving the Claude Desktop usage guide…"
GUIDE="$MCPB_DIR/claude-desktop-project.md"
if curl -fsSL -o "$GUIDE" "${RAW_BASE}/mcpb/claude-desktop-project.md"; then
  ok "guide saved → $GUIDE"
  if have pbcopy; then
    pbcopy < "$GUIDE" && ok "guide copied to your clipboard (paste into a Project)"
  fi
else
  warn "couldn't fetch the usage guide (continuing)"
  GUIDE=""
fi

# ---------- 4. sign in ----------
is_logged_in() { "$BIN" auth.status 2>/dev/null | grep -q "token:[[:space:]]*present"; }
LOGGED_IN=0
if is_logged_in; then
  LOGGED_IN=1
  ok "already signed in ($("$BIN" auth.status 2>/dev/null | sed -n 's/^email:[[:space:]]*//p'))"
elif [ "${GFIT_SKIP_LOGIN:-0}" != "1" ]; then
  say "Sign in to GFit (opens your browser)…"
  do_login=1
  if [ -r /dev/tty ]; then
    printf '%s' "  Press Enter to sign in now, or type 's' to skip: " > /dev/tty
    read -r ans < /dev/tty || ans=""
    case "$ans" in s|S) do_login=0 ;; esac
  fi
  if [ "$do_login" = "1" ]; then
    "$BIN" auth.login || warn "sign-in didn't complete — you can do it later (the extension prompts on first use)"
    if is_logged_in; then LOGGED_IN=1; ok "signed in"; fi
  else
    warn "skipped — the extension will open the sign-in page on first use"
  fi
fi

# ---------- 5. enable + import steps ----------
printf '\n'
say "${G}Done — gfit-cli is installed for Claude Desktop.${N}"
printf '\n%s\n' "${B}Enable it${N}"
printf '  1. %s\n' "Fully quit Claude Desktop (Cmd+Q) and reopen it."
if [ "$WIRED" = "1" ]; then
  printf '     %s\n' "→ it loads the gfit-cli tools automatically."
else
  printf '     %s\n' "→ then import the extension: Settings → Extensions → add"
  printf '       %s\n' "$MCPB_DIR/gfit-cli.mcpb"
fi
if [ -n "$GUIDE" ]; then
  printf '  2. %s\n' "Load the usage guide (recommended):"
  printf '     %s\n' "Claude Desktop → Projects → new Project → Instructions → paste the guide"
  printf '     %s\n' "${D}(it's already on your clipboard; or open ${GUIDE})${N}"
fi
printf '  3. %s\n' "Try it — ask Claude:  ${B}\"list my GFit clients\"${N}"
if [ "$LOGGED_IN" = "1" ]; then
  printf '     %s\n' "You're already signed in."
else
  printf '     %s\n' "If you're not signed in, it opens the GFit sign-in page in your browser — sign in once."
fi
printf '\n%s\n' "${D}Tools: gfit_list_commands · gfit_help · gfit_auth_status · gfit_login · gfit_run"
printf '%s\n'   "Config: $CFG"
printf '%s\n'   "Uninstall: remove the mcpServers.gfit-cli key from that file, then rm -rf $MCPB_DIR $BIN${N}"
