# 🌙 Never Sleep — v20.8.0

> **Release date:** November 11, 2026  
> **Platform:** Windows 10 / 11 (x64)  
> **App size:** ~8 MB

---

## 🚀 What's New in v20.8.0

### 🔒 1. Classic Mode Lock Warning — Full Bar Takeover
- **No More Overlap**: In Compact (Classic) Mode the lock warning pill no longer floats over and obscures the "Restart Timer" / "Disable" buttons. The entire 44 px bar is now cleanly replaced by the warning UI.
- **Full Bar Animation**: Dark blur background, red glow ring border, pulsing lock icon, countdown text, and a Cancel button — all inside the bar, nothing hidden behind it.
- **1-Second "Lock Cancelled" Feedback**: When you click Cancel the icon switches to a green 🔓, text changes to "Lock cancelled", the border glows green, and the bar smoothly restores itself after 1 second.
- **Full Localization**: Cancelled feedback translated in English, Hebrew, and Russian.

---

### ⏰ 2. Session Timer — "Session Finished" Rename & Bigger Font
- Renamed "Countdown Finished" → **"Session Finished"** across all three themes and all three languages (EN / HE / RU).
- Classic Mode bar now shows the title in a larger, bolder font (`15 px / 800 weight`) so it's readable in the compact layout.

---

### 🔄 3. Session Timer — Unlock Always Resets
- **Fixed**: Unlocking the PC while the Session Timer had already expired did not reset the timer. Now unlocking always calls the full reset routine regardless of sleep state, so the bar returns to its normal idle state immediately after login.

---

### 📦 4. Seamless In-App Update Experience (v20.1.0)
- **Zero Browser Redirects**: Checking for updates no longer opens the GitHub releases page. Everything happens inside the app.
- **In-App Release Modal**: Latest version badge, release date, and full markdown changelog displayed in Settings.
- **One-Click Download & Launch**: Stream the installer with a live progress bar (% and MB), auto-launch on completion.
- **Full Localization**: Entire update flow translated in English, Hebrew, and Russian.

---

## 🐛 Summary of Changes & Fixes

| Component | Improvement / Fix |
|---|---|
| **Classic Mode Bar** | Lock warning now takes over the full bar instead of floating over buttons |
| **Lock Cancel UX** | 1-second green feedback animation before bar restores |
| **Session Timer** | Renamed to "Session Finished"; bigger classic-bar font; unlock always resets |
| **Updater** | In-app modal with changelog, live progress bar, and auto-launch installer |
| **Localization** | `lock_cancelled` string added for EN / HE / RU |
