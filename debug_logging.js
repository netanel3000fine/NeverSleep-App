(function () {
    const api = window.__TAURI__;
    if (!api || typeof api.invoke !== 'function' || api.__neverSleepDebugWrapped) return;

    const invokeOriginal = api.invoke.bind(api);
    const startedAt = Date.now();
    const quietSuccessCommands = new Set([
        'get_last_activity', 'record_input_activity', 'is_media_playing', 'is_workstation_locked',
        'get_last_unlock_time', 'is_main_visible', 'check_autostart', 'is_classic_only',
        'get_screen_brightness', 'load_settings', 'save_settings', 'set_autostart',
        'set_window_size', 'resize_tray_window', 'update_tray_menu_state', 'set_border_color',
        'set_window_effect', 'set_window_height', 'set_decorations'
    ]);
    const safeJson = value => {
        try { return JSON.stringify(value); }
        catch (_) { return String(value); }
    };
    const setStatus = message => {
        document.querySelectorAll('[data-debug-log-status]').forEach(element => {
            element.textContent = message;
            element.style.color = /^Error:|^Could not/i.test(message) ? '#fca5a5' : '';
        });
    };
    const record = message => {
        invokeOriginal('log_to_file', { msg: message }).catch(error => {
            setStatus(`Error: Could not write debug log: ${error}`);
        });
    };

    api.invoke = function (command, args) {
        if (command === 'log_to_file') return invokeOriginal(command, args);
        const began = Date.now();
        return invokeOriginal(command, args).then(result => {
            if (!quietSuccessCommands.has(command)) {
                record(`Backend command succeeded: ${command}; duration=${Date.now() - began}ms; args=${safeJson(args)}; result=${safeJson(result)}`);
            }
            return result;
        }, error => {
            record(`Backend command failed: ${command}; duration=${Date.now() - began}ms; args=${safeJson(args)}; error=${String(error)}`);
            throw error;
        });
    };
    if (api.tauri && typeof api.tauri.invoke === 'function') {
        api.tauri.invoke = api.invoke;
    }
    api.__neverSleepDebugWrapped = true;
    window.__neverSleepDebugLog = record;

    if (api.event && typeof api.event.listen === 'function') {
        api.event.listen('debug-log-status', event => setStatus(String(event.payload || ''))).catch(() => {});
        const loadStatus = () => {
            if (document.querySelector('[data-debug-log-status]')) {
                invokeOriginal('get_debug_log_status').then(setStatus).catch(error => setStatus(`Error: ${error}`));
            }
        };
        if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', loadStatus, { once: true });
        else loadStatus();
    }

    window.addEventListener('error', event => {
        record(`Frontend error: ${event.message}; source=${event.filename}:${event.lineno}:${event.colno}`);
    });
    window.addEventListener('unhandledrejection', event => {
        record(`Unhandled frontend rejection: ${String(event.reason)}`);
    });
    record(`Frontend initialized: ${location.pathname}; initDelay=${Date.now() - startedAt}ms`);
})();
