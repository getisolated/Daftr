; Hooks de l'installeur NSIS de Tauri.
;
; Avant d'installer, les versions précédentes de l'application, publiées sous le nom
; « Everything Note », sont désinstallées en silence. Leurs données (%APPDATA%\everything-note)
; sont gardées : l'app les déplace vers %APPDATA%\daftr au premier lancement (`data_dir.rs`).
;
; - Version Electron (electron-builder, installée dans un autre dossier) : sa clé de registre
;   est l'UUID v5 de l'appId « com.everything.note ».
; - Version Tauri « Everything Note » : l'installeur NSIS de Tauri enregistre la désinstallation
;   sous le nom du produit ; Daftr en a une autre, et les deux cohabiteraient sinon.

!define ELECTRON_APP_GUID "34a98de4-b3ab-55df-9174-cd1780189c35"
!define ELECTRON_UNINSTALLER "Uninstall Everything Note.exe"
!define LEGACY_TAURI_UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Everything Note"

!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R0 HKCU "Software\${ELECTRON_APP_GUID}" "InstallLocation"
  ${If} $R0 != ""
  ${AndIf} ${FileExists} "$R0\${ELECTRON_UNINSTALLER}"
    DetailPrint "Removing the Electron version of Everything Note..."
    ; _?= : l'uninstaller tourne sur place (sans copie temporaire) et ExecWait l'attend vraiment
    ExecWait '"$R0\${ELECTRON_UNINSTALLER}" /S /KEEP_APP_DATA /currentuser _?=$R0' $R1
    Delete "$R0\${ELECTRON_UNINSTALLER}"
    RMDir "$R0"
    DeleteRegKey HKCU "Software\${ELECTRON_APP_GUID}"
  ${EndIf}

  ; Tauri écrit InstallLocation entre guillemets
  ReadRegStr $R0 HKCU "${LEGACY_TAURI_UNINSTKEY}" "InstallLocation"
  StrCpy $R1 $R0 1
  ${If} $R1 == '"'
    StrCpy $R0 $R0 "" 1
    StrCpy $R0 $R0 -1
  ${EndIf}
  ${If} $R0 != ""
  ${AndIf} ${FileExists} "$R0\uninstall.exe"
    DetailPrint "Removing the previous version (Everything Note)..."
    ; /S : silencieux, la case « supprimer les données » de l'uninstaller reste décochée
    ExecWait '"$R0\uninstall.exe" /S _?=$R0' $R1
    Delete "$R0\uninstall.exe"
    RMDir "$R0"
    DeleteRegKey HKCU "${LEGACY_TAURI_UNINSTKEY}"
  ${EndIf}
!macroend

; Synchronisation entre appareils (réseau local) : règles entrantes du pare-feu Windows pour
; l'écoute TCP (port par défaut 47820) et le mDNS (UDP 5353), profil « Privé » seulement et
; limitées à l'exécutable. Sans elles, la première écoute ouvre l'invite du pare-feu, qui
; propose le profil « Public ».
;
; L'installation est par utilisateur (sans droits d'administrateur) : netsh est lancé élevé
; (une invite UAC), une seule fois. Refusée, l'invite n'est pas reproposée aux mises à jour
; (marqueur dans HKCU) et Windows posera sa propre question à la première écoute. Un port
; changé dans les réglages n'est pas couvert par la règle TCP.

!define SYNC_RULE_TCP "Daftr - synchronisation LAN (TCP)"
!define SYNC_RULE_MDNS "Daftr - synchronisation LAN (mDNS)"
!define SYNC_PORT "47820"
!define SYNC_MARKER_KEY "Software\Daftr\Sync"
!define SYNC_MARKER_VALUE "FirewallRulesAsked"

!macro NSIS_HOOK_POSTINSTALL
  ; Code de sortie 0 : la règle existe déjà (installation précédente)
  nsExec::ExecToStack 'netsh advfirewall firewall show rule name="${SYNC_RULE_TCP}"'
  Pop $R0
  Pop $R1
  ReadRegStr $R2 HKCU "${SYNC_MARKER_KEY}" "${SYNC_MARKER_VALUE}"
  ${If} $R0 != 0
  ${AndIf} $R2 != "1"
    WriteRegStr HKCU "${SYNC_MARKER_KEY}" "${SYNC_MARKER_VALUE}" "1"
    DetailPrint "Adding the firewall rules of the LAN sync..."
    ExecShellWait "runas" "$SYSDIR\cmd.exe" '/c netsh advfirewall firewall add rule name="${SYNC_RULE_TCP}" dir=in action=allow protocol=TCP localport=${SYNC_PORT} profile=private program="$INSTDIR\${MAINBINARYNAME}.exe" & netsh advfirewall firewall add rule name="${SYNC_RULE_MDNS}" dir=in action=allow protocol=UDP localport=5353 profile=private program="$INSTDIR\${MAINBINARYNAME}.exe"' SW_HIDE
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Une mise à jour désinstalle l'ancienne version (/UPDATE) : les règles restent
  ${If} $UpdateMode <> 1
    nsExec::ExecToStack 'netsh advfirewall firewall show rule name="${SYNC_RULE_TCP}"'
    Pop $R0
    Pop $R1
    ${If} $R0 == 0
      ExecShellWait "runas" "$SYSDIR\cmd.exe" '/c netsh advfirewall firewall delete rule name="${SYNC_RULE_TCP}" & netsh advfirewall firewall delete rule name="${SYNC_RULE_MDNS}"' SW_HIDE
    ${EndIf}
    DeleteRegKey HKCU "${SYNC_MARKER_KEY}"
  ${EndIf}
!macroend
