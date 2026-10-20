import { bootstrapApplication } from '@angular/platform-browser';
import { appConfig } from './app/app.config';
import { AppComponent } from './app/app.component';
import { OverlayComponent } from './app/features/overlay/overlay.component';
import { currentWindowLabel } from './app/core/services/desktop-api';

// The main process loads the same bundle in the quick-note window, labelled `overlay`
const isOverlay = currentWindowLabel() === 'overlay';
if (isOverlay) {
  document.documentElement.classList.add('overlay-mode');
}

bootstrapApplication(isOverlay ? OverlayComponent : AppComponent, appConfig)
  .catch((err) => console.error(err));
