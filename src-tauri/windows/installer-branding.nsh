!include Util.nsh
!pragma warning error 6000
!define /ifndef EM_SETCHARFORMAT 0x444

SetFont "Segoe UI" 10
Var StarframeHighContrast
Var StarframeHeadingFace
Var StarframeHeadingFont
Var StarframePageOverride
!define MUI_BGCOLOR "0F172A"
!define MUI_TEXTCOLOR "F8FAFC"
!define MUI_WELCOMEFINISHPAGE_BITMAP_STRETCH AspectFitHeight
!define MUI_CUSTOMFUNCTION_GUIINIT StarframeGuiInit
!define MUI_CUSTOMFUNCTION_UNGUIINIT un.StarframeGuiInit

!macro StarframeColors CONTROL FOREGROUND
  ${If} $StarframeHighContrast = 1
    SetCtlColors ${CONTROL} SYSCLR:8 SYSCLR:5
  ${Else}
    SetCtlColors ${CONTROL} ${FOREGROUND} 0F172A
  ${EndIf}
!macroend

!macro StarframePlaceControl CONTROL X Y WIDTH HEIGHT
  System::Store "S"
  StrCpy $9 ${CONTROL}
  System::Call '*(i ${X}, i ${Y}, i ${WIDTH}, i ${HEIGHT}) p.r0'
  System::Call 'user32::MapDialogRect(p $HWNDPARENT, p r0)'
  System::Call '*$0(i.r1, i.r2, i.r3, i.r4)'
  System::Free $0
  System::Call 'user32::SetWindowPos(p r9, p 0, i r1, i r2, i r3, i r4, i 0x14)'
  System::Store "L"
!macroend

!macro StarframeControlTheme CONTROL
  ${If} $StarframeHighContrast = 1
    System::Call 'uxtheme::SetWindowTheme(p ${CONTROL}, p 0, p 0)'
  ${Else}
    System::Call 'uxtheme::SetWindowTheme(p ${CONTROL}, w "DarkMode_Explorer", p 0)'
  ${EndIf}
!macroend

!macro StarframeBrandingFunctions PREFIX
Function ${PREFIX}StarframeInitializeBranding
  Push $0
  Push $1
  Push $2
  StrCpy $StarframeHighContrast 0
  ${If} ${IsHighContrastModeActive}
    StrCpy $StarframeHighContrast 1
  ${EndIf}
  StrCpy $StarframeHeadingFace "Segoe UI"
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts" "Bahnschrift (TrueType)"
  ${If} $0 != ""
    StrCpy $StarframeHeadingFace "Bahnschrift"
  ${EndIf}
  Pop $2
  Pop $1
  Pop $0
FunctionEnd

Function ${PREFIX}StarframeGuiInit
  CreateFont $StarframeHeadingFont $StarframeHeadingFace 12 600
  Call ${PREFIX}StarframePageShow
FunctionEnd

Function ${PREFIX}StarframePageShow
  Push $0
  Push $1
  Push $2
  Push $3
  Call ${PREFIX}StarframeInitializeBranding
  IntOp $0 $StarframeHighContrast ^ 1
  System::Call 'dwmapi::DwmSetWindowAttribute(p $HWNDPARENT, i 20, *i r0, i 4)'
  ${For} $0 1 3
    GetDlgItem $1 $HWNDPARENT $0
    !insertmacro StarframeControlTheme $1
  ${Next}
  !insertmacro StarframeColors $HWNDPARENT F8FAFC
  !insertmacro StarframeColors $mui.Header.Background F8FAFC
  !insertmacro StarframeColors $mui.Header.Image F8FAFC
  !insertmacro StarframeColors $mui.Header.Text F97316
  !insertmacro StarframeColors $mui.Header.SubText F8FAFC
  SendMessage $mui.Header.Text ${WM_SETFONT} $StarframeHeadingFont 1
  !insertmacro StarframeColors $mui.Branding.Background F8FAFC
  !insertmacro StarframeColors $mui.Branding.Text F8FAFC
  ${If} $StarframePageOverride != ""
    StrCpy $0 $StarframePageOverride
    StrCpy $StarframePageOverride ""
  ${Else}
    FindWindow $0 "#32770" "" $HWNDPARENT
  ${EndIf}
  !insertmacro StarframeColors $0 F8FAFC
  System::Call 'user32::GetWindow(p r0, i 5) p .r1'
  ${DoWhile} $1 <> 0
    System::Call 'user32::GetClassName(p r1, t .r2, i ${NSIS_MAX_STRLEN})'
    ${If} $2 == "Static"
      !insertmacro StarframeColors $1 F8FAFC
    ${ElseIf} $2 == "Button"
      System::Call 'user32::GetWindowLong(p r1, i -16) i .r3'
      IntOp $3 $3 & 0xF
      ${If} $3 < 2
        !insertmacro StarframeControlTheme $1
      ${ElseIf} $3 >= 2
      ${AndIf} $3 <= 9
        ${If} $StarframeHighContrast = 0
          System::Call 'uxtheme::SetWindowTheme(p r1, w "", w "")'
        ${EndIf}
        !insertmacro StarframeColors $1 F8FAFC
      ${EndIf}
    ${ElseIf} $2 == "Edit"
      !insertmacro StarframeControlTheme $1
      ${If} $StarframeHighContrast = 1
        SetCtlColors $1 SYSCLR:8 SYSCLR:5
      ${Else}
        SetCtlColors $1 F8FAFC 1E293B
      ${EndIf}
    ${ElseIf} $2 == "RichEdit20W"
      !insertmacro StarframeControlTheme $1
      ${If} $StarframeHighContrast = 1
        SendMessage $1 ${EM_SETBKGNDCOLOR} 1 0
        System::Call 'user32::GetSysColor(i 8) i.r3'
      ${Else}
        SendMessage $1 ${EM_SETBKGNDCOLOR} 0 0x3B291E
        StrCpy $3 0xFCFAF8
      ${EndIf}
      ; Keep URLs as readable, selectable license text; RichEdit's automatic links force blue.
      SendMessage $1 ${EM_AUTOURLDETECT} 0 0
      System::Call '*(i 92, i 0x40000020, i 0, i 0, i 0, i r3, &i2 0, &w32 "", &i2 0) p.r3'
      SendMessage $1 ${EM_SETCHARFORMAT} 4 $3
      System::Free $3
    ${ElseIf} $2 == "SysListView32"
      !insertmacro StarframeControlTheme $1
      ${If} $StarframeHighContrast = 0
        SendMessage $1 0x1001 0 0x3B291E ; LVM_SETBKCOLOR
        SendMessage $1 0x1024 0 0xFCFAF8 ; LVM_SETTEXTCOLOR
        SendMessage $1 0x1026 0 0x3B291E ; LVM_SETTEXTBKCOLOR
      ${EndIf}
    ${ElseIf} $2 == "msctls_progress32"
      ${If} $StarframeHighContrast = 0
        System::Call 'uxtheme::SetWindowTheme(p r1, w "", w "")'
        SendMessage $1 ${PBM_SETBARCOLOR} 0 0x1673F9
        SendMessage $1 ${PBM_SETBKCOLOR} 0 0x3B291E
      ${EndIf}
    ${EndIf}
    System::Call 'user32::GetWindow(p r1, i 2) p .r1'
  ${Loop}
  Pop $3
  Pop $2
  Pop $1
  Pop $0
FunctionEnd
!macroend

; AspectFitHeight can make the artwork wider than MUI's fixed bitmap slot.
!macro StarframeSidebarLayout PAGE IMAGE
  System::Store "S"
  System::Call 'user32::GetWindowRect(p ${IMAGE}, @r0)'
  System::Call 'user32::MapWindowPoints(p 0, p ${PAGE}, p r0, i 2)'
  System::Call '*$0(i, i, i.r1, i)'
  System::Call '*(i 0, i 0, i 11, i 0) p.r0'
  System::Call 'user32::MapDialogRect(p $HWNDPARENT, p r0)'
  System::Call '*$0(i, i, i.r2, i)'
  System::Free $0
  IntOp $1 $1 + $2
  System::Call 'user32::GetWindow(p ${PAGE}, i 5) p.r2'
  ${DoWhile} $2 <> 0
    System::Call 'user32::GetWindowRect(p r2, @r0)'
    System::Call 'user32::MapWindowPoints(p 0, p ${PAGE}, p r0, i 2)'
    System::Call '*$0(i.r3, i.r4, i.r5, i.r6)'
    ${If} $3 > 0
      IntOp $5 $5 - $1
      IntOp $6 $6 - $4
      System::Call 'user32::SetWindowPos(p r2, p 0, i r1, i r4, i r5, i r6, i 0x14)'
    ${EndIf}
    System::Call 'user32::GetWindow(p r2, i 2) p.r2'
  ${Loop}
  System::Store "L"
!macroend

!macro StarframeInstallerPageFunctions
Function StarframeWelcomeShow
  StrCpy $StarframePageOverride $mui.WelcomePage
  Call StarframePageShow
  !insertmacro StarframeSidebarLayout $mui.WelcomePage $mui.WelcomePage.Image
  !insertmacro StarframeColors $mui.WelcomePage.Title F97316
  SendMessage $mui.WelcomePage.Title ${WM_SETFONT} $StarframeHeadingFont 1
FunctionEnd

Function StarframeFinishShow
  StrCpy $StarframePageOverride $mui.FinishPage
  Call StarframePageShow
  !insertmacro StarframeSidebarLayout $mui.FinishPage $mui.FinishPage.Image
  !insertmacro StarframeColors $mui.FinishPage.Title F97316
  SendMessage $mui.FinishPage.Title ${WM_SETFONT} $StarframeHeadingFont 1
FunctionEnd
!macroend
