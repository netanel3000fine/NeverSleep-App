# 🌙 Never Sleep — v20.1.0

> **Release date:** November 10, 2026  
> **Platform:** Windows 10 / 11 (x64)  
> **App size:** ~8 MB

---

## 🚀 What's New in v20.1.0

### 📦 1. Seamless In-App Update Experience
- **Zero Browser Redirects**: Checking for updates no longer kicks you out to the GitHub releases web page. Everything happens directly inside the application.
- **In-App Release Modal**: View the latest version badge, release date, and full markdown changelog right within Settings.
- **One-Click Download & Launch**: Click **"Download & Update"** to stream the installer chunk-by-chunk with a real-time progress bar (percentage & downloaded MB / total MB), and launch the setup automatically when complete.
- **Subtle Toast Status**: When already running the latest version, a sleek non-intrusive in-app banner (`✓ You're up to date!`) appears instead of browser popups or alerts.
- **Full Localization**: The entire update experience is translated in English, Hebrew, and Russian.

---

### ⏰ 2. Restored Time Profiles ("Keep PC Awake")
- **Profile Cards Creation**: Resolved the script initialization order issue that previously prevented adding new cards.
- **Interactive 24-Hour Circular Dial**: Seamlessly set automated wake hours by dragging wake/sleep knobs around the circular dial with 5-minute precision snapping.
- **Multi-Day Selectors**: Choose active days per profile or toggle "All day" mode with one click.

---

### 🔒 3. Enhanced Lock Screen Schedule & Multi-Pin Dial
- **Interactive Multi-Pin Dial**: Click anywhere along the dial ring to drop daily lock pins, or drag existing pins smoothly to adjust times.
- **Live Status Badging**: Main window and system tray flyout dynamically display `Lock at HH:MM` along with a live ticking countdown (`HH:MM:SS`), or clearly indicate `NO LOCK SET` if no times are scheduled.
- **30-Second Warning Toast**: Automatically alerts you 30 seconds before any scheduled workstation lock, with a single-click **Cancel** button to abort if you are still working.
- **Auto-Resume Upon Unlock**: Sleep prevention resumes immediately upon logging back into Windows.

---

## 🐛 Summary of Changes & Fixes

| Component | Improvement / Fix |
|---|---|
| **Updater** | Modern in-app update modal with changelog, live progress bar, and automatic installer launch |
| **Keep PC Awake** | Fixed "+ Add Profile" button and card rendering in Settings |
| **Lock Screen Dial** | Restored interactive pin dragging, live countdown badges, and removable chips |
| **Tray Integration** | Live synchronization of lock screen mode, next lock time, and countdown |
| **Localization** | Added complete updater strings for English, Hebrew, and Russian |
