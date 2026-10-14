import { bootstrapApplication } from '@angular/platform-browser';
import { appConfig } from './app/app.config';
import { AppComponent } from './app/app.component';
import { OverlayComponent } from './app/features/overlay/overlay.component';

// The Electron main process loads the same bundle with `#overlay` for the quick-note window
const isOverlay = window.location.hash === '#overlay';
if (isOverlay) {
  document.documentElement.classList.add('overlay-mode');
}

bootstrapApplication(isOverlay ? OverlayComponent : AppComponent, appConfig)
  .catch((err) => console.error(err));
