# Never Sleep ☕

## Description
Never Sleep אפליקציה קלת משקל שמונעת מהמחשב שלך להירדם או להיכנס למצב השהייה. האפליקציה כוללת טיימרים להחשכת מסך אוטומטית וטיימר עירנות עד…, עם ממשק משתמש מודרני ונקי בעברית.

 זה אשכרה שוקל 8mb 
 
 <img width="671" height="48" alt="image" src="https://github.com/user-attachments/assets/5820a97f-1782-4dfc-ab4f-4737a9c189b0" />

Never Sleep a lightweight application that prevents your computer from going to sleep or entering standby mode. It features timers for automatic screen dimming and scheduled shutdowns, with a modern, clean Hebrew interface.

---

## 🖼️ Screenshots

### Main Window
<img width="625" height="325" alt="image" src="https://github.com/user-attachments/assets/b1d7fc71-c0ff-449e-8e4e-3397045ba270" />

<img width="700" height="55" alt="image" src="https://github.com/user-attachments/assets/fcd74ebf-a48a-4e80-816d-bbb40b951114" />

### Settings Window
<img width="627" height="788" alt="image" src="https://github.com/user-attachments/assets/58806d01-caf1-47f2-917b-7e4e24600124" />

### Sleep mode
<img width="625" height="325" alt="image" src="https://github.com/user-attachments/assets/a5f98005-abdb-41b1-8af8-0c6c9bae4116" />

### Pause app / Media is playing
<img width="625" height="325" alt="image" src="https://github.com/user-attachments/assets/cca666e8-75d2-467c-876f-88a264c30e03" />

---

## Key Features

- ✅ **שמירה על המחשב ער** - מונע מהמחשב להיכנס למצב שינה
- ⏱️ **טיימר להחשכת מסך** - החשכה אוטומטית לאחר זמן מוגדר
- 🔌 **כיבוי אוטומטי** - כיבוי מתוזמן של האפליקציה
- 🎨 **ערכות נושא צבעוניות** - התאמה אישית של צבע הממשק
- 🔔 **התראות מערכת** - התראות על פעולות חשובות
- 🚀 **הפעלה אוטומטית** - אופציה להפעלה עם הפעלת המחשב





- ✅ **Keep Computer Awake** - Prevents sleep/standby mode
- ⏱️ **Screen Darken Timer** - Auto-dim after specified time  
- 🔌 **Auto Shutdown** - Scheduled shutdown
- 🎨 **Color Themes** - Customize interface colors
- 🔔 **System Notifications** - Alerts for important actions
- 🚀 **Auto-start** - Launch on system startup


======================================



---

## Installation
1. הורד את קובץ ההתקנה `Never Sleep_...._x64-setup.exe` 📦 [Releases](https://github.com/netanel3000fine/NeverSleep-App/releases)  
2. הרץ את ההתקנה
3. פתח את האפליקציה


---
<img width="320" height="124" alt="68747470733a2f2f692e6962622e636f2f71306d6463345a2f6765742d69742d6f6e2d6769746875622e706e67" src="https://github.com/user-attachments/assets/a8681235-4c97-46f2-acfc-26e294ea133d" />

## Links

📦 [Releases](https://github.com/netanel3000fine/NeverSleep-App/releases)  
🐛 Issues & Feedback: Create an issue on GitHub
##Technologies

- **Tauri** - Cross-platform desktop framework
- **Rust** - Backend logic and system integration  
- **HTML/CSS/JavaScript** - Modern, responsive UI
- **WinAPI** - Windows system calls for sleep prevention

---

## License

MIT License - חופשי לשימוש ושינוי / Free to use and modify
# ☕ Never Sleep v13.0.0

## 🌍 Multi-Language Support (NEW)

Never Sleep now speaks your language! The app ships with full UI localization across all windows (main, settings, sleep):

| Language | Flag | Status |
|---|---|---|
| English | 🇬🇧 | ✅ Full |
| עברית (Hebrew) | 🇮🇱 | ✅ Full — RTL support |
| Русский (Russian) | 🇷🇺 | ✅ Full |

- All strings translated including category headers, tooltips, notifications, and status messages
- Correct RTL text direction automatically applied for Hebrew
- Language persists across restarts via saved settings
- **Fixed:** Language switching now applies immediately to the running window without restart

---

## 📅 Schedule Mode (NEW)

Keep your PC awake **only when you need it** using Time Profiles:

- Enable **Schedule Mode** in Settings → Schedule
- Add **Time Profiles** (e.g., 09:00 → 17:00 for work hours)
- Outside of those hours, sleep prevention automatically **pauses**
- A live indicator dot pulses next to the currently active profile
- The right panel shows a schedule overlay when sleep prevention is paused by a schedule
- **"Next schedule" countdown** shows in the main window when the next profile is upcoming

---

## 🎨 Three Visual Themes (NEW)

Choose your style in Settings → Design → View Mode:

| Theme | Description |
|---|---|
| **Classic** | Clean, minimal dark mode — lowest resource usage |
| **Liquid Glass** | Premium frosted-glass effect with mouse-tracking shimmer and acrylic Windows blur |
| **Coffee ☕** | Warm dark-brown aesthetic with coffee bean background and pulsing heartbeat animation |

Each theme adapts the border color, backgrounds, glows, and hover effects throughout the entire app.

---

## 🖼️ Custom App Icons (NEW)

Personalize your taskbar and tray icon:

- **6 icon choices** including 3D coffee cups, battery icons, and more
- Icons are applied live to the taskbar button, title bar, system tray, and Windows shortcuts
- Multi-resolution ICO files (16×16 → 256×256) are auto-generated from PNG sources
- Hard apply mode also updates `.lnk` shortcuts on the Desktop and Start Menu
- Icon preference is saved and restored on every app launch

---

## 📺 Media Detection — Auto-Pause (NEW)

Never Sleep can now detect when you're watching a video and **automatically pause** sleep prevention:

- Uses the Windows **Global Media Transport Controls** API (same as the taskbar media widget)
- Triggers only for video/browser content — **music from Spotify/Apple Music is excluded**
- The status pill in the header shows "**Media**" when paused by media detection
- A red "Paused — media playing" banner confirms the app is in auto-pause mode
- Toggle on/off in Settings → System → **Disable during video**

---

## 🔔 Notifications & Break Reminders (NEW)

- **Break Reminder**: Set a recurring interval (e.g., every 1 hour) to receive a system notification prompting you to take a break
- **Enable/disable** system notifications globally
- **System sounds** toggle for audio feedback on important events
- Notifications are suppressed when the Windows workstation is locked

---

## 🪟 Window Vibrancy & DWM Border Colors

- **Liquid Glass mode** applies Windows **Acrylic** blur effect to both the main and settings windows via `window-vibrancy`
- The **window border color** (DWM accent) updates to match your chosen theme color in real time
- Border color applies to both the main and settings windows simultaneously

---

## 🪲 Bug Fixes & Performance Improvements

| Fix | Details |
|---|---|
| 🐛 Language switching | Fixed: changing language in settings now immediately applies to the UI without needing a restart |
| ⚡ High CPU from mouse tracking | Mouse move events now debounced to **500ms** — eliminated unnecessary lock contention on cursor movement |
| 🔒 Single instance enforcement | Windows Mutex ensures only one instance of the app runs at a time |
| 🧠 Activity tracking | Global keyboard & mouse listener now correctly distinguishes input type (`mouse` vs `keyboard`) |
| 🖥️ Multi-monitor overlay | Screen darken overlays now correctly cover all monitors including non-primary ones |
| 💾 Settings persistence | All settings saved to and restored from `%AppData%\never-sleep\settings.json` |
| 🔄 Workstation lock detection | Notifications and certain operations now skip when the Windows session is locked |

---

## 🏗️ Technical Highlights

- Built with **Tauri** + **Rust** backend + **HTML/CSS/JS** frontend
- Uses **WinAPI** (`SetThreadExecutionState`) for sleep prevention
- Uses **Windows Media Transport Controls** (UWP API via `windows` crate) for media detection
- Global input listener via `rdev` crate
- Autostart via Windows Registry (`HKCU\...\Run`)
- Window effects via `window-vibrancy` crate (Acrylic)

---

## 📦 Download

| File | Description |
|---|---|
| `Never Sleep_10.0.0_x64-setup.exe` | Windows installer (recommended) |

**Requirements:** Windows 10 1903+ / Windows 11 · 64-bit · ~8 MB

---

*Made with ☕ — Never Sleep keeps your PC awake so you don't have to worry.*

