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
  !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  ; Upgrades retain the game integration and all data for the new app to prepare.
  ${If} $UpdateMode <> 1
    StrCpy $R1 "delete"
    ${If} $DeleteAppDataCheckboxState = 1
      StrCpy $R1 "keep"
    ${EndIf}
    ${GetOptions} $CMDLINE "/KEEPDATA" $R2
    ${IfNot} ${Errors}
      StrCpy $R1 "keep"
    ${EndIf}
    DetailPrint "Removing Starframe game integration. App data choice: $R1."
    nsExec::ExecToStack '"$INSTDIR\${MAINBINARYNAME}.exe" --installer-uninstall "${BUNDLEID}" $R1'
    Pop $R0
    Pop $R2
    ${If} $R0 != 0
      DetailPrint "$R2"
      SetErrorLevel 2
      ${IfNot} ${Silent}
        MessageBox MB_OK|MB_ICONEXCLAMATION "Starframe could not finish cleanup. The app was kept so you can retry.$\r$\n$R2"
      ${EndIf}
      Abort "Game cleanup or app data removal did not finish."
    ${EndIf}
  ${EndIf}
  ; The maintenance command owns deletion. Never run NSIS's recursive data removal.
  StrCpy $DeleteAppDataCheckboxState 0
!macroend
