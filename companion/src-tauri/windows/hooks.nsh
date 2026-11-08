; SlideRelay Windows installer hooks (NSIS).
;
; Viewers connect to SlideRelay over the local network, so Windows Firewall
; must let them in. When installed for all users (which runs as
; administrator) we add an inbound rule for SlideRelay only. When installed
; for just the current user there are no admin rights; Windows then asks
; "Allow access?" the first time SlideRelay starts. See docs/INSTALL-WINDOWS.md.

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="SlideRelay"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="SlideRelay" description="Lets viewers on this network watch SlideRelay broadcasts." dir=in action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" enable=yes profile=any'
  Pop $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="SlideRelay"'
  Pop $0
!macroend
