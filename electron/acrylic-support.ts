import { execFile } from 'child_process';
import * as os from 'os';

// backgroundMaterial (system blur of what is behind the window) needs Windows 11 22H2
const ACRYLIC_MIN_WINDOWS_BUILD = 22621;
const PROBE_TIMEOUT_MS = 5000;

// Windows turns acrylic into a flat grey when Battery Saver is on or when "Transparency effects" is off,
// and neither state is exposed by Electron: GetSystemPowerStatus (SystemStatusFlag) + the Personalize key tell.
const PROBE_SCRIPT = `
Add-Type 'using System.Runtime.InteropServices; public class PowerProbe { [StructLayout(LayoutKind.Sequential)] public struct Status { public byte Line; public byte Flag; public byte Life; public byte Saver; public int A; public int B; } [DllImport("kernel32.dll")] public static extern bool GetSystemPowerStatus(out Status s); }'
$status = New-Object PowerProbe+Status
[void][PowerProbe]::GetSystemPowerStatus([ref]$status)
$transparency = (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' -ErrorAction SilentlyContinue).EnableTransparency
"$($status.Saver),$transparency"
`;

let acrylicActive: boolean | null = null;

function isWindows11With22H2(): boolean {
  return process.platform === 'win32' && Number(os.release().split('.')[2]) >= ACRYLIC_MIN_WINDOWS_BUILD;
}

function probeAcrylic(): Promise<boolean> {
  if (!isWindows11With22H2()) {
    return Promise.resolve(false);
  }
  return new Promise(resolve => {
    execFile(
      'powershell.exe',
      ['-NoProfile', '-NonInteractive', '-Command', PROBE_SCRIPT],
      { timeout: PROBE_TIMEOUT_MS, windowsHide: true },
      (error, stdout) => {
        if (error) {
          // Unknown state: assume the effect works, the reference behaviour of a Windows 11 machine
          resolve(true);
          return;
        }
        const [saver, transparency] = stdout.trim().split(',');
        resolve(saver !== '1' && transparency !== '0');
      },
    );
  });
}

/** Whether the OS acrylic is currently rendered for real. Cached: the first call pays the probe, later ones are instant. */
export async function isAcrylicActive(): Promise<boolean> {
  if (acrylicActive === null) {
    acrylicActive = await probeAcrylic();
  }
  return acrylicActive;
}

/** Re-probes in the background so the next call sees a Battery Saver / transparency change. */
export function refreshAcrylicState(): void {
  void probeAcrylic().then(active => { acrylicActive = active; });
}
