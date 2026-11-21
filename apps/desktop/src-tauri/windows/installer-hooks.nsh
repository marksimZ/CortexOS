!macro NSIS_HOOK_PREINSTALL
  ; Reject a folder at the executable path before NSIS writes any version or shortcut.
  IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe\*.*" 0 cortexos_install_path_ok
  IfSilent cortexos_install_path_abort
  StrCmp $LANGUAGE 2052 0 cortexos_install_path_english
  MessageBox MB_OK|MB_ICONSTOP "安装目录中有一个与 CortexOS 程序同名的文件夹。请将该文件夹移到其他位置后重试。"
  Goto cortexos_install_path_abort
  cortexos_install_path_english:
  MessageBox MB_OK|MB_ICONSTOP "A folder occupies the CortexOS executable path. Move the folder to another location and retry."
  cortexos_install_path_abort:
  SetErrorLevel 2
  Abort
  cortexos_install_path_ok:
!macroend
