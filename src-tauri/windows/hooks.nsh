!include "WinMessages.nsh"

!macro NSIS_HOOK_POSTINSTALL
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5

  ClearErrors
  ReadRegStr $0 HKCU "Environment" "Path"
  IfErrors 0 +2
    StrCpy $0 ""

  StrCmp $0 "" _gg_pi_empty

  StrCpy $1 ";$0;"
  StrCpy $2 ";$INSTDIR;"
  StrLen $3 $2
  StrLen $4 $1
  IntOp $4 $4 - $3
  IntCmp $4 0 _gg_pi_loop _gg_pi_append _gg_pi_loop

  _gg_pi_loop:
    StrCpy $5 $1 $3
    StrCmp $5 $2 _gg_pi_done
    StrCpy $1 $1 "" 1
    IntOp $4 $4 - 1
    IntCmp $4 0 _gg_pi_loop _gg_pi_append _gg_pi_loop

  _gg_pi_empty:
    StrCpy $0 "$INSTDIR"
    Goto _gg_pi_write

  _gg_pi_append:
    StrCpy $5 $0 "" -1
    StrCmp $5 ";" 0 +3
      StrCpy $0 "$0$INSTDIR"
      Goto _gg_pi_write
    StrCpy $0 "$0;$INSTDIR"

  _gg_pi_write:
    WriteRegExpandStr HKCU "Environment" "Path" "$0"
    SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000

  _gg_pi_done:
    Pop $5
    Pop $4
    Pop $3
    Pop $2
    Pop $1
    Pop $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6

  ClearErrors
  ReadRegStr $0 HKCU "Environment" "Path"
  IfErrors _gg_pu_done
  StrCmp $0 "" _gg_pu_done

  StrCpy $1 ";$0;"
  StrCpy $2 ";$INSTDIR;"
  StrLen $3 $2
  StrCpy $6 ""
  StrCpy $4 "0"

  _gg_pu_loop:
    StrCmp $1 "" _gg_pu_finish
    StrCpy $5 $1 $3
    StrCmp $5 $2 0 _gg_pu_keep_char
      IntOp $5 $3 - 1
      StrCpy $1 $1 "" $5
      StrCpy $4 "1"
      Goto _gg_pu_loop
    _gg_pu_keep_char:
      StrCpy $5 $1 1
      StrCpy $6 "$6$5"
      StrCpy $1 $1 "" 1
      Goto _gg_pu_loop

  _gg_pu_finish:
    StrCmp $4 "0" _gg_pu_done
    StrCpy $5 $6 1
    StrCmp $5 ";" 0 +2
      StrCpy $6 $6 "" 1
    StrCmp $6 "" _gg_pu_write
    StrCpy $5 $6 "" -1
    StrCmp $5 ";" 0 _gg_pu_write
      StrCpy $6 $6 -1

  _gg_pu_write:
    StrCmp $6 "" 0 +3
      DeleteRegValue HKCU "Environment" "Path"
      Goto _gg_pu_broadcast
    WriteRegExpandStr HKCU "Environment" "Path" "$6"
  _gg_pu_broadcast:
    SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000

  _gg_pu_done:
    Pop $6
    Pop $5
    Pop $4
    Pop $3
    Pop $2
    Pop $1
    Pop $0
!macroend
