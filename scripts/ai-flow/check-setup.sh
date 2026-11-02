#!/usr/bin/env bash
# Checks that the repository has everything the AI flow needs. Read-only: changes nothing.
# Reports every problem found (not only the first one) and exits non-zero if any.
#
# What it cannot check from your machine: that the App is installed on the repository and
# that the secrets hold valid values. The "AI flow · self-test" workflow does that, from
# the Actions tab (Run workflow).
#
# Usage: bash scripts/ai-flow/check-setup.sh

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
load_flow_env
require_gh

failures=0
fail() { printf '  \033[31m✗\033[0m %s\n' "$*" >&2; failures=$((failures + 1)); }

step "Vérification du flux IA sur $REPO"

# Secrets (names only: values are never readable)
secrets="$(ghq secret list --repo "$REPO" --json name --jq '.[].name')"
for secret in CLAUDE_CODE_OAUTH_TOKEN AI_FLOW_APP_ID AI_FLOW_APP_PRIVATE_KEY; do
  if grep -qx "$secret" <<< "$secrets"; then ok "secret $secret"; else fail "secret $secret absent"; fi
done

# PM logins
pm_logins="$(ghq variable get AI_FLOW_PM_LOGINS --repo "$REPO" 2> /dev/null || true)"
if [ -n "$pm_logins" ]; then
  ok "variable AI_FLOW_PM_LOGINS = $pm_logins"
  IFS=',' read -r -a logins <<< "$pm_logins"
  for login in "${logins[@]}"; do
    login="$(trim "$login")"
    [ -n "$login" ] || continue
    if ghq api "users/$login" > /dev/null 2>&1; then
      ok "login PM « $login » existe"
    else
      fail "login PM « $login » introuvable sur GitHub"
    fi
  done
else
  fail "variable AI_FLOW_PM_LOGINS absente ou vide"
fi

# GitHub App: its record must match flow.env (when visible from here)
if app_id="$(ghq api "apps/$AI_FLOW_APP_SLUG" --jq '.id' 2> /dev/null)"; then
  if [ "$app_id" = "$AI_FLOW_APP_ID" ]; then
    ok "App $AI_FLOW_APP_SLUG (id $app_id)"
  else
    fail "App $AI_FLOW_APP_SLUG : id $app_id, .ai-flow/flow.env indique $AI_FLOW_APP_ID"
  fi
else
  info "App $AI_FLOW_APP_SLUG privée : invisible d'ici, elle est vérifiée par l'auto-test (Actions)."
fi

# Labels
existing_labels="$(ghq label list --repo "$REPO" --limit 500 --json name --jq '.[].name')"
missing=0
while IFS=$'\t' read -r name _color _role _desc; do
  grep -qxF "$name" <<< "$existing_labels" || { fail "label $name absent"; missing=$((missing + 1)); }
done < <(read_labels)
if [ "$missing" -eq 0 ]; then ok "labels du flux présents ($(read_labels | wc -l | tr -d ' '))"; fi

# Branches
for branch in "$AI_FLOW_MAIN_BRANCH" "$AI_FLOW_INTEGRATION_BRANCH"; do
  if ghq api "repos/$REPO/git/ref/heads/$branch" > /dev/null 2>&1; then ok "branche $branch"; else fail "branche $branch absente"; fi
done

# Rulesets
rulesets="$(ghq api "repos/$REPO/rulesets?includes_parents=false" --paginate --jq '.[] | "\(.name)\t\(.enforcement)"')"
for name in "ai-flow: main" "ai-flow: integration"; do
  enforcement="$(awk -F'\t' -v n="$name" '$1 == n { print $2 }' <<< "$rulesets")"
  case "$enforcement" in
    active) ok "ruleset « $name » actif" ;;
    "") fail "ruleset « $name » absent" ;;
    *) fail "ruleset « $name » présent mais « $enforcement » (attendu : active)" ;;
  esac
done

echo
if [ "$failures" -eq 0 ]; then
  ok "Tout est prêt. Dernière étape : onglet Actions > « AI flow · self-test » > Run workflow."
else
  die "$failures problème(s) à corriger, puis relancez ce script."
fi
