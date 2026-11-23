---
name: recon
description: Passive + active recon — subdomains, DNS, ports, screenshots, tech fingerprint, origin discovery.
tags: [enumeration, dns, nmap, osint]
---
# Recon

Rule: breadth before depth. One forgotten subdomain beats a perfect SQLi.

## Passive first
- crt.sh: `curl -s "https://crt.sh/?q=%25.$D&output=json" | jq -r '.[].name_value' | sort -u`
- subfinder: `subfinder -d $D -all -recursive -silent | anew subs.txt`
- amass passive: `amass enum -passive -d $D -o amass.txt`
- DNS history: SecurityTrails, DNSDumpster; `dig +short TXT $D` — SPF includes leak SaaS.
- GitHub dorks: `"$D" password`, `"$D" api_key`; org repos via `gh search code`.
- ASN: `whois -h whois.radb.net -- '-i origin AS1234' | grep -E '^route'` → scan netblocks, not just hostnames.

## Active
- Resolve: `dnsx -l subs.txt -a -resp -silent`; wildcard check `dig +short A $(head -c4 /dev/urandom | xxd -p).$D`.
- Ports: `nmap -sS -p- --min-rate 5000 -Pn -n -oA all $IP` then `nmap -sVC -p$PORTS $IP`.
  Fast top: `nmap --top-ports 1000 -sS -Pn -n --open $IP`.
  UDP: `nmap -sU --top-ports 50 -Pn $IP` (SNMP 161, NTP 123, IKE 500, mDNS 5353).
- HTTP: `httpx -l ips.txt -ports 80,443,8080,8443 -title -tech-detect -status-code -favicon -jarm`.
- Screenshots: `aquatone` / `gowitness scan file -f urls.txt`.
- Vhosts: `ffuf -u http://$IP -H "Host: FUZZ.$D" -w subs.txt -fs 0`.

## Origin discovery (behind CDN)
- Direct A records in DNS history, MX/SPF hosts, `dig TXT _dmarc`.
- Staging/dev subdomains that skip the CDN.
- Censys/Shodan: `ssl:"$D"`, `http.title:"$D"`, certificate SANs.
- Favicon hash: `python3 -c "import mmh3,requests,codecs;print(mmh3.hash(codecs.encode(requests.get('https://$D/favicon.ico').content,'base64')))"` → Shodan `http.favicon.hash:`.

## Hidden knowledge
- `nmap -sS` needs root; `-sT` works unprivileged but is slower and logs.
- `--min-rate` too high drops packets silently; re-scan misses with `--max-retries 1`.
- Check IPv6: `nmap -6`, `dig AAAA` — often un-firewalled.
- Certificate SANs reveal internal names: `openssl s_client -connect $IP:443 -servername $D </dev/null | openssl x509 -noout -text | grep DNS`.
- Always read `robots.txt`, `sitemap.xml`, `/.well-known/security.txt`, `/.git/config`, `/.env`.
