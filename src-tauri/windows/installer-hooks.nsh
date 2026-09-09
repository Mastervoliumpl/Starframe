; Tauri's default macro kills running apps in silent/passive mode. File recovery
; needs normal Starframe shutdown, so replace that macro through the hook include.
!macroundef CheckIfAppIsRunning
!macro CheckIfAppIsRunning executableName productName
  nsis_tauri_utils::FindProcessCurrentUser "${executableName}"
  Pop $R0
  ${If} $R0 <> 1
    SetErrorLevel 2
    ${IfNot} ${Silent}
    ${AndIf} $PassiveMode <> 1
      MessageBox MB_OK|MB_ICONINFORMATION "Close ${productName} normally, then run setup again. Let any active file operation finish before closing."
    ${EndIf}
    Abort "Close ${productName} normally before continuing."
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Upgrades invoke this uninstaller too; their handoff must stay unattended.
  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
    MessageBox MB_OKCANCEL|MB_ICONINFORMATION "This removes the Starframe desktop app. It leaves Sanctuary and its installed game integration unchanged.$\r$\n$\r$\nTo remove game integration first, cancel and open Starframe Settings > Remove Starframe from game while the game is closed.$\r$\n$\r$\nYour library, collections and settings are kept unless you selected the option to delete app data. Local mod source folders are always kept." IDOK starframe_remove_app
    Abort
    starframe_remove_app:
  ${EndIf}
!macroend
