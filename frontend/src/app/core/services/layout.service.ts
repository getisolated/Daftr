import { Injectable, signal } from '@angular/core';

const STORAGE_KEY = 'daftr-wide-mode';
const HOME_VIEW_KEY = 'daftr-home-view';

export type HomeView = 'graph' | 'list';

@Injectable({ providedIn: 'root' })
export class LayoutService {
  private readonly _wideMode = signal(this.loadWideMode());

  private readonly _homeView = signal<HomeView>(this.loadHomeView());

  readonly wideMode = this._wideMode.asReadonly();
  /** What the home page shows: the graph of the notes (default) or their list. */
  readonly homeView = this._homeView.asReadonly();

  setHomeView(view: HomeView): void {
    this._homeView.set(view);

    try {
      localStorage.setItem(HOME_VIEW_KEY, view);
    } catch {
      // Storage unavailable: the choice only lasts for this session
    }
  }

  private loadHomeView(): HomeView {
    try {
      return localStorage.getItem(HOME_VIEW_KEY) === 'list' ? 'list' : 'graph';
    } catch {
      return 'graph';
    }
  }

  toggle(): void {
    const next = !this._wideMode();
    this._wideMode.set(next);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  }

  private loadWideMode(): boolean {
    try {
      return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'false');
    } catch {
      return false;
    }
  }
}
