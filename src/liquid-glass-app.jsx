import React, { useState, useEffect, useRef } from 'react';
import { createRoot } from 'react-dom/client';
import {
  GlassScene,
  GlassContent,
  GlassSurface,
  GlassSwitch,
  GlassButton
} from '@glass-sdk/liquid-glass';

export function LiquidGlassApp({ initialProps = {} }) {
  const [state, setState] = useState({
    appearance: 'dark', // 'dark' | 'light'
    material: 'clear',  // 'clear' | 'regular'
    isAwake: true,
    isDisabled: false,
    keepAwakeLabel: 'Keep screen',
    pcAwakeTitle: 'Your PC is<br><span>awake now</span>',
    timeToDarkenLabel: 'Time to darken',
    darkenTime: '00:10:00',
    scheduleLabel: 'Schedule',
    timeLeftLabel: 'Time left',
    shutdownTime: '00:00:00',
    themeColor: '#34C759',
    ...initialProps
  });

  const appRef = useRef(null);

  useEffect(() => {
    window.__updateNeverSleepLiquidGlass = (partial) => {
      setState((prev) => ({ ...prev, ...partial }));
    };
    return () => {
      window.__updateNeverSleepLiquidGlass = null;
    };
  }, []);

  // Pointer movement tracking for corner glass specular caustics
  const handlePointerMove = (e) => {
    if (!appRef.current) return;
    const rect = appRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    appRef.current.style.setProperty('--mouse-x', `${x}px`);
    appRef.current.style.setProperty('--mouse-y', `${y}px`);
    appRef.current.style.setProperty('--lg-glow-x', `${(x / rect.width) * 100}%`);
    appRef.current.style.setProperty('--lg-glow-y', `${(y / rect.height) * 100}%`);
  };

  const handleToggleAwake = (checked) => {
    setState((prev) => ({ ...prev, isAwake: checked }));
    if (typeof window.__onLiquidGlassToggleAwake === 'function') {
      window.__onLiquidGlassToggleAwake(checked);
    }
  };

  const handleToggleDisabled = (e) => {
    e.stopPropagation();
    if (typeof window.__onLiquidGlassToggleDisabled === 'function') {
      window.__onLiquidGlassToggleDisabled();
    }
  };

  const handleOpenSettings = (e) => {
    e.stopPropagation();
    if (typeof window.__onLiquidGlassOpenSettings === 'function') {
      window.__onLiquidGlassOpenSettings();
    }
  };

  const handleOpenTray = (e) => {
    e.stopPropagation();
    if (typeof window.__onLiquidGlassOpenTray === 'function') {
      window.__onLiquidGlassOpenTray();
    }
  };

  const handleOpenSchedule = (e) => {
    e.stopPropagation();
    if (typeof window.__onLiquidGlassOpenSchedule === 'function') {
      window.__onLiquidGlassOpenSchedule();
    }
  };

  const isDark = state.appearance !== 'light';
  const textColor = isDark ? '#ffffff' : '#111113';
  const textSecondary = isDark ? 'rgba(255, 255, 255, 0.65)' : 'rgba(17, 17, 19, 0.60)';
  const activeAccent = state.themeColor || '#34C759';

  return (
    <div
      ref={appRef}
      onPointerMove={handlePointerMove}
      style={{
        width: 500,
        height: 260,
        position: 'relative',
        userSelect: 'none',
        overflow: 'hidden',
        boxSizing: 'border-box',
        padding: '5px',
        fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif',
        '--lg-accent': activeAccent
      }}
      data-tauri-drag-region
    >
      <GlassScene
        material={state.material}
        appearance={state.appearance}
        style={{
          width: '100%',
          height: '100%',
          position: 'relative',
          display: 'flex',
          flexDirection: 'column',
          boxSizing: 'border-box',
          overflow: 'visible'
        }}
      >
        <GlassContent>
          {/* Rich optical backdrop that gets refracted across all squircle corners */}
          <div
            style={{
              position: 'absolute',
              inset: 0,
              width: '100%',
              height: '100%',
              borderRadius: 28,
              background: isDark
                ? 'radial-gradient(ellipse 130% 100% at 50% -20%, rgba(255, 255, 255, 0.16) 0%, rgba(20, 28, 42, 0.95) 55%, #080c14 100%)'
                : 'radial-gradient(ellipse 130% 100% at 50% -20%, rgba(255, 255, 255, 0.98) 0%, rgba(235, 240, 248, 0.94) 60%, #cbd5e1 100%)',
              boxShadow: isDark
                ? '0 16px 48px rgba(0,0,0,0.6), inset 0 1.5px 0 rgba(255,255,255,0.35)'
                : '0 16px 48px rgba(0,0,0,0.12), inset 0 1.5px 0 rgba(255,255,255,0.95)',
              overflow: 'hidden'
            }}
          >
            {/* Prismatic caustic corner lighting */}
            <div
              style={{
                position: 'absolute',
                top: -30,
                left: -30,
                width: 120,
                height: 120,
                borderRadius: '50%',
                background: `radial-gradient(circle, ${activeAccent}44 0%, transparent 70%)`,
                filter: 'blur(16px)',
                pointerEvents: 'none'
              }}
            />
            <div
              style={{
                position: 'absolute',
                bottom: -30,
                right: -30,
                width: 130,
                height: 130,
                borderRadius: '50%',
                background: `radial-gradient(circle, ${activeAccent}33 0%, transparent 70%)`,
                filter: 'blur(18px)',
                pointerEvents: 'none'
              }}
            />
          </div>
        </GlassContent>

        {/* Outer Continuous Squircle Glass Surface (radius=28) */}
        <GlassSurface
          interactive
          radius={28}
          style={{
            width: '100%',
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            justifyContent: 'space-between',
            boxSizing: 'border-box',
            position: 'relative',
            zIndex: 1
          }}
          data-tauri-drag-region
        >
          {/* ── Header Toolbar ── */}
          <div
            data-tauri-drag-region
            style={{
              height: 48,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '0 16px',
              position: 'relative',
              zIndex: 10
            }}
          >
            {/* Logo & Brand Title */}
            <div
              data-tauri-drag-region
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 9,
                cursor: 'default'
              }}
            >
              <div
                style={{
                  width: 24,
                  height: 24,
                  borderRadius: '50%',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  background: activeAccent,
                  boxShadow: `0 0 14px ${activeAccent}99`,
                  transition: 'background-color 0.25s ease'
                }}
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#ffffff" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
                </svg>
              </div>
              <span
                style={{
                  fontWeight: 700,
                  fontSize: 14,
                  letterSpacing: -0.2,
                  color: textColor
                }}
              >
                Never Sleep
              </span>
            </div>

            {/* Right Header Actions: Disable Pill, Tray Button, Settings Button */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              {/* Disable App Toggle Pill (mirroring Coffee theme) */}
              <div
                onClick={handleToggleDisabled}
                title={state.isDisabled ? 'Enable Never Sleep' : 'Disable Never Sleep'}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  padding: '3px 10px',
                  borderRadius: 20,
                  fontSize: 11,
                  fontWeight: 600,
                  cursor: 'pointer',
                  background: state.isDisabled ? 'rgba(239, 68, 68, 0.22)' : 'rgba(128, 128, 128, 0.16)',
                  border: state.isDisabled ? '1px solid rgba(239, 68, 68, 0.55)' : '1px solid rgba(255, 255, 255, 0.16)',
                  color: state.isDisabled ? '#ef4444' : textColor,
                  transition: 'all 0.2s ease',
                  marginRight: 4
                }}
              >
                <span
                  style={{
                    width: 6,
                    height: 6,
                    borderRadius: '50%',
                    background: state.isDisabled ? '#ef4444' : '#34C759',
                    boxShadow: state.isDisabled ? '0 0 6px #ef4444' : '0 0 6px #34C759'
                  }}
                />
                <span>{state.isDisabled ? 'Disabled' : 'Active'}</span>
              </div>

              {/* Tray Action Disc */}
              <GlassButton
                data-size="icon-sm"
                onClick={handleOpenTray}
                title="System Tray"
                style={{ borderRadius: '50%' }}
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </GlassButton>

              {/* Settings Action Disc */}
              <GlassButton
                data-size="icon-sm"
                onClick={handleOpenSettings}
                title="Settings"
                style={{ borderRadius: '50%' }}
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="12" cy="12" r="3" />
                  <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
                </svg>
              </GlassButton>
            </div>
          </div>

          {/* ── Main 2-Panel Grid ── */}
          <div
            data-tauri-drag-region
            style={{
              flex: 1,
              display: 'grid',
              gridTemplateColumns: '1fr 1fr',
              gap: 12,
              padding: '0 12px 14px',
              boxSizing: 'border-box'
            }}
          >
            {/* Left Glass Surface (Keep Screen / Status or Darken Timer) */}
            <GlassSurface
              interactive
              radius={22}
              style={{
                padding: '14px 16px',
                display: 'flex',
                flexDirection: 'column',
                justifyContent: 'space-between',
                boxSizing: 'border-box',
                cursor: 'default',
                background: isDark ? 'rgba(255, 255, 255, 0.04)' : 'rgba(255, 255, 255, 0.65)',
                border: isDark ? '1px solid rgba(255, 255, 255, 0.16)' : '1px solid rgba(0, 0, 0, 0.08)'
              }}
              data-tauri-drag-region
            >
              {/* Top row: Label + GlassSwitch with active tint */}
              <div
                data-tauri-drag-region
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between'
                }}
              >
                <span
                  style={{
                    fontSize: 14,
                    fontWeight: 600,
                    letterSpacing: -0.2,
                    color: textColor
                  }}
                >
                  {state.keepAwakeLabel}
                </span>
                <GlassSwitch
                  checked={state.isAwake}
                  onCheckedChange={handleToggleAwake}
                  tint={activeAccent}
                  name="keepAwakeSwitch"
                />
              </div>

              {/* Bottom: Follows Coffee Theme Rule exactly! */}
              {state.isAwake ? (
                /* When Keep Screen is ON: Show 'Your PC is awake now' status and HIDE the timer countdown */
                <div data-tauri-drag-region style={{ marginTop: 'auto' }}>
                  <div
                    style={{
                      fontSize: 13,
                      fontWeight: 500,
                      color: textSecondary,
                      lineHeight: 1.25
                    }}
                  >
                    Your PC is
                  </div>
                  <div
                    style={{
                      fontSize: 19,
                      fontWeight: 700,
                      letterSpacing: -0.3,
                      color: textColor,
                      marginTop: 2
                    }}
                  >
                    awake now
                  </div>
                </div>
              ) : (
                /* When Keep Screen is OFF: Reveal 'Time to darken' and the live ticking countdown */
                <div data-tauri-drag-region style={{ marginTop: 'auto' }}>
                  <div
                    style={{
                      fontSize: 11,
                      fontWeight: 500,
                      color: textSecondary,
                      marginBottom: 2
                    }}
                  >
                    {state.timeToDarkenLabel}
                  </div>
                  <div
                    style={{
                      fontSize: 27,
                      fontWeight: 700,
                      letterSpacing: -0.6,
                      fontVariantNumeric: 'tabular-nums',
                      color: activeAccent,
                      textShadow: isDark ? `0 2px 14px ${activeAccent}55` : 'none'
                    }}
                  >
                    {state.darkenTime}
                  </div>
                </div>
              )}
            </GlassSurface>

            {/* Right Glass Surface (Schedule / Time Left — clickable like Coffee theme) */}
            <GlassSurface
              interactive
              radius={22}
              style={{
                padding: '14px 16px',
                display: 'flex',
                flexDirection: 'column',
                justifyContent: 'space-between',
                boxSizing: 'border-box',
                cursor: 'default',
                background: isDark ? 'rgba(255, 255, 255, 0.04)' : 'rgba(255, 255, 255, 0.65)',
                border: isDark ? '1px solid rgba(255, 255, 255, 0.16)' : '1px solid rgba(0, 0, 0, 0.08)'
              }}
              data-tauri-drag-region
            >
              {/* Top row: Schedule Title (Clickable link to Settings) */}
              <div
                data-tauri-drag-region
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between'
                }}
              >
                <span
                  onClick={handleOpenSchedule}
                  title="Open Schedule Settings"
                  style={{
                    fontSize: 14,
                    fontWeight: 600,
                    letterSpacing: -0.2,
                    color: textColor,
                    cursor: 'pointer',
                    textDecoration: 'underline dotted',
                    textUnderlineOffset: '3px'
                  }}
                >
                  {state.scheduleLabel}
                </span>
                <div
                  style={{
                    width: 7,
                    height: 7,
                    borderRadius: '50%',
                    background: activeAccent,
                    boxShadow: `0 0 8px ${activeAccent}`
                  }}
                />
              </div>

              {/* Bottom: Time left & Countdown */}
              <div data-tauri-drag-region style={{ marginTop: 'auto' }}>
                <div
                  style={{
                    fontSize: 11,
                    fontWeight: 500,
                    color: textSecondary,
                    marginBottom: 2
                  }}
                >
                  {state.timeLeftLabel}
                </div>
                <div
                  style={{
                    fontSize: 27,
                    fontWeight: 700,
                    letterSpacing: -0.6,
                    fontVariantNumeric: 'tabular-nums',
                    color: isDark ? 'rgba(255, 255, 255, 0.95)' : '#111113',
                    textShadow: isDark ? '0 2px 10px rgba(0,0,0,0.4)' : 'none'
                  }}
                >
                  {state.shutdownTime}
                </div>
              </div>
            </GlassSurface>
          </div>
        </GlassSurface>
      </GlassScene>
    </div>
  );
}

// Global mounting API
let currentRoot = null;

window.NeverSleepLiquidGlass = {
  mount(containerEl, initialProps = {}) {
    if (!containerEl) return;
    if (currentRoot) {
      try { currentRoot.unmount(); } catch (_) {}
      currentRoot = null;
    }
    currentRoot = createRoot(containerEl);
    currentRoot.render(<LiquidGlassApp initialProps={initialProps} />);
  },
  updateState(partialState = {}) {
    if (typeof window.__updateNeverSleepLiquidGlass === 'function') {
      window.__updateNeverSleepLiquidGlass(partialState);
    }
  },
  unmount() {
    if (currentRoot) {
      try { currentRoot.unmount(); } catch (_) {}
      currentRoot = null;
    }
  }
};
