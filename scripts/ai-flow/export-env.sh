#!/usr/bin/env bash
# Exports the values of .ai-flow/flow.env to the next steps of a GitHub Actions job,
# through $GITHUB_ENV. Used at the start of every ai-* workflow job.
#
# Usage (in a workflow step): bash scripts/ai-flow/export-env.sh

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
: "${GITHUB_ENV:?GITHUB_ENV is not set: this script runs inside GitHub Actions only.}"

set -a
# shellcheck disable=SC1090
source <(tr -d '\r' < "$root/.ai-flow/flow.env")
set +a

while IFS= read -r name; do
  printf '%s=%s\n' "$name" "${!name}" >> "$GITHUB_ENV"
done < <(compgen -v | grep '^AI_FLOW_')
