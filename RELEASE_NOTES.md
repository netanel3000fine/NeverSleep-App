# 🌙 Never Sleep — v21.1.0

> **Release date:** November 16, 2026  
> **Platform:** Windows 10 / 11 (x64)  
> **App size:** ~8 MB

---

## 🚀 What's New in v21.1.0

### 🔄 In-App Updater & Seamless Auto-Relaunch
- **Markdown Tables & Formatting**: The in-app release notes modal now renders markdown tables, headers, code, and bullet lists cleanly.
- **Auto-Relaunch on Update**: Passing `/R` to the silent NSIS installer ensures the application automatically relaunches as the newly installed version immediately following countdown.

### 🎨 Header & Navigation Enhancements
- **Seamless Full-Header Window Dragging**: The entire top header area (including text, title, and floating icon) can now be dragged effortlessly to move the window in Liquid Glass mode, while maintaining precise clickability for the macOS traffic light controls.
- **Unified Modern Floating App Icon**: Boxless 44px icon styling standardized across all themes (Liquid Glass, Coffee, and Classic), with Liquid Glass featuring an active theme-color ambient glow.

---

## 🐛 Summary of Changes & Fixes

| Component | Improvement / Fix |
|---|---|
| **Updater Release Notes** | Full markdown pipe table parser with frosted glass header styling and syntax formatting |
| **Silent Updater** | Auto-relaunch flag (`/R`) added to NSIS command to start the upgraded app automatically after countdown |
| **Window Dragging** | Full header drag zone coverage across icon, title, and blank header areas with protected traffic light controls |
| **App Header Icon** | Standardized 44px boxless floating icon across Liquid Glass, Coffee, and Classic themes |
