#!/usr/bin/env bash
# Runs after an agent, before the finalisation: checks what the agent changed, records its
# output in the run folder, commits what is left uncommitted, and pushes the flow branch.
#
#   bash scripts/ai-flow/collect.sh
#
# Environment:
#   AI_FLOW_AGENT, AI_FLOW_ISSUE, AI_FLOW_TOUR, AI_FLOW_RUN_DIR, AI_FLOW_BRANCH
#   AI_FLOW_ZONE        "run": the agent may only write in its run folder (specs, plan…)
#                       "code": anywhere except .ai-flow/forbidden-paths.txt (tests, code…)
#   AI_FLOW_BASE_SHA    HEAD before the agent ran
#   STRUCTURED_OUTPUT   the agent's structured result (JSON), to archive the questions
#   GITHUB_OUTPUT       receives `forbidden` (one path per line) and `push_error`
#
# A change outside the allowed zone blocks everything: nothing is committed or pushed, and
# the finalisation parks the issue in ai:erreur with the list of paths.

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

: "${AI_FLOW_AGENT:?}" "${AI_FLOW_ISSUE:?}" "${AI_FLOW_TOUR:?}" "${AI_FLOW_RUN_DIR:?}" "${AI_FLOW_BRANCH:?}" "${AI_FLOW_BASE_SHA:?}"
zone="${AI_FLOW_ZONE:-run}"
out="${GITHUB_OUTPUT:-/dev/stdout}"
run_dir="${AI_FLOW_RUN_DIR%/}/"

emit() { # name value (value may span lines)
  local delimiter="EOF_$RANDOM$RANDOM"
  printf '%s<<%s\n%s\n%s\n' "$1" "$delimiter" "$2" "$delimiter" >> "$out"
}

cd "$AI_FLOW_WORKDIR"

# Archive the questions of the specs agent next to its other outputs.
status="$(jq -r '.status // empty' <<< "${STRUCTURED_OUTPUT:-}" 2> /dev/null || true)"
if [ "$AI_FLOW_AGENT" = "specs" ] && [ "$status" = "QUESTIONS" ]; then
  mkdir -p "$run_dir"
  jq -r '.comment' <<< "$STRUCTURED_OUTPUT" > "${run_dir}questions-${AI_FLOW_TOUR}.md"
fi

# Every path changed since the base: committed by the agent, modified, deleted or new.
changed="$( { git diff --name-only "$AI_FLOW_BASE_SHA"; git ls-files --others --exclude-standard; } | sort -u)"

mapfile -t prefixes < <(tr -d '\r' < "$AI_FLOW_ROOT/.ai-flow/forbidden-paths.txt" | sed -e 's/[[:space:]]*$//' | grep -v -e '^#' -e '^$')

forbidden=()
while IFS= read -r path; do
  [ -n "$path" ] || continue
  case "$path" in "$run_dir"*) continue ;; esac
  if [ "$zone" = "run" ]; then
    forbidden+=("$path")
    continue
  fi
  for prefix in "${prefixes[@]}"; do
    if [[ "$prefix" == */ && "$path" == "$prefix"* ]] || [ "$path" = "$prefix" ]; then
      forbidden+=("$path")
      break
    fi
  done
done <<< "$changed"

if [ "${#forbidden[@]}" -gt 0 ]; then
  printf '::error::Changes outside the allowed zone (%s): %s\n' "$zone" "${forbidden[*]}"
  emit forbidden "$(printf '%s\n' "${forbidden[@]}")"
  emit push_error ""
  exit 0
fi
emit forbidden ""

# Commit what the agent left uncommitted (always the run folder; in the code zone, the
# rest too, since the paths were checked above).
if [ "$zone" = "run" ]; then
  if [ -d "$run_dir" ]; then git add -- "$run_dir"; fi
else
  git add -A
fi
if ! git diff --cached --quiet; then
  git commit -q -m "chore(ai-flow): ${AI_FLOW_AGENT} output for #${AI_FLOW_ISSUE} (tour ${AI_FLOW_TOUR})"
fi

if [ "$(git rev-parse HEAD)" = "$AI_FLOW_BASE_SHA" ]; then
  info "Nothing to push."
  emit push_error ""
  exit 0
fi

if push_log="$(git push origin "HEAD:refs/heads/$AI_FLOW_BRANCH" 2>&1)"; then
  ok "Pushed $(git rev-parse --short HEAD) to $AI_FLOW_BRANCH."
  emit push_error ""
else
  printf '%s\n' "$push_log" >&2
  emit push_error "$(tail -n 3 <<< "$push_log" | tr '\n' ' ')"
fi
