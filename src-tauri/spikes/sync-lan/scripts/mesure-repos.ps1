# Mesure RAM et CPU au repos : reference, spike sans mDNS, spike avec mDNS (+ ip-check 0).
# Usage : copier target/release/reference.exe en sync-spike-reference.exe et
# target/release/sync-lan-spike.exe en sync-spike-demo.exe dans -Dir, puis
#   powershell -File mesure-repos.ps1 -Dir <dossier> -Seconds 60
param([string]$Dir, [int]$Seconds = 60)
Get-Process sync-spike-demo, sync-spike-reference -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false
$runs = @(
  @{ Name = 'reference';         Exe = "$Dir\sync-spike-reference.exe"; Args = '47831' },
  @{ Name = 'spike sans mDNS';   Exe = "$Dir\sync-spike-demo.exe"; Args = 'listen --port 47832 --no-mdns' },
  @{ Name = 'spike + mDNS';      Exe = "$Dir\sync-spike-demo.exe"; Args = 'listen --port 47833' },
  @{ Name = 'mDNS ip-check 0';   Exe = "$Dir\sync-spike-demo.exe"; Args = 'listen --port 47834 --ip-check 0' }
)
foreach ($r in $runs) {
  $r.Proc = Start-Process -FilePath $r.Exe -ArgumentList $r.Args -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput "$Dir\sync-spike-idle-$($r.Args.Split(' ')[-1]).log"
}
function Get-Snap($p) {
  $p.Refresh()
  [pscustomobject]@{ WS = $p.WorkingSet64; Priv = $p.PrivateMemorySize64; Threads = $p.Threads.Count; Cpu = $p.TotalProcessorTime.TotalMilliseconds }
}
Start-Sleep -Seconds $Seconds
$t1 = @{}; foreach ($r in $runs) { $t1[$r.Name] = Get-Snap $r.Proc }
Start-Sleep -Seconds $Seconds
foreach ($r in $runs) {
  $a = $t1[$r.Name]; $b = Get-Snap $r.Proc
  '{0,-16} WS={1,6:N0} Kio  Prive={2,6:N0} Kio  threads={3,2}  CPU(t1)={4,5:N0} ms  CPU sur {5}s={6,4:N0} ms  WS(t2)={7,6:N0} Kio' -f `
    $r.Name, ($a.WS/1KB), ($a.Priv/1KB), $a.Threads, $a.Cpu, $Seconds, ($b.Cpu - $a.Cpu), ($b.WS/1KB)
  Stop-Process -Id $r.Proc.Id -Force -Confirm:$false
}
