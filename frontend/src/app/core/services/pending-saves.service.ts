import { Injectable } from '@angular/core';

/** Saves still waiting for their debounce (editor): written right away before the window reloads on another profile. */
@Injectable({ providedIn: 'root' })
export class PendingSavesService {
  private readonly flushers = new Set<() => Promise<void>>();

  /** Registers a flush; returns the function that unregisters it. */
  register(flush: () => Promise<void>): () => void {
    this.flushers.add(flush);
    return () => this.flushers.delete(flush);
  }

  async flushAll(): Promise<void> {
    await Promise.all([...this.flushers].map(flush => flush()));
  }
}
