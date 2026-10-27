import { Injectable, signal } from '@angular/core';
import { ZenSidebarReveal } from '../utils/zen-sidebar.util';

const STORAGE_KEY = 'daftr-wide-mode';
const HOME_VIEW_KEY = 'daftr-home-view';

export type HomeView = 'graph' | 'list';

@Injectable({ providedIn: 'root' })
export class LayoutService {
  private readonly _wideMode = signal(this.loadWideMode());

  private readonly _homeView = signal<HomeView>(this.loadHomeView());

  /**
   * Not persisted: it hides the tabs and the window buttons, so a launch always starts from the usual layout
   * rather than a bare page whose way back may be forgotten.
   */
  private readonly _zenMode = signal(false);
  private readonly _zenSidebar = signal<ZenSidebarReveal>(null);

  readonly wideMode = this._wideMode.asReadonly();
  /** What the home page shows: the graph of the notes (default) or their list. */
  readonly homeView = this._homeView.asReadonly();
  /** Zen mode: no tabs, no outline, and the sidebar floats tucked away on the left edge. */
  readonly zenMode = this._zenMode.asReadonly();
  /** Whether the floating sidebar of the zen mode is slid in, and by what. */
  readonly zenSidebar = this._zenSidebar.asReadonly();

  toggleZenMode(): void {
    this._zenMode.update(zen => !zen);
    this._zenSidebar.set(null);
  }

  setZenSidebar(reveal: ZenSidebarReveal): void {
    this._zenSidebar.set(reveal);
  }

  /** The sidebar shortcut in zen mode: slides the floating sidebar in, or away. */
  toggleZenSidebar(): void {
    this._zenSidebar.update(reveal => (reveal === null ? 'keyboard' : null));
  }

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
