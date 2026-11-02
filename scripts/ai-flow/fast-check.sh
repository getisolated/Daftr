#!/usr/bin/env bash
# Fast check of the AI flow: what the tests and code agents must pass, what the workflow
# re-runs after the code agent, and what the `ai-ci-check` job of ai-ci.yml runs.
#
#   bash scripts/ai-flow/fast-check.sh [report.md]
#
# Steps (all run, even after a failure): Rust format, clippy, Rust tests, front tests,
# front lint, front build (Angular templates are only type-checked by the build). With
# AI_FLOW_CHECK_BINDINGS=1, also fails when `cargo test` regenerated TypeScript bindings
# that are not committed (CI only: in an agent job the workflow commits them).
#
# Cargo runs from src-tauri/: its .cargo/config.toml sends the ts-rs bindings to
# frontend/src/app/core/bindings (with --manifest-path from the root they would land in
# the crates). Requires the npm dependencies (npm ci) and, on Linux, the Tauri system
# libraries. The report (Markdown, French, for agents and the PM) goes to the file given
# as argument, or stdout. Exit status 1 if any step failed.

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
set +e
# Plain logs: the report is read by agents and in issue comments.
export CARGO_TERM_COLOR=never NO_COLOR=1 FORCE_COLOR=0

report="${1:-/dev/stdout}"
cd "$AI_FLOW_WORKDIR" || die "Dossier de travail introuvable : $AI_FLOW_WORKDIR"
logs="$(mktemp -d)"
names=()
results=()

run_step() { # name, command...
  local name="$1"
  shift
  local log="$logs/${#names[@]}.log"
  printf '\n::group::%s\n' "$name"
  local start=$SECONDS
  "$@" > "$log" 2>&1
  local status=$?
  cat "$log"
  printf '::endgroup::\n'
  names+=("$name")
  results+=("$status:$((SECONDS - start))")
  if [ "$status" -ne 0 ]; then printf '::error::%s a échoué\n' "$name"; fi
}

run_step "Format Rust (cargo fmt)" bash -c 'cd src-tauri && cargo fmt --all --check -- --color never'
run_step "Lint Rust (cargo clippy)" bash -c 'cd src-tauri && cargo clippy --workspace --all-targets -- -D warnings'
run_step "Tests Rust (cargo test)" bash -c 'cd src-tauri && cargo test --workspace'
run_step "Tests front (Vitest)" npm test
run_step "Lint front (ng lint)" npm run lint
run_step "Build front (ng build)" npm run build:frontend
if [ "${AI_FLOW_CHECK_BINDINGS:-0}" = "1" ]; then
  run_step "Bindings TypeScript à jour" git diff --exit-code --stat -- frontend/src/app/core/bindings
fi

failed=0
{
  echo "# Check rapide"
  echo
  echo "Commit \`$(git rev-parse --short HEAD 2> /dev/null || echo '?')\` · $(date -u '+%Y-%m-%d %H:%M UTC')"
  echo
  echo "| Vérification | Résultat | Durée |"
  echo "| --- | --- | --- |"
  for i in "${!names[@]}"; do
    status="${results[$i]%%:*}"
    duration="${results[$i]#*:}"
    if [ "$status" -eq 0 ]; then icon="✅"; else icon="❌"; failed=1; fi
    echo "| ${names[$i]} | $icon | ${duration} s |"
  done
  if [ "$failed" -eq 1 ]; then
    echo
    echo "## Échecs"
    for i in "${!names[@]}"; do
      [ "${results[$i]%%:*}" -eq 0 ] && continue
      echo
      echo "### ${names[$i]}"
      echo
      echo '```text'
      # The end of the log holds the errors; cargo and Angular are verbose before them.
      grep -v -E '^\s*(Compiling|Checking|Downloaded|Downloading|Fresh) ' "$logs/$i.log" | tail -n 80
      echo '```'
    done
  fi
} > "$report"

rm -rf "$logs"
exit "$failed"
