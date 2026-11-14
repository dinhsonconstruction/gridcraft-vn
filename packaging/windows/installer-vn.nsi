; GridCraft VN Edition - Windows User-Scope Installer (NSIS 3 Unicode)
; Builds a standalone Setup .exe that installs without UAC elevation to
; %LOCALAPPDATA%\Programs\ArtCraft\GridCraft, registers Open With handlers,
; and embeds Vietnamese + English MUI screens.

Unicode True

!ifndef VERSION
  !define VERSION "0.19.1"
!endif
!ifndef VI_VERSION
  !define VI_VERSION "0.19.1.0"
!endif
!ifndef DIST_STAGE
  !define DIST_STAGE "../../target/windows-vn-stage/GridCraft-VN-${VERSION}-windows-x64-portable"
!endif
!ifndef ICON_PATH
  !define ICON_PATH "../../assets/app-icon/gridcraft.ico"
!endif
!ifndef OUT_EXE
  !define OUT_EXE "../../dist/windows-x64/GridCraft-VN-${VERSION}-Setup.exe"
!endif

!include "MUI2.nsh"
!include "FileFunc.nsh"

Name "GridCraft VN ${VERSION}"
OutFile "${OUT_EXE}"
InstallDir "$LOCALAPPDATA\Programs\ArtCraft\GridCraft"
InstallDirRegKey HKCU "Software\GridCraft" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
ShowUnInstDetails show

VIProductVersion "${VI_VERSION}"
VIFileVersion "${VI_VERSION}"
VIAddVersionKey "ProductName" "GridCraft VN Edition"
VIAddVersionKey "FileDescription" "GridCraft VN Edition Installer"
VIAddVersionKey "CompanyName" "ArtCraft / Dinh Son Construction VN Edition"
VIAddVersionKey "LegalCopyright" "Copyright (c) the GridCraft authors. MIT OR Apache-2.0."
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"

!define MUI_ICON "${ICON_PATH}"
!define MUI_UNICON "${ICON_PATH}"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\gridcraft.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Mở GridCraft ngay (Launch GridCraft)"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Put Vietnamese first so systems without an exact match still get Vietnamese by default,
; while supporting English UI too.
!insertmacro MUI_LANGUAGE "Vietnamese"
!insertmacro MUI_LANGUAGE "English"

Section "GridCraft (${VERSION})" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"

  File "${DIST_STAGE}/gridcraft.exe"
  File "${DIST_STAGE}/gridcraft-cli.exe"
  File "${DIST_STAGE}/NOTICE"
  File "${DIST_STAGE}/ATTRIBUTION.md"
  File /nonfatal "${DIST_STAGE}/README.md"
  File /nonfatal "${DIST_STAGE}/LICENSE"
  File /nonfatal "${DIST_STAGE}/LICENSE-MIT"
  File /nonfatal "${DIST_STAGE}/LICENSE-APACHE"
  File /nonfatal "${DIST_STAGE}/OFL-*.txt"

  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ; Start Menu shortcuts
  CreateDirectory "$SMPROGRAMS\GridCraft"
  CreateShortCut "$SMPROGRAMS\GridCraft\GridCraft.lnk" "$INSTDIR\gridcraft.exe" "" "$INSTDIR\gridcraft.exe" 0
  CreateShortCut "$SMPROGRAMS\GridCraft\Gỡ cài đặt GridCraft.lnk" "$INSTDIR\Uninstall.exe"

  ; App Paths & InstallDir
  WriteRegStr HKCU "Software\GridCraft" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\gridcraft.exe" "" "$INSTDIR\gridcraft.exe"

  ; ProgIds (HKCU - preserves user's current default spreadsheet app)
  WriteRegStr HKCU "Software\Classes\GridCraft.Workbook" "" "GridCraft Workbook"
  WriteRegStr HKCU "Software\Classes\GridCraft.Workbook\DefaultIcon" "" '"$INSTDIR\gridcraft.exe",0'
  WriteRegStr HKCU "Software\Classes\GridCraft.Workbook\shell\open\command" "" '"$INSTDIR\gridcraft.exe" "%1"'

  WriteRegStr HKCU "Software\Classes\GridCraft.TextData" "" "GridCraft Text Data"
  WriteRegStr HKCU "Software\Classes\GridCraft.TextData\DefaultIcon" "" '"$INSTDIR\gridcraft.exe",0'
  WriteRegStr HKCU "Software\Classes\GridCraft.TextData\shell\open\command" "" '"$INSTDIR\gridcraft.exe" "%1"'

  ; "Open with" menu registration
  WriteRegStr HKCU "Software\Classes\.xlsx\OpenWithProgids" "GridCraft.Workbook" ""
  WriteRegStr HKCU "Software\Classes\.xlsm\OpenWithProgids" "GridCraft.Workbook" ""
  WriteRegStr HKCU "Software\Classes\.xlsb\OpenWithProgids" "GridCraft.Workbook" ""
  WriteRegStr HKCU "Software\Classes\.ods\OpenWithProgids" "GridCraft.Workbook" ""
  WriteRegStr HKCU "Software\Classes\.csv\OpenWithProgids" "GridCraft.TextData" ""
  WriteRegStr HKCU "Software\Classes\.tsv\OpenWithProgids" "GridCraft.TextData" ""

  ; Windows Settings -> Default Apps capabilities
  WriteRegStr HKCU "Software\GridCraft\Capabilities" "ApplicationName" "GridCraft"
  WriteRegStr HKCU "Software\GridCraft\Capabilities" "ApplicationDescription" "Open-source spreadsheet for .xlsx, CSV and TSV files (VN Edition)"
  WriteRegStr HKCU "Software\GridCraft\Capabilities\FileAssociations" ".xlsx" "GridCraft.Workbook"
  WriteRegStr HKCU "Software\GridCraft\Capabilities\FileAssociations" ".xlsm" "GridCraft.Workbook"
  WriteRegStr HKCU "Software\GridCraft\Capabilities\FileAssociations" ".xlsb" "GridCraft.Workbook"
  WriteRegStr HKCU "Software\GridCraft\Capabilities\FileAssociations" ".ods" "GridCraft.Workbook"
  WriteRegStr HKCU "Software\GridCraft\Capabilities\FileAssociations" ".csv" "GridCraft.TextData"
  WriteRegStr HKCU "Software\GridCraft\Capabilities\FileAssociations" ".tsv" "GridCraft.TextData"
  WriteRegStr HKCU "Software\RegisteredApplications" "GridCraft" "Software\GridCraft\Capabilities"

  ; Add/Remove Programs (Settings -> Apps)
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "DisplayName" "GridCraft VN Edition"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "Publisher" "ArtCraft / Dinh Son Construction"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "DisplayIcon" "$INSTDIR\gridcraft.exe,0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "NoRepair" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft" "EstimatedSize" $0

  System::Call 'shell32.dll::SHChangeNotify(i 0x08000000, i 0x0000, p 0, p 0)'
SectionEnd

Section "Tạo biểu tượng trên Desktop (Desktop Shortcut)" SecDesktop
  CreateShortCut "$DESKTOP\GridCraft.lnk" "$INSTDIR\gridcraft.exe" "" "$INSTDIR\gridcraft.exe" 0
SectionEnd

LangString DESC_SecApp ${LANG_VIETNAMESE} "Cài đặt GridCraft VN Edition (${VERSION}), công cụ dòng lệnh gridcraft-cli và đăng ký mở file Excel/CSV."
LangString DESC_SecDesktop ${LANG_VIETNAMESE} "Tạo biểu tượng GridCraft trên màn hình Desktop."
LangString DESC_SecApp ${LANG_ENGLISH} "Install GridCraft VN Edition (${VERSION}), gridcraft-cli, and Open With file associations."
LangString DESC_SecDesktop ${LANG_ENGLISH} "Create a GridCraft shortcut on the Desktop."

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} $(DESC_SecApp)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} $(DESC_SecDesktop)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$DESKTOP\GridCraft.lnk"
  Delete "$SMPROGRAMS\GridCraft\GridCraft.lnk"
  Delete "$SMPROGRAMS\GridCraft\Gỡ cài đặt GridCraft.lnk"
  RMDir "$SMPROGRAMS\GridCraft"

  Delete "$INSTDIR\gridcraft.exe"
  Delete "$INSTDIR\gridcraft-cli.exe"
  Delete "$INSTDIR\NOTICE"
  Delete "$INSTDIR\ATTRIBUTION.md"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\LICENSE-MIT"
  Delete "$INSTDIR\LICENSE-APACHE"
  Delete "$INSTDIR\OFL-*.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GridCraft"
  DeleteRegValue HKCU "Software\RegisteredApplications" "GridCraft"
  DeleteRegKey HKCU "Software\GridCraft"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\gridcraft.exe"
  DeleteRegKey HKCU "Software\Classes\GridCraft.Workbook"
  DeleteRegKey HKCU "Software\Classes\GridCraft.TextData"
  DeleteRegValue HKCU "Software\Classes\.xlsx\OpenWithProgids" "GridCraft.Workbook"
  DeleteRegValue HKCU "Software\Classes\.xlsm\OpenWithProgids" "GridCraft.Workbook"
  DeleteRegValue HKCU "Software\Classes\.xlsb\OpenWithProgids" "GridCraft.Workbook"
  DeleteRegValue HKCU "Software\Classes\.ods\OpenWithProgids" "GridCraft.Workbook"
  DeleteRegValue HKCU "Software\Classes\.csv\OpenWithProgids" "GridCraft.TextData"
  DeleteRegValue HKCU "Software\Classes\.tsv\OpenWithProgids" "GridCraft.TextData"

  System::Call 'shell32.dll::SHChangeNotify(i 0x08000000, i 0x0000, p 0, p 0)'
SectionEnd
