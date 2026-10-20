import { bootstrapApplication } from '@angular/platform-browser';
import { appConfig } from './app/app.config';
import { AppComponent } from './app/app.component';
import { OverlayComponent } from './app/features/overlay/overlay.component';
import { currentWindowLabel } from './app/core/services/desktop-api';
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

bootstrapApplication(isOverlay ? OverlayComponent : AppComponent, appConfig)
  .catch((err) => console.error(err));
