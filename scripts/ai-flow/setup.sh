#!/usr/bin/env bash
# One-shot setup of the AI flow on the repository, then a check of everything the flow
# needs. Safe to run again at any time: every step is idempotent.
#
#   1. labels         (setup-labels.sh)
#   2. integration    (setup-branches.sh)
#   3. rulesets       (setup-rulesets.sh)
#   4. check          (check-setup.sh: secrets, variable, App, labels, branch, rulesets)
#
# Usage, from the root of a clone (Git Bash on Windows):
#   bash scripts/ai-flow/setup.sh

set -euo pipefail

dir="$(dirname "${BASH_SOURCE[0]}")"

bash "$dir/setup-labels.sh"
bash "$dir/setup-branches.sh"
bash "$dir/setup-rulesets.sh"
bash "$dir/check-setup.sh"
