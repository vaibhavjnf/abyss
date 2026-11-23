---
name: forensics
description: Host triage, timelines, memory, logs, artifacts.
tags: [dfir, timeline, memory, logs]
---
# Forensics

## Triage (live)
- Order: memory → network → disk → logs. Volatile dies first.
- `date -u`, `uptime`, `who`, `last -a`, `ps auxf`, `ss -tulpn`, `lsof -i`.
- `find / -mtime -2 -type f 2>/dev/null | head -100` (recent changes).
- Hash known-good binaries: `sha256sum /usr/bin/*` vs the package DB.

## Linux artifacts
- Auth: `/var/log/auth.log`, `secure`, `wtmp`, `btmp`, `lastlog`.
- Persistence: `crontab -l`, `/etc/cron.*`, `systemctl list-unit-files`, `~/.bashrc`, `ld.so.preload`.
- Exec history: `~/.bash_history`, `/root/.viminfo`, `journalctl --since`.
- Deleted-but-open: `lsof +L1`.

## Windows artifacts
- Event IDs: 4624 logon, 4625 fail, 4688 process, 4720 user, 7045 service, 1102 log clear.
- `Get-WinEvent -FilterHashtable @{LogName='Security';Id=4624} -MaxEvents 50`.
- Prefetch `C:\Windows\Prefetch`, Amcache, ShimCache, UserAssist, MFT (`MFTECmd`).
- `wevtutil epl Security C:\sec.evtx` before touching anything.

## Memory
- `vol -f mem.raw windows.pslist`, `netscan`, `cmdline`, `malfind`, `dumpfiles`.
- Linux: `avml`, `lime`; `strings -el` for UTF-16.

## Timeline
- `log2timeline.py` + `psort.py -o l2tcsv`; `mactime -b body.csv`.
- Sort by time, then look for clusters around first execution.

## Hidden knowledge
- Timestomping shows as `$STANDARD_INFORMATION` vs `$FILE_NAME` mismatch.
- Check `~/.ssh/authorized_keys` and `known_hosts` for lateral movement.
- `/proc/$PID/exe` reveals deleted binaries still running.
- Container logs: `docker logs`, `/var/lib/docker/containers/*/*-json.log`.
- Look for `curl|bash` in shell history and package manager logs (`/var/log/apt/history.log`).
