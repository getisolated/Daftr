/** Update found by the main process (mirror of electron/update-manifest.ts). */
export interface UpdateInfo {
  currentVersion: string;
  version: string;
  /** The installer is downloaded and verified: installing is immediate. */
  downloaded: boolean;
}

/** Download progress of the installer. `percent` is -1 when the total size is unknown. */
export interface UpdateProgress {
  received: number;
  total: number;
  percent: number;
}
