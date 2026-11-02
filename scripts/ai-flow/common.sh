#!/usr/bin/env bash
# Shared helpers for scripts/ai-flow/*.sh. Sourced, never executed directly.
#
# Works on Linux, macOS and Windows (Git Bash). Only dependency: the GitHub CLI `gh`,
# authenticated with an account that is admin of the repository (`gh auth login`).

set -euo pipefail

# Git Bash rewrites arguments that look like POSIX paths; `gh api` paths must stay as is.
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'

AI_FLOW_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

info() { printf '  %s\n' "$*"; }
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }
warn() { printf '  \033[33m!\033[0m %s\n' "$*" >&2; }
die()  { printf '  \033[31m✗\033[0m %s\n' "$*" >&2; exit 1; }
step() { printf '\n\033[1m%s\033[0m\n' "$*"; }

# Loads .ai-flow/flow.env (CR stripped, in case of a CRLF checkout). An AI_FLOW_REPO
# already set in the environment wins, to target a fork or a test repository.
load_flow_env() {
  local forced_repo="${AI_FLOW_REPO:-}"
  set -a
  # shellcheck disable=SC1090
  source <(tr -d '\r' < "$AI_FLOW_ROOT/.ai-flow/flow.env")
  set +a
  if [ -n "$forced_repo" ]; then AI_FLOW_REPO="$forced_repo"; fi
  REPO="$AI_FLOW_REPO"
}

# gh output without CR (gh.exe under Git Bash).
ghq() { gh "$@" | tr -d '\r'; }

require_gh() {
  command -v gh > /dev/null 2>&1 || die "GitHub CLI introuvable : installez-le (https://cli.github.com) puis lancez « gh auth login »."
  gh auth status > /dev/null 2>&1 || die "gh n'est pas connecté : lancez « gh auth login »."
  local admin
  admin="$(ghq api "repos/$REPO" --jq '.permissions.admin' 2> /dev/null || true)"
  [ "$admin" = "true" ] || die "Le compte connecté à gh n'est pas admin de $REPO (ou le repo est introuvable)."
}

# Prints every label line of .ai-flow/labels.txt as: name<TAB>color<TAB>role<TAB>description
read_labels() {
  tr -d '\r' < "$AI_FLOW_ROOT/.ai-flow/labels.txt" | while IFS= read -r line; do
    case "$line" in '' | '#'*) continue ;; esac
    local name color role desc
    IFS='|' read -r name color role desc <<< "$line"
    printf '%s\t%s\t%s\t%s\n' "$(trim "$name")" "$(trim "$color")" "$(trim "$role")" "$(trim "$desc")"
  done
}

trim() {
  local s="$1"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "$s"
}

# JSON string escaping for the few values we inline into request bodies.
json_str() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  printf '"%s"' "$s"
}
