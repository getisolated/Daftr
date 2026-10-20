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
