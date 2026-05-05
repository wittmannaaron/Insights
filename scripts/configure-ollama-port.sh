#!/usr/bin/env bash
#
# Configure the local Ollama daemon to bind 127.0.0.1:11435 instead of the
# default 11434 so Insights can take 11434 transparently. Re-runnable.
#
# Assumes the userspace install (GUI Ollama.app + ~/Library/LaunchAgents/
# environment.plist) per ~/.claude/.../reference_ollama_setup.md.
#
# Usage:
#   bash scripts/configure-ollama-port.sh         # apply
#   bash scripts/configure-ollama-port.sh --revert  # restore default

set -euo pipefail

PLIST="$HOME/Library/LaunchAgents/environment.plist"
LABEL="my.ollama.env"
UID_=$(id -u)
TARGET_DOMAIN="gui/$UID_"

current_args() {
  /usr/libexec/PlistBuddy -c "Print :ProgramArguments:2" "$PLIST" 2>/dev/null || true
}

apply() {
  if [[ ! -f "$PLIST" ]]; then
    echo "error: $PLIST not found — Ollama tuning agent missing" >&2
    exit 1
  fi
  local cur
  cur=$(current_args)
  local desired="launchctl setenv OLLAMA_FLASH_ATTENTION 1; launchctl setenv OLLAMA_KV_CACHE_TYPE q8_0; launchctl setenv OLLAMA_HOST 127.0.0.1:11435"
  if [[ "$cur" == "$desired" ]]; then
    echo "plist already configured for port 11435"
  else
    /usr/libexec/PlistBuddy -c "Set :ProgramArguments:2 '$desired'" "$PLIST"
    echo "plist updated"
  fi

  launchctl bootout "$TARGET_DOMAIN/$LABEL" 2>/dev/null || true
  launchctl bootstrap "$TARGET_DOMAIN" "$PLIST"
  launchctl kickstart -k "$TARGET_DOMAIN/$LABEL"

  if pgrep -f '/Applications/Ollama.app/Contents/MacOS/Ollama' >/dev/null; then
    echo "restarting Ollama.app to pick up new env"
    kill -TERM "$(pgrep -f '/Applications/Ollama.app/Contents/MacOS/Ollama' | head -1)" || true
    sleep 4
    open -a Ollama
  else
    echo "Ollama.app not running — start it manually when ready"
  fi

  sleep 2
  if curl -sf "http://127.0.0.1:11435/api/tags" >/dev/null; then
    echo "verified: Ollama listens on 127.0.0.1:11435"
  else
    echo "warning: could not reach Ollama on 11435 — check 'launchctl getenv OLLAMA_HOST'"
  fi
}

revert() {
  local desired="launchctl setenv OLLAMA_FLASH_ATTENTION 1; launchctl setenv OLLAMA_KV_CACHE_TYPE q8_0"
  /usr/libexec/PlistBuddy -c "Set :ProgramArguments:2 '$desired'" "$PLIST"
  launchctl bootout "$TARGET_DOMAIN/$LABEL" 2>/dev/null || true
  launchctl bootstrap "$TARGET_DOMAIN" "$PLIST"
  launchctl kickstart -k "$TARGET_DOMAIN/$LABEL"
  launchctl unsetenv OLLAMA_HOST 2>/dev/null || true
  if pgrep -f '/Applications/Ollama.app/Contents/MacOS/Ollama' >/dev/null; then
    kill -TERM "$(pgrep -f '/Applications/Ollama.app/Contents/MacOS/Ollama' | head -1)" || true
    sleep 4
    open -a Ollama
  fi
  echo "reverted: Ollama back on default port 11434"
}

case "${1:-apply}" in
  apply) apply ;;
  --revert|revert) revert ;;
  *) echo "usage: $0 [apply|--revert]" >&2; exit 2 ;;
esac
