import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { ToastService } from '../src/app/core/services/toast.service';

describe('ToastService', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('removes a notification by itself, later when it has an action', () => {
    const toasts = new ToastService();
    toasts.show('linked');
    toasts.show('offer', { label: 'Choose', run: () => undefined });

    vi.advanceTimersByTime(6000);
    expect(toasts.toasts().map(toast => toast.message)).toEqual(['offer']);

    vi.advanceTimersByTime(9000);
    expect(toasts.toasts()).toEqual([]);
  });

  it('runs the action once, then removes the notification', () => {
    const toasts = new ToastService();
    const run = vi.fn();
    const id = toasts.show('offer', { label: 'Choose', run });

    toasts.act(id);
    toasts.act(id);

    expect(run).toHaveBeenCalledTimes(1);
    expect(toasts.toasts()).toEqual([]);
  });

  it('keeps the newest notifications only', () => {
    const toasts = new ToastService();
    ['a', 'b', 'c', 'd', 'e'].forEach(message => toasts.show(message));

    expect(toasts.toasts().map(toast => toast.message)).toEqual(['b', 'c', 'd', 'e']);
  });
});
