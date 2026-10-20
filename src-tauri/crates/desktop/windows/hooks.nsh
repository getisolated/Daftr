; Hooks de l'installeur NSIS de Tauri.
;
; Avant d'installer, la version Electron (electron-builder, installée dans un autre dossier)
; est désinstallée en silence, en gardant les données de %APPDATA%\everything-note que la
; version Tauri reprend. Sa clé est l'UUID v5 de l'appId « com.everything.note ».

!define ELECTRON_APP_GUID "34a98de4-b3ab-55df-9174-cd1780189c35"
!define ELECTRON_UNINSTALLER "Uninstall Everything Note.exe"

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
!macroend
