import { Signal, computed, effect, signal } from '@angular/core';
import { PairingTicket } from '../../core/models/sync.model';
import { formatCountdown, remainingMs } from '../../core/utils/sync.util';

/** Refresh of the countdown of the pairing code. */
export const COUNTDOWN_TICK_MS = 1000;

/** Countdown of a pairing code, shared by the screens that show one. */
export interface PairingCountdown {
  /** Time left before the code expires, in ms (0 without a code). */
  readonly remaining: Signal<number>;
  /** "4:59". */
  readonly label: Signal<string>;
}

/**
 * Counts down to the end of `ticket`, ticking every second while `ticking()` and a code exists. To create in an
 * injection context (field initializer of a component): the timer stops with it.
 */
export function pairingCountdown(ticket: Signal<PairingTicket | null>, ticking: () => boolean): PairingCountdown {
  const now = signal(Date.now());

  effect(onCleanup => {
    if (!ticking() || !ticket()) {
      return;
    }

    // A new code counts from now, not from the last tick
    now.set(Date.now());
    const timer = setInterval(() => now.set(Date.now()), COUNTDOWN_TICK_MS);
    onCleanup(() => clearInterval(timer));
  });

  const remaining = computed(() => remainingMs(ticket()?.expiresAt ?? 0, now()));

  return { remaining, label: computed(() => formatCountdown(remaining())) };
}
