// Provides window.api on top of Tauri commands and events
import { invoke } from '@tauri-apps/api/core';
import { listen, type EventCallback } from '@tauri-apps/api/event';
import type { AppAPI } from '@/types/api';

const isTauri = '__TAURI_INTERNALS__' in window;

// listen() resolves asynchronously, but callers expect a synchronous unsubscribe
const subscribe = <T>(event: string, callback: (data: T) => void) => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    const handler: EventCallback<T> = (e) => callback(e.payload);
    listen<T>(event, handler).then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
    });
    return () => {
        cancelled = true;
        unlisten?.();
    };
};

const tauriAPI: AppAPI = {
    // Cloudflared check
    checkCloudflared: () => invoke('cloudflared_check'),

    // Tunnel operations
    getTunnelStatus: () => invoke('tunnel_status'),
    listTunnels: () => invoke('tunnel_list'),
    startTunnel: (tunnelName) => invoke('tunnel_start', { tunnelName }),
    stopTunnel: () => invoke('tunnel_stop'),
    createTunnel: (name, makeActive, route) => invoke('tunnel_create', { name, makeActive, route }),
    updateTunnel: (tunnelId, options) => invoke('tunnel_update', { tunnelId, ...options }),
    deleteTunnel: (tunnelId, force = false) => invoke('tunnel_delete', { tunnelId, force }),

    // Auth operations
    login: () => invoke('auth_login'),
    checkAuth: () => invoke('auth_check'),
    logout: () => invoke('auth_logout'),

    // Config operations
    getConfig: () => invoke('config_get'),
    saveConfig: (data) => invoke('config_save', data),

    // Shell operations
    openExternal: (url) => invoke('shell_open_external', { url }),
    openPath: (filePath) => invoke('shell_open_path', { filePath }),

    // Network operations
    checkPort: (host, port) => invoke('network_check_port', { host, port }),
    checkPortLsof: (port) => invoke('network_check_port_lsof', { port }),

    // DNS operations
    checkDns: (hostname) => invoke('dns_check', { hostname }),

    // DNS route operations
    routeDns: (tunnelId, hostname, overwrite = false) =>
        invoke('tunnel_route_dns', { tunnelId, hostname, overwrite }),
    deleteDns: (hostname) => invoke('tunnel_delete_dns', { hostname }),

    // Restart with retry
    restartTunnel: (options) => invoke('tunnel_restart', { ...(options || {}) }),

    // Event listeners
    onTunnelLog: (callback) => subscribe('tunnel-log', callback),
    onTunnelExit: (callback) => subscribe('tunnel-exit', callback),
};

if (isTauri && !window.api) {
    window.api = tauriAPI;
}
