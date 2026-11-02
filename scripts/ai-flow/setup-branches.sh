#!/usr/bin/env bash
# Creates the integration branch from main if it does not exist yet.
#
# Idempotent: an existing integration branch is left untouched (never reset, never
# force-pushed); the script only reports how far it is from main.
#
# Usage: bash scripts/ai-flow/setup-branches.sh

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
load_flow_env
require_gh

step "Branche $AI_FLOW_INTEGRATION_BRANCH sur $REPO"

main_sha="$(ghq api "repos/$REPO/git/ref/heads/$AI_FLOW_MAIN_BRANCH" --jq '.object.sha')" \
  || die "Branche $AI_FLOW_MAIN_BRANCH introuvable."

if integration_sha="$(ghq api "repos/$REPO/git/ref/heads/$AI_FLOW_INTEGRATION_BRANCH" --jq '.object.sha' 2> /dev/null)"; then
  compare="$(ghq api "repos/$REPO/compare/$AI_FLOW_MAIN_BRANCH...$AI_FLOW_INTEGRATION_BRANCH" \
    --jq '"\(.ahead_by) commit(s) d’avance, \(.behind_by) de retard"' 2> /dev/null || echo "comparaison indisponible")"
  ok "$AI_FLOW_INTEGRATION_BRANCH existe déjà (${integration_sha:0:7}) : $compare sur $AI_FLOW_MAIN_BRANCH."
else
  ghq api -X POST "repos/$REPO/git/refs" \
    -f "ref=refs/heads/$AI_FLOW_INTEGRATION_BRANCH" -f "sha=$main_sha" > /dev/null \
    || die "Impossible de créer $AI_FLOW_INTEGRATION_BRANCH."
  ok "$AI_FLOW_INTEGRATION_BRANCH créée depuis $AI_FLOW_MAIN_BRANCH (${main_sha:0:7})."
fi
