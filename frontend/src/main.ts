import { bootstrapApplication } from '@angular/platform-browser';
import { appConfig } from './app/app.config';
import { AppComponent } from './app/app.component';
import { OverlayComponent } from './app/features/overlay/overlay.component';
import { UnlockComponent } from './app/features/profiles/unlock.component';
import { currentWindowLabel } from './app/core/services/desktop-api';
import { setSessionProfile } from './app/core/services/profile-session';
import { fetchProfilesState } from './app/core/services/profiles-bridge.service';
import { ProfilesState } from './app/core/models/profile.model';
import { migrateStorageKeys } from './app/core/utils/storage-migration';

// Keys saved before the app became Daftr are renamed before any service reads them
try {
  migrateStorageKeys(localStorage);
} catch {
  // Storage unavailable: nothing to migrate
}

// The main process loads the same bundle in the quick-note window, labelled `overlay`
const isOverlay = currentWindowLabel() === 'overlay';
if (isOverlay) {
  document.documentElement.classList.add('overlay-mode');
}

// The native menu of the webview never shows; the app draws its own, and their handlers still run (capture does not stop the event)
document.addEventListener('contextmenu', (event) => event.preventDefault(), { capture: true });

/** The active profile, read before any service: its session keys depend on it. `null` when it cannot be read. */
async function readProfiles(): Promise<ProfilesState | null> {
  try {
    return await fetchProfilesState();
  } catch (error) {
    console.error('[Daftr] Profiles not read:', error);
    return null;
  }
}

async function start(): Promise<void> {
  const profiles = await readProfiles();
  setSessionProfile(profiles?.activeId ?? null);

  // A protected profile shows its lock screen: nothing of its data is read before its password
  if (isOverlay) {
    await bootstrapApplication(OverlayComponent, appConfig);
  } else if (profiles?.locked) {
    await bootstrapApplication(UnlockComponent, appConfig);
  } else {
    await bootstrapApplication(AppComponent, appConfig);
  }
}

start().catch((err) => console.error(err));
