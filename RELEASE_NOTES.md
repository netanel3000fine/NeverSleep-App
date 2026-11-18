# 🌙 Never Sleep — v21.5.0

> **Release date:** November 19, 2026  
> **Platform:** Windows 10 / 11 (x64)  
> **App size:** ~8 MB

---

## 🚀 What's New in v21.5.0

### 🎨 Liquid Glass Theme — Major Polish
- **Logo & Icon as GlassButton**: The Never Sleep logo and app name are now grouped into a single cohesive `GlassButton`, matching the visual style of all other header controls.
- **Dynamic App Icon**: The header icon now reflects the user-selected app icon from Settings, consistent with how the Settings window title icon works.
- **Translucency Restored**: Main window and Settings window are now properly translucent in White Mode (frosted glass backdrop — not opaque).
- **Height Trimmed**: Main window height reduced to 190px, removing the extra empty zone between the toggle row and the status bar.
- **White Mode Toggle Contrast**: Toggles (keep screen, all settings toggles) now render with improved contrast when in Liquid Glass + White Mode.
- **Black Hole Dot**: The Black Hole animation now renders correctly in White Mode, with lensed arcs and accretion glow visible against light backgrounds.

### ☕ Coffee Theme — Settings Window
- **Animated Coffee Beans in Settings**: The floating coffee bean particles now also appear in the full settings window background, drifting behind setting cards.
- **White Mode Settings Fix**: Settings in Coffee + White Mode now render with warm latte colors (creamy background, espresso text) matching the main window's styling.

### 🗂️ Tray Window Enhancements
- **Comic Beak / Arrow**: A small downward-pointing nob now appears at the bottom-center of the tray flyout, cleanly themed across all 5 theme combinations (Default Dark, Liquid Glass Dark, Liquid Glass White, Coffee, White Mode).
- **Darken Method → Separator When Hidden**: When "Show quick toggles in tray" is OFF, the Darken Method section is now hidden and replaced by a clean separator line, reducing clutter.
- **Liquid Glass Caustic Orbs Removed**: The tinted ambient orbs (`::before` / `::after`) around the tray card borders in Liquid Glass mode have been removed for a cleaner flyout.

### 💻 Keep on Desktop — Win+D Support
- **All Themes**: When the app is pinned ("Always on Top" enabled), pressing Win+D no longer minimizes or hides the app window. Works across Classic, Liquid Glass, and Coffee themes.

### 🖼️ Update Modal — New Design
- **Download & Update Button**: Redesigned with a theme-colored gradient, specular top highlight, animated download icon, and smooth hover elevation.
- **Later Button**: Redesigned as a frosted glass secondary button with smooth hover transitions.
- **Full Theme Adaptation**: Both buttons adapt across Liquid Glass, Coffee, and White Mode.
- **Markdown Table Parser Fix**: Release notes tables no longer render as raw pipe text — CRLF line endings are now normalized, and tables use a styled frosted glass card layout.

### 🧹 Classic Theme
- **Cleaning Mode Button Removed**: The Cleaning Mode button that was erroneously added to the Classic theme has been fully removed.

---

## 🐛 Summary of Changes & Fixes

| Component | Improvement / Fix |
|---|---|
| **Liquid Glass Logo** | Icon + name grouped into single GlassButton matching header controls |
| **Dynamic App Icon** | Header icon in LG theme reflects user-selected icon from Settings |
| **LG Window Height** | Reduced to 190px; removed empty area between toggle row and status bar |
| **LG White Mode Translucency** | Main + Settings window properly frosted glass (not opaque white) |
| **LG Toggle Contrast** | Toggles show correct contrast in Liquid Glass White Mode |
| **Black Hole White Mode** | Accretion disk and lensed arcs now visible in White Mode |
| **Coffee Settings Beans** | Animated beans also appear in Settings window background |
| **Coffee White Mode Settings** | Settings adopt warm latte palette (not dark) in White + Coffee mode |
| **Tray Comic Beak** | Downward-pointing nob at bottom-center of tray flyout, all themes |
| **Tray Quick Toggles OFF** | Darken Method section replaced by a separator line when hidden |
| **Tray LG Caustics** | Removed tinted fade orbs around tray card borders in LG mode |
| **Keep on Desktop** | Win+D no longer hides pinned window in any theme |
| **Keyboard Icon Bug** | Critical fix: keyboard keys now show keyboard symbol (not cursor icon) |
| **Update Modal Buttons** | Modern glass-styled Download & Update + Later buttons with hover effects |
| **Markdown Table Parser** | CRLF normalization; tables render as frosted glass cards (not raw pipes) |
| **Classic Cleaning Mode** | Cleaning Mode button fully removed from Classic theme |
