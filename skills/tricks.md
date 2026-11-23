---
name: tricks
description: One-liners, transfers, encoding, tmux, curl, ports — the stuff you forget.
tags: [oneliner, shell, transfer, encoding]
---
# Tricks

## File transfer
- Serve: `python3 -m http.server 8000 --bind 0.0.0.0`
- Fetch: `curl -O http://$IP:8000/f`, `wget`, `certutil -urlcache -f`, `bitsadmin`.
- Upload without curl: `nc $IP 9001 < file` (listener `nc -l -p 9001 > file`).
- Base64 over clipboard: `base64 -w0 f` → `echo $B64 | base64 -d > f`.
- SMB: `impacket-smbserver share . -smb2support`; Windows `copy \\$IP\share\f .`.

## Shell
- Loop: `for i in $(seq 1 254); do ping -c1 -W1 10.0.0.$i >/dev/null && echo $i; done`
- Port check: `nc -zvw1 $IP 22 80 443`; `/dev/tcp/$IP/$PORT`.
- Find writable: `find / -writable -type d 2>/dev/null`.
- Recent: `ls -lt | head`; `find . -newermt '-1 hour'`.
- Grep binary: `grep -a`, `strings -n 6`.

## Encoding
- URL: `python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))"`.
- Hex: `xxd -p`, `echo -n ... | xxd -r -p`.
- JWT: split on `.`, base64url decode parts 1-2.
- gzip+base64 one-liner: `echo -n 'cmd' | gzip | base64 -w0`.

## curl
- `-k` insecure, `-L` follow, `-I` headers, `-v` verbose, `--resolve host:443:IP` (test vhost before DNS).
- `-x http://127.0.0.1:8080` proxy, `--socks5-hostname 127.0.0.1:1080` (DNS through proxy).
- `-d @file`, `-H 'Content-Type: application/json'`, `--data-binary`.
- `-w '%{http_code} %{size_download}\n' -o /dev/null -s` for status sweeps.

## tmux
- `tmux new -s work`, `Ctrl-b d`, `tmux a -t work`.
- `Ctrl-b %` split vertical, `Ctrl-b "` horizontal, `Ctrl-b z` zoom.
- Sync panes: `:setw synchronize-panes on` — one command on ten shells.

## Ports & math
- CIDR: `nmap -sL -n 10.0.0.0/24` lists targets without scanning.
- `ipcalc`, `prips`; `/24` = 254 hosts, `/16` = 65k.
- Common: 22, 80, 443, 445, 3389, 5985, 8080, 8443, 9200, 6379, 27017.

## Hidden knowledge
- `Ctrl-x Ctrl-e` opens $EDITOR for the current command line — a lifesaver for long one-liners.
- `!!` repeats the last command; `!$` last arg; `^old^new` re-run with substitution.
- `history | grep` then `!1234`.
- `script -q out.txt` records a session; `asciinema rec` for demos.
- When a command "hangs", check DNS first (`getent hosts`), then MTU (`ping -M do -s 1472`).
