#!/usr/bin/env bash
# Creates or updates the two rulesets of the AI flow.
#
#   "ai-flow: main"         pull request required, required CI green (ci.yml on Windows and
#                           macOS), no direct push, no force-push, no deletion.
#   "ai-flow: integration"  only the flow's GitHub App (through a pull request) and the repo
#                           admins may update it; required fast check of the flow; no
#                           force-push, no deletion.
#
# Bypass: repository admins, always. release.yml pushes its "chore(release)" commit straight
# to main with RELEASE_TOKEN: that token must belong to an admin of the repository, which
# is the case of a personal access token of the owner.
#
# Idempotent: a ruleset with the same name is replaced (PUT), otherwise created (POST).
# Other rulesets and classic branch protections are left untouched, only reported.
#
# Usage: bash scripts/ai-flow/setup-rulesets.sh

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
load_flow_env
require_gh

# Repository role "admin" (built-in role id 5): the owner of a personal repository.
ADMIN_BYPASS='{ "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" }'
# The flow's GitHub App may bypass only when merging a pull request, never by pushing.
APP_BYPASS="{ \"actor_id\": $AI_FLOW_APP_ID, \"actor_type\": \"Integration\", \"bypass_mode\": \"pull_request\" }"

# "a,b" -> [{"context":"a"},{"context":"b"}]
checks_json() {
  local list="$1" out="" item
  IFS=',' read -r -a items <<< "$list"
  for item in "${items[@]}"; do
    item="$(trim "$item")"
    [ -n "$item" ] || continue
    out+="${out:+,}{\"context\":$(json_str "$item")}"
  done
  printf '[%s]' "$out"
}

pull_request_rule='{
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false
      }
    }'

status_checks_rule() {
  printf '{
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": false,
        "required_status_checks": %s
      }
    }' "$(checks_json "$1")"
}

main_payload="{
  \"name\": \"ai-flow: main\",
  \"target\": \"branch\",
  \"enforcement\": \"active\",
  \"conditions\": { \"ref_name\": { \"include\": [\"refs/heads/$AI_FLOW_MAIN_BRANCH\"], \"exclude\": [] } },
  \"bypass_actors\": [ $ADMIN_BYPASS ],
  \"rules\": [
    { \"type\": \"deletion\" },
    { \"type\": \"non_fast_forward\" },
    $pull_request_rule,
    $(status_checks_rule "$AI_FLOW_MAIN_REQUIRED_CHECKS")
  ]
}"

integration_payload="{
  \"name\": \"ai-flow: integration\",
  \"target\": \"branch\",
  \"enforcement\": \"active\",
  \"conditions\": { \"ref_name\": { \"include\": [\"refs/heads/$AI_FLOW_INTEGRATION_BRANCH\"], \"exclude\": [] } },
  \"bypass_actors\": [ $ADMIN_BYPASS, $APP_BYPASS ],
  \"rules\": [
    { \"type\": \"deletion\" },
    { \"type\": \"non_fast_forward\" },
    { \"type\": \"update\", \"parameters\": { \"update_allows_fetch_and_merge\": false } },
    $pull_request_rule,
    $(status_checks_rule "$AI_FLOW_INTEGRATION_REQUIRED_CHECKS")
  ]
}"

upsert_ruleset() {
  local name="$1" payload="$2" id
  id="$(ghq api "repos/$REPO/rulesets?includes_parents=false" --paginate \
    --jq ".[] | select(.name == $(json_str "$name")) | .id" | head -n 1)"
  if [ -n "$id" ]; then
    printf '%s' "$payload" | ghq api -X PUT "repos/$REPO/rulesets/$id" --input - > /dev/null \
      || die "Mise à jour du ruleset « $name » refusée."
    ok "$name : mis à jour (id $id)."
  else
    id="$(printf '%s' "$payload" | ghq api -X POST "repos/$REPO/rulesets" --input - --jq '.id')" \
      || die "Création du ruleset « $name » refusée."
    ok "$name : créé (id $id)."
  fi
}

step "Rulesets du flux IA sur $REPO"

# The integration ruleset restricts updates: the branch must exist before it applies.
ghq api "repos/$REPO/git/ref/heads/$AI_FLOW_INTEGRATION_BRANCH" > /dev/null 2>&1 \
  || die "La branche $AI_FLOW_INTEGRATION_BRANCH n'existe pas : lancez d'abord setup-branches.sh."

# The App id must match the record of the App, or the bypass would target nobody.
check_app_id

upsert_ruleset "ai-flow: main" "$main_payload"
upsert_ruleset "ai-flow: integration" "$integration_payload"

# Report what else protects these branches: rules stack, the strictest one wins.
for branch in "$AI_FLOW_MAIN_BRANCH" "$AI_FLOW_INTEGRATION_BRANCH"; do
  if ghq api "repos/$REPO/branches/$branch/protection" > /dev/null 2>&1; then
    warn "$branch a aussi une protection de branche classique (Settings > Branches) : elle s'ajoute aux rulesets."
  fi
done
while IFS= read -r other; do
  if [ -n "$other" ]; then warn "Autre ruleset présent : « $other » (laissé tel quel)."; fi
done < <(ghq api "repos/$REPO/rulesets?includes_parents=false" --paginate \
  --jq '.[] | select(.name | startswith("ai-flow: ") | not) | .name')

info "Rappel : RELEASE_TOKEN (release.yml) doit appartenir à un admin du repo pour pousser sur $AI_FLOW_MAIN_BRANCH."
