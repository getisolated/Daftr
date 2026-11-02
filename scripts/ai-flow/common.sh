#!/usr/bin/env bash
# Shared helpers for scripts/ai-flow/*.sh. Sourced, never executed directly.
#
# Works on Linux, macOS and Windows (Git Bash). Only dependency: the GitHub CLI `gh`,
# authenticated with an account that is admin of the repository (`gh auth login`).

set -euo pipefail

# Git Bash rewrites arguments that look like POSIX paths; `gh api` paths must stay as is.
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'

# AI_FLOW_ROOT: where the flow's own files are (.ai-flow/flow.env, labels, forbidden paths).
# AI_FLOW_WORKDIR: the git checkout the flow works on (run folders, commits). They are the
# same folder locally; in an agent job the tools come from main (.ai-flow-tools/) while the
# work tree is the flow branch (see ai-agent-run.yml).
AI_FLOW_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
AI_FLOW_WORKDIR="${AI_FLOW_WORKDIR:-$AI_FLOW_ROOT}"

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

# Compares the App id of flow.env with GitHub's record of the App. A private App ("Only on
# this account") is invisible to the gh token (404): then nothing can be checked from here,
# and the "AI flow · self-test" workflow, which authenticates as the App, does it instead.
# Never reads an error body as an id: the output is used only when gh succeeded.
check_app_id() {
  local app_id
  if app_id="$(ghq api "apps/$AI_FLOW_APP_SLUG" --jq '.id' 2> /dev/null)"; then
    [ "$app_id" = "$AI_FLOW_APP_ID" ] \
      || die "L'App « $AI_FLOW_APP_SLUG » a l'id $app_id, .ai-flow/flow.env indique $AI_FLOW_APP_ID."
    ok "App $AI_FLOW_APP_SLUG (id $app_id)"
  else
    info "App $AI_FLOW_APP_SLUG privée : invisible d'ici, elle est vérifiée par l'auto-test (Actions)."
  fi
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
