# 🌙 Never Sleep — v20.0.0

> **Release date:** November 10, 2026  
> **Platform:** Windows 10 / 11 (x64)  
> **App size:** ~8 MB

---

## 🚀 Major Release Highlights

### 🔒 1. Redesigned Lock Screen Schedule Mode
- **Interactive 24-Hour Circular Dial**: Set daily workstation lock times by clicking directly on the 24-hour dial ring.
- **Draggable Pins & Live Badge Preview**: Drag any lock pin around the dial with smooth 5-minute precision snapping and a real-time floating preview tooltip.
- **Removable Time Chips**: Every scheduled time appears in an interactive chips list below the dial with a one-click `×` remove button.
- **30-Second Warning Toast with Cancel Option**: Never gets caught by surprise — 30 seconds before any scheduled lock time, an alert toast with a live countdown and a **"Cancel"** button appears, letting you abort the lock if you are actively working.
- **Auto-Resume Upon Windows Unlock**: When you log back in or unlock your workstation, NeverSleep automatically re-enables sleep prevention seamlessly.

---

### 🎵 2. WASAPI Core Audio & Deep Video Player Detection
- **Broad Player Compatibility**: Intelligent background session tracking now detects video playback in players that do not register standard Windows SMTC controls, including **Stremio, VLC, MPC-HC, and browser media**.
- **Accurate Silence Filtering**: Distinguishes between actual active audio output and silent background sessions or system notification chimes.
- **Automatic Darken & Lock Suppression**: Sleep prevention stays active and screen darkening is gracefully deferred while media is playing.

---

### 🖥️ 3. Hardware-Level DDC/CI Lock Screen Protection
- **No More Lock-Screen Monitor Blinking**: Physical DDC/CI hardware brightness commands are now completely blocked at the OS driver level while Windows is locked.
- **State-Preserving Darken Freeze**: The screen darken timer pauses while the lock screen is active and restarts fresh upon unlocking.

---

### 🔄 4. Reliability, Tray & Updater Enhancements
- **Multi-Window Scope Fix**: Fixed an issue where scheduled lock pins were not saving to the background tray state.
- **Live Tray Countdown**: Tray flyout dynamically shows `LOCK AT HH:MM` and a live ticking countdown.
- **Streamlined GitHub Updater**: GitHub release checker queries the latest endpoint with direct tag parsing and download resolution.
- **Modern UI Fixes**: Fixed schedule card creation and layout alignment in classic mode.

---

## 🐛 Summary of Fixes

| # | Component | Resolution |
|---|---|---|
| 1 | **Schedule Dial** | Resolved JavaScript scope bug so multi-pin daily lock times reliably persist to settings |
| 2 | **Keep PC Awake Cards** | Fixed premature execution order so "+ Add Profile" and time cards work seamlessly |
| 3 | **Tray Synchronization** | Added instant `settings-updated` bridge so the tray menu immediately reflects lock times |
| 4 | **DDC/CI Brightness** | Hardware-level blocking ensures zero monitor blinking on the Windows lock screen |
| 5 | **Updater** | Enhanced GitHub API release query and browser launch mechanism |

---

## 📦 Download

| Installer | Description |
|---|---|
| `Never Sleep_20.0.0_x64-setup.exe` | **Recommended** — NSIS one-click installer |
| `Never Sleep_20.0.0_x64.msi` | MSI for enterprise / Group Policy deployment |
| `Never Sleep.exe` | Portable standalone executable (no installation required) |

> ℹ️ Windows may show a SmartScreen prompt on first launch — click **"More info → Run anyway"**. Never Sleep is 100% open-source, private, and offline.

---

*Made with ❤️ and Rust · [NeverSleep-App](https://github.com/netanel3000fine/NeverSleep-App)*
