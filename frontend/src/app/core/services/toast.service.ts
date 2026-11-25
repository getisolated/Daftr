import { Injectable, signal } from '@angular/core';
import { Toast, ToastAction } from '../models/toast.model';

/** How long a notification stays; one with an action waits longer for the user to reach it. */
const TOAST_MS = 6000;
const TOAST_WITH_ACTION_MS = 15000;
/** Older notifications go first beyond this many. */
const MAX_TOASTS = 4;

/** In-app notifications (the app has no system notifications): shown by the toast stack of the app shell. */
@Injectable({ providedIn: 'root' })
export class ToastService {
  private readonly _toasts = signal<Toast[]>([]);
  private readonly timers = new Map<number, ReturnType<typeof setTimeout>>();
  private nextId = 1;

  readonly toasts = this._toasts.asReadonly();

  /** Shows a notification, removed by itself after a few seconds; returns its id. */
  show(message: string, action: ToastAction | null = null): number {
    const id = this.nextId++;
    const kept = this._toasts().slice(-(MAX_TOASTS - 1));

    this._toasts().filter(toast => !kept.includes(toast)).forEach(toast => this.clearTimer(toast.id));
    this._toasts.set([...kept, { id, message, action }]);
    this.timers.set(id, setTimeout(() => this.dismiss(id), action ? TOAST_WITH_ACTION_MS : TOAST_MS));

    return id;
  }

  /** Removes a notification. */
  dismiss(id: number): void {
    this.clearTimer(id);
    this._toasts.update(toasts => toasts.filter(toast => toast.id !== id));
  }

  /** Runs the action of a notification, then removes it. */
  act(id: number): void {
    const toast = this._toasts().find(candidate => candidate.id === id);

    this.dismiss(id);
    toast?.action?.run();
  }

  private clearTimer(id: number): void {
    const timer = this.timers.get(id);

    if (timer) {
      clearTimeout(timer);
      this.timers.delete(id);
    }
  }
}
