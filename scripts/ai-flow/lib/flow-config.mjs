// Reads the two configuration files of the AI flow: .ai-flow/flow.env (KEY=VALUE) and
// .ai-flow/labels.txt (name | color | role | description). Same parsing rules as
// scripts/ai-flow/common.sh, so the bash scripts and the router agree.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/** Parses the content of flow.env: comments and blank lines ignored, quotes stripped. */
export function parseFlowEnv(text) {
  const env = {};
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith('#')) continue;
    const eq = line.indexOf('=');
    if (eq <= 0) continue;
    const key = line.slice(0, eq).trim();
    let value = line.slice(eq + 1).trim();
    if (value.length >= 2 && (value[0] === '"' || value[0] === "'") && value.at(-1) === value[0]) {
      value = value.slice(1, -1);
    }
    env[key] = value;
  }
  return env;
}

/** Parses the content of labels.txt into [{ name, color, role, description }]. */
export function parseLabels(text) {
  const labels = [];
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith('#')) continue;
    // Like bash `IFS='|' read -r name color role desc`: the description keeps any further "|".
    const parts = line.split('|');
    const [name, color, role] = parts.slice(0, 3).map((part) => part.trim());
    if (!name || !role) continue;
    labels.push({ name, color, role, description: parts.slice(3).join('|').trim() });
  }
  return labels;
}

export function loadFlowEnv(root) {
  return parseFlowEnv(readFileSync(join(root, '.ai-flow', 'flow.env'), 'utf8'));
}

export function loadLabels(root) {
  return parseLabels(readFileSync(join(root, '.ai-flow', 'labels.txt'), 'utf8'));
}
