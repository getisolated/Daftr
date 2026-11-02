#!/usr/bin/env bash
# Creates or updates the labels of the AI flow from .ai-flow/labels.txt.
#
# Idempotent: an existing label gets the color and description of the file; nothing is
# ever deleted. Labels starting with "ai:" that are not in the file are only reported.
#
# Usage (Git Bash on Windows, or any bash): bash scripts/ai-flow/setup-labels.sh

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
load_flow_env
require_gh

step "Labels du flux IA sur $REPO"

expected=()
while IFS=$'\t' read -r name color role desc; do
  expected+=("$name")
  gh label create "$name" --repo "$REPO" --color "$color" --description "$desc" --force > /dev/null \
    || die "Impossible de créer ou mettre à jour le label « $name »."
  ok "$name ($role)"
done < <(read_labels)

# Labels "ai:" présents sur le repo mais inconnus du fichier : signalés, jamais supprimés.
while IFS= read -r existing; do
  [ -n "$existing" ] || continue
  known=false
  for name in "${expected[@]}"; do
    if [ "$name" = "$existing" ]; then known=true; break; fi
  done
  $known || warn "Label « $existing » présent sur le repo mais absent de .ai-flow/labels.txt (laissé tel quel)."
done < <(ghq label list --repo "$REPO" --limit 500 --json name --jq '.[].name | select(startswith("ai:"))')

ok "${#expected[@]} labels à jour."
