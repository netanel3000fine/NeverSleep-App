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
    appIcon: 'A visually striking illustration.png',
    tipStatus: '',
    tipDarken: '',
    tipShutdown: '',
    ...initialProps
  });

  const [hoveredPanel, setHoveredPanel] = useState(null); // 'left' | 'right' | null
  const [activeActivity, setActiveActivity] = useState(null); // 'mouse' | 'keyboard' | null
  const activityTimerRef = useRef(null);
  const appRef = useRef(null);

  useEffect(() => {
    window.__updateNeverSleepLiquidGlass = (partial) => {
      setState((prev) => ({ ...prev, ...partial }));
    };
    window.__triggerNeverSleepActivity = (type) => {
      setActiveActivity(type);
      if (activityTimerRef.current) clearTimeout(activityTimerRef.current);
      activityTimerRef.current = setTimeout(() => {
        setActiveActivity(null);
      }, 800);
    };
    return () => {
      window.__updateNeverSleepLiquidGlass = null;
      window.__triggerNeverSleepActivity = null;
      if (activityTimerRef.current) clearTimeout(activityTimerRef.current);
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

  const handleOpenCleaning = (e) => {
    e.stopPropagation();
    if (typeof window.__onLiquidGlassOpenCleaning === 'function') {
      window.__onLiquidGlassOpenCleaning();
    }
  };

  const isDark = state.appearance !== 'light';
  const textColor = isDark ? '#ffffff' : '#111113';
  const textSecondary = isDark ? 'rgba(255, 255, 255, 0.65)' : 'rgba(17, 17, 19, 0.60)';
  const activeAccent = state.themeColor || '#34C759';

  const leftTooltipHtml = state.isAwake
    ? (state.tipStatus || "Screen won't turn off as long as app is active.")
    : (state.tipDarken || 'After countdown, screen goes dark but PC stays awake.');

  const rightTooltipHtml = state.tipShutdown || 'When timer is active, PC will sleep after time ends.';

  return (
    <div
      ref={appRef}
      onPointerMove={handlePointerMove}
      style={{
        width: 500,
        height: 190,
        position: 'relative',
        userSelect: 'none',
        overflow: 'hidden',
        boxSizing: 'border-box',
        borderRadius: 28,
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
          overflow: 'hidden',
          borderRadius: 28
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
                : 'radial-gradient(ellipse 130% 100% at 50% -20%, rgba(255,255,255,0.55) 0%, rgba(220,230,245,0.38) 60%, rgba(185,200,220,0.28) 100%)',
              boxShadow: isDark
                ? '0 16px 48px rgba(0,0,0,0.6)'
                : '0 16px 48px rgba(0,0,0,0.12)',
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
            zIndex: 1,
            borderRadius: 28,
            overflow: 'hidden'
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
            {/* Logo & Brand — single GlassButton matching the right action pills */}
            <GlassButton
              data-tauri-drag-region
              title="Never Sleep"
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 7,
                padding: '4px 10px',
                borderRadius: 20,
                height: 28,
                minWidth: 0,
                cursor: 'default',
                pointerEvents: 'none'
              }}
            >
              <img
                src={state.appIcon || 'A visually striking illustration.png'}
                alt="App Icon"
                style={{
                  width: 20,
                  height: 20,
                  borderRadius: 5,
                  objectFit: 'cover',
                  flexShrink: 0,
                  pointerEvents: 'none'
                }}
              />
              <span
                style={{
                  fontWeight: 700,
                  fontSize: 14,
                  letterSpacing: -0.2,
                  color: textColor,
                  whiteSpace: 'nowrap'
                }}
              >
                Never Sleep
              </span>
            </GlassButton>

            {/* Right Header Actions: Cleaning Button, Disable Pill, Tray Button, Settings Button */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              {/* Cleaning Mode Button */}
              <GlassButton
                data-size="icon-sm"
                onClick={handleOpenCleaning}
                title="Keyboard Cleaning Mode"
                style={{ borderRadius: '50%' }}
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="2" y="4" width="20" height="16" rx="3.5" ry="3.5" />
                  <line x1="6" y1="8" x2="6.01" y2="8" strokeWidth="2.5" />
                  <line x1="10" y1="8" x2="10.01" y2="8" strokeWidth="2.5" />
                  <line x1="14" y1="8" x2="14.01" y2="8" strokeWidth="2.5" />
                  <line x1="18" y1="8" x2="18.01" y2="8" strokeWidth="2.5" />
                  <line x1="6" y1="12" x2="6.01" y2="12" strokeWidth="2.5" />
                  <line x1="10" y1="12" x2="10.01" y2="12" strokeWidth="2.5" />
                  <line x1="14" y1="12" x2="14.01" y2="12" strokeWidth="2.5" />
                  <line x1="18" y1="12" x2="18.01" y2="12" strokeWidth="2.5" />
                  <line x1="8" y1="16" x2="16" y2="16" strokeWidth="2" />
                </svg>
              </GlassButton>

              {/* Disable App Toggle Pill */}
              <GlassButton
                onClick={handleToggleDisabled}
                title={state.isDisabled ? 'Enable Never Sleep' : 'Disable Never Sleep'}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 6,
                  padding: '4px 10px',
                  borderRadius: 20,
                  fontSize: 11,
                  fontWeight: 600,
                  height: 28,
                  minWidth: 0,
                  color: state.isDisabled ? '#ef4444' : textColor,
                  background: state.isDisabled ? 'rgba(239, 68, 68, 0.22)' : undefined,
                  borderColor: state.isDisabled ? 'rgba(239, 68, 68, 0.55)' : undefined
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
              </GlassButton>

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
            <div
              style={{ position: 'relative', height: '100%', borderRadius: 22, overflow: 'hidden' }}
              data-tauri-drag-region
            >
              <GlassSurface
                interactive
                radius={22}
                style={{
                  width: '100%',
                  height: '100%',
                  padding: '14px 16px',
                  display: 'flex',
                  flexDirection: 'column',
                  justifyContent: 'space-between',
                  boxSizing: 'border-box',
                  cursor: 'default',
                  background: isDark ? 'rgba(255, 255, 255, 0.04)' : 'rgba(255, 255, 255, 0.65)',
                  border: isDark ? '1px solid rgba(255, 255, 255, 0.16)' : '1px solid rgba(0, 0, 0, 0.08)',
                  borderRadius: 22,
                  position: 'relative'
                }}
                data-tauri-drag-region
              >
                {/* Top row: Label + GlassSwitch with active tint (never hidden by tooltip) */}
                <div
                  data-tauri-drag-region
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    position: 'relative',
                    zIndex: 25
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

                {/* Bottom Section: Tooltip hover target (matches Coffee theme darkenSection) */}
                <div
                  onMouseEnter={() => setHoveredPanel('left')}
                  onMouseLeave={() => setHoveredPanel(null)}
                  data-tauri-drag-region
                  style={{
                    marginTop: 'auto',
                    position: 'relative',
                    minHeight: 52,
                    display: 'flex',
                    flexDirection: 'column',
                    justifyContent: 'flex-end',
                    cursor: 'help'
                  }}
                >
                  {state.isAwake ? (
                    /* When Keep Screen is ON: Show 'Your PC is awake now' status and HIDE the timer countdown */
                    <div data-tauri-drag-region>
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
                    <div data-tauri-drag-region>
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
                          position: 'relative',
                          height: 32,
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'flex-start'
                        }}
                      >
                        <div
                          style={{
                            fontSize: 27,
                            fontWeight: 700,
                            letterSpacing: -0.6,
                            fontVariantNumeric: 'tabular-nums',
                            color: activeAccent,
                            textShadow: isDark ? `0 2px 14px ${activeAccent}55` : 'none',
                            opacity: activeActivity ? 0 : 1,
                            transition: 'opacity 0.2s ease',
                            textAlign: 'left'
                          }}
                        >
                          {state.darkenTime}
                        </div>

                        {/* Animated Activity Indicator (Mouse / Keyboard) - Left-aligned over count numbers */}
                        {activeActivity && (
                          <div
                            style={{
                              position: 'absolute',
                              left: 0,
                              top: '50%',
                              transform: 'translateY(-50%)',
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'flex-start',
                              pointerEvents: 'none',
                              color: activeAccent,
                              animation: 'popIn 0.3s cubic-bezier(0.34, 1.56, 0.64, 1)'
                            }}
                          >
                            {activeActivity === 'keyboard' ? (
                              <svg width="26" height="26" viewBox="0 0 24 24" fill="currentColor">
                                <path d="M20 5H4c-1.1 0-1.99.9-1.99 2L2 17c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm-9 3h2v2h-2V8zm0 3h2v2h-2v-2zM8 8h2v2H8V8zm0 3h2v2H8v-2zm-1 2H5v-2h2v2zm0-3H5V8h2v2zm9 7H8v-2h8v2zm0-4h-2v-2h2v2zm0-3h-2V8h2v2zm3 3h-2v-2h2v2zm0-3h-2V8h2v2z" />
                              </svg>
                            ) : (
                              <svg width="26" height="26" viewBox="0 0 24 24" fill="currentColor">
                                <path d="M7 2L19.2 13.2 13.4 13.7 16.7 21 14.5 21.9 11.3 14.6 6.9 18.9V2z" />
                              </svg>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                  )}

                  {/* Confined In-Card Frosted Glass Tooltip Overlay (bottom area only, matching Coffee theme) */}
                  <div
                    style={{
                      position: 'absolute',
                      inset: '-4px -6px -4px -6px',
                      borderRadius: 14,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      padding: '8px 10px',
                      background: isDark ? 'rgba(16, 22, 34, 0.94)' : 'rgba(255, 255, 255, 0.94)',
                      color: textColor,
                      fontSize: 12,
                      lineHeight: 1.25,
                      textAlign: 'center',
                      boxSizing: 'border-box',
                      pointerEvents: 'none',
                      opacity: hoveredPanel === 'left' ? 1 : 0,
                      transform: hoveredPanel === 'left' ? 'scale(1)' : 'scale(0.96)',
                      transition: 'opacity 0.22s cubic-bezier(0.16, 1, 0.3, 1), transform 0.22s cubic-bezier(0.16, 1, 0.3, 1)',
                      zIndex: 20,
                      backdropFilter: 'blur(16px)',
                      WebkitBackdropFilter: 'blur(16px)',
                      boxShadow: isDark
                        ? 'inset 0 1px 0 rgba(255,255,255,0.2), 0 8px 24px rgba(0,0,0,0.5)'
                        : 'inset 0 1px 0 rgba(255,255,255,0.9), 0 8px 24px rgba(0,0,0,0.1)'
                    }}
                    dangerouslySetInnerHTML={{ __html: leftTooltipHtml }}
                  />
                </div>
              </GlassSurface>
            </div>

            {/* Right Glass Surface (Schedule / Time Left — clickable like Coffee theme) */}
            <div
              style={{ position: 'relative', height: '100%', borderRadius: 22, overflow: 'hidden' }}
              data-tauri-drag-region
            >
              <GlassSurface
                interactive
                radius={22}
                style={{
                  width: '100%',
                  height: '100%',
                  padding: '14px 16px',
                  display: 'flex',
                  flexDirection: 'column',
                  justifyContent: 'space-between',
                  boxSizing: 'border-box',
                  cursor: 'default',
                  background: isDark ? 'rgba(255, 255, 255, 0.04)' : 'rgba(255, 255, 255, 0.65)',
                  border: isDark ? '1px solid rgba(255, 255, 255, 0.16)' : '1px solid rgba(0, 0, 0, 0.08)',
                  borderRadius: 22,
                  position: 'relative'
                }}
                data-tauri-drag-region
              >
                {/* Top row: Schedule Title (Clickable link to Settings, never hidden by tooltip) */}
                <div
                  data-tauri-drag-region
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    position: 'relative',
                    zIndex: 25
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

                {/* Bottom Section: Tooltip hover target (matches Coffee theme shutdownSection) */}
                <div
                  onMouseEnter={() => setHoveredPanel('right')}
                  onMouseLeave={() => setHoveredPanel(null)}
                  data-tauri-drag-region
                  style={{
                    marginTop: 'auto',
                    position: 'relative',
                    minHeight: 52,
                    display: 'flex',
                    flexDirection: 'column',
                    justifyContent: 'flex-end',
                    cursor: 'help'
                  }}
                >
                  <div data-tauri-drag-region>
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

                  {/* Confined In-Card Frosted Glass Tooltip Overlay (bottom area only, matching Coffee theme) */}
                  <div
                    style={{
                      position: 'absolute',
                      inset: '-4px -6px -4px -6px',
                      borderRadius: 14,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      padding: '8px 10px',
                      background: isDark ? 'rgba(16, 22, 34, 0.94)' : 'rgba(255, 255, 255, 0.94)',
                      color: textColor,
                      fontSize: 12,
                      lineHeight: 1.25,
                      textAlign: 'center',
                      boxSizing: 'border-box',
                      pointerEvents: 'none',
                      opacity: hoveredPanel === 'right' ? 1 : 0,
                      transform: hoveredPanel === 'right' ? 'scale(1)' : 'scale(0.96)',
                      transition: 'opacity 0.22s cubic-bezier(0.16, 1, 0.3, 1), transform 0.22s cubic-bezier(0.16, 1, 0.3, 1)',
                      zIndex: 20,
                      backdropFilter: 'blur(16px)',
                      WebkitBackdropFilter: 'blur(16px)',
                      boxShadow: isDark
                        ? 'inset 0 1px 0 rgba(255,255,255,0.2), 0 8px 24px rgba(0,0,0,0.5)'
                        : 'inset 0 1px 0 rgba(255,255,255,0.9), 0 8px 24px rgba(0,0,0,0.1)'
                    }}
                    dangerouslySetInnerHTML={{ __html: rightTooltipHtml }}
                  />
                </div>
              </GlassSurface>
            </div>
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
  triggerActivity(type) {
    if (typeof window.__triggerNeverSleepActivity === 'function') {
      window.__triggerNeverSleepActivity(type);
    }
  },
  unmount() {
    if (currentRoot) {
      try { currentRoot.unmount(); } catch (_) {}
      currentRoot = null;
    }
  }
};
