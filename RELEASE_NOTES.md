# 🌙 Never Sleep — v19.5.0

> **Release date:** October 29, 2026  
> **Platform:** Windows 10 / 11 (x64)  
> **App size:** ~8 MB

---

## ✨ What's New

### 🕳️ Black Hole Darken Mode — Visual Overhaul
- The status indicator in **Darken Mode** has been transformed into an **animated black hole** with a glowing, tilted accretion ring, complete with a dynamic event-horizon shimmer.
- Refined the black hole visual: now features a **crisp 1 px ring** with a subtle outer glow while cleanly preserving the event-horizon boundary for a polished astronomy-grade look.

### ⌨️ Keyboard Cleaning Mode (improvements)
- Already introduced in v19.0.0 — this release polishes the unlock experience and makes the mode more reliable across edge cases.

### 🧊 Liquid Glass Theme
- Updated glass styling across all windows (main, settings, tray).
- Native window blur/vibrancy effect is now consistently applied to **Settings** and all HTML panels.
- Fixed **Liquid Glass contrast** in white/light mode — text and icons are now legible in all brightness conditions.

### 🗂️ System Tray — Overhaul
- Redesigned tray flyout with **section status indicators** (dot badges for active features).
- Tray status dot is now **perfectly circular** at all DPI scales.
- **White-mode toggle contrast** improved — the active state is now visually distinct on light backgrounds.
- Tray behavior settings **renamed** for clarity (e.g., "Pin on top" is now labelled consistently across UI and tray).

### 🛡️ Crash Resistance & Stability
- Hardened the Rust backend: **all unsafe `unwrap()` calls** have been replaced with proper error handling.
- Added a **desktop panic logger** — if the app encounters an unrecoverable error, a timestamped log file is written to the Desktop for easy diagnostics.
- Formatted the entire `main.rs` with `rustfmt` for long-term maintainability.

### 🔧 Developer & Build
- Added a `debug_logging.js` utility for structured front-end debug output (stripped in production).
- Updated Tauri configuration and dependencies to the latest patch versions.
- Automated codegen cache invalidation in the `prebuild` npm hook — stale builds are now fully prevented.

---

## 🐛 Bug Fixes

| # | Area | Description |
|---|------|-------------|
| 1 | **Tray** | Status dot was rendered as an ellipse on non-standard DPI — fixed to always render as a circle |
| 2 | **Tray** | White-mode toggle was nearly invisible against light tray background |
| 3 | **Settings** | Liquid Glass theme had low contrast in white/light mode |
| 4 | **Crash Safety** | Several Rust `unwrap()` panics on edge-case OS states now handled gracefully |
| 5 | **Darken Mode** | Accretion ring clipped incorrectly at certain window sizes — now fully contained within its event horizon |
| 6 | **Schedule** | Schedule dial no longer overshoots midnight (previously could wrap to 00:01 erroneously) |
| 7 | **App Icon** | Windows shortcut icon was reverting to generic Tauri icon on some machines — now correctly restored on launch |
| 8 | **WebView2 Cache** | Stale HTTP/code cache caused UI to show outdated JS after an update — invalidated on every startup |

---

## 🔄 Changes Since v19.0.0

```
✦ Refine darken mode black hole: thin 1px crisp ring with subtle glow
✦ Transform darken mode dot into animated black hole with accretion ring
✦ Harden crash resistance, desktop panic logger, eliminate unsafe unwraps
✦ Update Liquid Glass theme — native window effect in settings & HTML files
✦ Add debug_logging.js utility, update Tauri config and dependencies
✦ Fix Liquid Glass contrast in white mode
✦ Rename tray behavior settings for consistency
✦ Add tray section status indicators (dot badges)
✦ Restore saved app icon on Windows shortcuts
✦ Keep tray status dot circular at all DPI
✦ Improve tray toggle contrast in white mode
```

---

## 📦 Download

| Installer | Description |
|-----------|-------------|
| `Never Sleep_19.5.0_x64-setup.exe` | **Recommended** — NSIS one-click installer |
| `Never Sleep_19.5.0_x64.msi` | MSI for enterprise / Group Policy deployment |

> ℹ️ Windows may show a SmartScreen prompt on first launch — click **"More info → Run anyway"**. The app is unsigned but open-source.

---

## 🛠️ Requirements

- **OS:** Windows 10 (1903+) or Windows 11
- **Runtime:** [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (pre-installed on Win 11; auto-installed on Win 10)
- **Architecture:** x86-64 only

---

## 💬 Feedback & Issues

Found a bug or have a feature request?  
👉 [Open an issue](https://github.com/netanel3000fine/NeverSleep-App/issues) on GitHub

---

*Made with ❤️ and Rust · [NeverSleep-App](https://github.com/netanel3000fine/NeverSleep-App)*
