import type { Translator } from '../i18n/translate';
import type { Profile } from '../models/profile.model';
import type { ContextMenuItem } from '../../shared/context-menu/context-menu.component';
import { profileDisplayName } from './profile.util';

const ACTION_PREFIX = 'transfer:';
const ICON_TRANSFER = 'M16 3h5v5M21 3l-7 7M8 21H3v-5M3 21l7-7';
const ICON_LOCKED = 'M5 11h14v10H5zM8 11V7a4 4 0 0 1 8 0v4';

/**
 * Context-menu entry "Transfer to profile": one child per other profile (a protected one is written to without its
 * password). `null` when there is no other profile.
 */
export function transferMenu(t: Translator, targets: readonly Profile[]): ContextMenuItem | null {
  if (targets.length === 0) {
    return null;
  }

  const children: ContextMenuItem[] = targets.map(profile => ({
    id: `${ACTION_PREFIX}${profile.id}`,
    label: profileDisplayName(profile, t),
    ...(profile.protected ? { icon: ICON_LOCKED } : {}),
  }));

  return { id: 'transfer', label: t('profiles.transfer.menu'), icon: ICON_TRANSFER, children };
}

/** Target profile of a menu action, `null` for any other action. */
export function transferTarget(actionId: string): string | null {
  return actionId.startsWith(ACTION_PREFIX) ? actionId.slice(ACTION_PREFIX.length) || null : null;
}
