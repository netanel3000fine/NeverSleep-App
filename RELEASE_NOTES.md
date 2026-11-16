# 🌙 Never Sleep — v21.0.0

> **Release date:** November 16, 2026  
> **Platform:** Windows 10 / 11 (x64)  
> **App size:** ~8 MB

---

## 🚀 What's New in v21.0.0

### 💎 1. Authentic Liquid Glass Theme Integration
- **Official `@glass-sdk/liquid-glass` Engine**: Complete integration of continuous squircle optical glass (`radius={28}`).
- **Pure Refractive Optics**: Real-time glass refraction, specular caustics, and theme-accented ambient light sliding with cursor movements.
- **Translucent Glass Chassis**: Clean dark and luminous white glass gradient profiles with zero wallpaper dependencies.
- **Coffee Theme Parity**: Full parity with countdown rules (dynamic toggle between awake status and live ticking darken timer, and schedule settings navigation).
- **In-Card Frosted Glass Tooltips**: Scoped hover tooltips in both panels matching Coffee theme without obscuring the top "Keep screen" toggle.
- **Left-Aligned Activity Indicators**: Mouse cursor and keyboard activity indicators smoothly pop in directly on the left side where countdown digits tick.

---

### ⚙️ 2. Liquid Glass Settings Window & macOS Traffic Lights
- **Borderless Squircle Chassis**: On Liquid Glass theme, the native Windows title bar is replaced with a smooth squircle frame (`border-radius: 24px`) with no outer title bar borders.
- **macOS-Style Traffic Light Window Controls**:
  - 🟢 **Green**: Authentic macOS green button.
  - 🟡 **Yellow**: Minimizes the settings window.
  - 🔴 **Red**: Closes the settings window.
  - **Hover Micro-interactions**: Smooth fade-in of macOS symbols (`+`, `−`, `✕`) on hover.
- **Draggable Window Header**: Smooth header window dragging across your desktop.
- **Frosted Glass Cards & Controls**: Glass category blocks, Apple-style glass switches, glass selects/inputs, and floating popover modals.

---

### 🪟 3. Liquid Glass System Tray Flyout
- **Continuous Squircle Flyout**: Custom glass flyout card (`border-radius: 22px`) with refractive border, ambient corner caustics, and backdrop blur.
- **Frosted Sub-Boxes**: Schedule countdown and darken method selectors elevated on frosted glass surfaces (`border-radius: 14px`).
- **Glass Power Switch & Quick Timers**: Glass power slider with active glow and pill duration selectors matching the Liquid Glass aesthetic.

---

## 🐛 Summary of Changes & Fixes

| Component | Improvement / Fix |
|---|---|
| **Liquid Glass Mode** | Official `@glass-sdk/liquid-glass` integration with pure refractive optics and squircle curves (`radius={28}`) |
| **Settings Window** | Liquid Glass styling; native title bar replaced with top-right macOS traffic light controls (Green, Yellow, Red) |
| **Tray Flyout** | Squircle glass chassis (`radius={22}`), frosted sub-boxes, and glass controls |
| **Hover Tooltips** | Confined to bottom timer area, keeping top toggles fully accessible |
| **Activity Indicators** | Mouse cursor and keyboard icons now appear left-aligned in place of the countdown numbers |
| **Theme Isolation** | Coffee and Classic themes remain 100% untouched |
