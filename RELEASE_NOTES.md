# 🌙 Never Sleep — v19.6.0

> **Release date:** November 3, 2026  
> **Platform:** Windows 10 / 11 (x64)  
> **App size:** ~8 MB

---

## ✨ What's New & Fixed in v19.6.0

### 🔒 Workstation Lock Awareness for Screen Darken
- **Darken countdown is now completely paused while Windows is locked (Win + L / workstation lock)**.
- **Prevents unnecessary DDC/CI monitor commands, overlays, focus window popups, and minimize actions while the lock screen is active**.
- **Fresh restart on unlock**: When you unlock your PC, the darken timer automatically resets to its full duration for seamless workflow continuation.

---

## 🐛 Fixes Summary

| # | Area | Description |
|---|------|-------------|
| 1 | **Darken Mode** | DDC/CI and overlay no longer attempt to activate when the computer is locked |
| 2 | **Timer** | Darken countdown freezes while workstation is locked, then cleanly resets on unlock |
| 3 | **UI / Focus** | Pop-up reminder and minimize behavior suppressed during Windows lock state |

---

## 📦 Download

| Installer | Description |
|-----------|-------------|
| Never Sleep_19.6.0_x64-setup.exe | **Recommended** — NSIS one-click installer |
| Never Sleep_19.6.0_x64.msi | MSI for enterprise / Group Policy deployment |

> ℹ️ Windows may show a SmartScreen prompt on first launch — click **"More info → Run anyway"**. The app is open-source.

---

*Made with ❤️ and Rust · [NeverSleep-App](https://github.com/netanel3000fine/NeverSleep-App)*