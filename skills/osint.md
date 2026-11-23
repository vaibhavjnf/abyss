---
name: osint
description: People, domains, dorks, Shodan/Censys, breaches, metadata.
tags: [osint, dorks, shodan, people]
---
# OSINT

## Domains
- `whois`, `dig any`, `dig TXT`, `host -t mx`.
- crt.sh, `subfinder`, `amass`, `dnsx`.
- Shodan: `ssl:"example.com"`, `http.title:"Example"`, `org:"Example Inc"`, `http.favicon.hash:`.
- Censys: `services.tls.certificates.leaf_data.subject.common_name: example.com`.
- Wayback: `waybackurls example.com | grep -E '\.(js|json|env)$'`.

## Google dorks
- `site:example.com -www`, `filetype:pdf site:example.com`, `inurl:admin`, `intitle:"index of"`.
- `site:pastebin.com "example.com"`, `site:github.com "example.com" password`.
- `"@example.com" -site:example.com` (leaks on other sites).

## People
- `sherlock username`, `holehe email`, Hunter.io patterns.
- Email pattern: `first.last@`, `f.last@`, `first@` — verify with SMTP `VRFY` or a catch-all test.
- LinkedIn → team structure → usernames → `nxc smb` user list.
- Phone: `phoneinfoga`.

## Breaches
- HIBP API, `dehashed`, `snusbase`; check `;`-separated combo lists for reuse.
- Credential stuffing: one password across old breaches → spray.

## Metadata
- `exiftool file.pdf` — author, software, GPS, internal paths.
- Office docs: `unzip -p doc.docx docProps/core.xml` (creator, lastModifiedBy).
- Images: `exiftool -a -G1`, reverse image `tineye`, `yandex`.

## Hidden knowledge
- Certificate transparency is near-real-time: new subdomains appear within minutes.
- `dig +trace` reveals the real DNS provider when `NS` is proxied.
- GitHub commit history keeps deleted secrets: `git log -p -- .env`.
- Job posts leak stack: "AWS, Okta, Jenkins" → target selection.
- Check `/.well-known/security.txt` and `robots.txt` for internal paths.
