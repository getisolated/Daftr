import { ApplicationConfig, ErrorHandler, Injectable } from '@angular/core';

@Injectable()
class GlobalErrorHandler implements ErrorHandler {
  handleError(error: unknown): void {
    console.error('[Daftr] Unhandled error:', error);
  }
}

export const appConfig: ApplicationConfig = {
  providers: [
    { provide: ErrorHandler, useClass: GlobalErrorHandler },
  ]
};
