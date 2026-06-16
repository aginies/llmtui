import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';
import Soup from 'gi://Soup?version=3.0';

import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

const WS_METRICS = [
    { key: 'model_name', label: 'Model', type: 'text' },
    { key: 'state', label: 'State', type: 'badge' },
    { key: 'tps', label: 'TPS', type: 'number', unit: 't/s' },
    { key: 'prompt_tps', label: 'Prompt TPS', type: 'number', unit: 't/s' },
    { key: 'gen_tps', label: 'Gen TPS', type: 'number', unit: 't/s' },
    { key: 'ctx', label: 'Ctx', type: 'ratio', used: 'ctx_used', max: 'ctx_max', unit: 'tokens' },
    { key: 'vram', label: 'VRAM', type: 'ratio_gb', used: 'gpu_mem_used', total: 'gpu_mem_total' },
    { key: 'ram', label: 'RAM', type: 'gb', field: 'ram_used' },
    { key: 'cpu', label: 'CPU', type: 'percent', field: 'cpu_usage' },
    { key: 'decoded_tokens', label: 'Decoded', type: 'number' },
];

      function buildWsUrl(metricsUrl, secret) {
    try {
        const match = metricsUrl.match(/^(https?:)\/\/([^\/?#]+)([^?#]*)(?:\?([^#]*))?/);
        if (!match) {
            throw new Error('Invalid URL format');
        }
        const protocol = match[1];
        const host = match[2];
        const query = match[4] || '';

        const wsProtocol = protocol === 'https:' ? 'wss:' : 'ws:';
        let auth = null;
        if (secret) {
            auth = secret;
        } else if (query) {
            const params = query.split('&');
            for (const param of params) {
                const [key, value] = param.split('=');
                if (key === 'auth') {
                    auth = decodeURIComponent(value);
                    break;
                }
            }
        }
        return {
            wsUrl: `${wsProtocol}//${host}/ws${auth ? '?auth=' + encodeURIComponent(auth) : ''}`,
            hasAuth: !!auth,
        };
    } catch (e) {
        return { wsUrl: 'ws://127.0.0.1:8080/ws', hasAuth: false };
    }
}

function formatNumber(value, decimals) {
    if (value === undefined || value === null || isNaN(value)) return 'N/A';
    return value.toFixed(decimals);
}

function formatGB(bytes) {
    if (bytes === undefined || bytes === null || isNaN(bytes)) return 'N/A';
    return (bytes / 1024 / 1024 / 1024).toFixed(1) + ' GB';
}

function formatBytes(bytes) {
    if (bytes === undefined || bytes === null || isNaN(bytes)) return 'N/A';
    if (bytes >= 1e9) return (bytes / 1e9).toFixed(1) + ' GB';
    if (bytes >= 1e6) return (bytes / 1e6).toFixed(1) + ' MB';
    return Math.round(bytes) + ' B';
}

function formatTokens(tokens) {
    if (tokens === undefined || tokens === null || isNaN(tokens)) return 'N/A';
    if (tokens >= 1024) return Math.floor(tokens / 1024) + 'K';
    return Math.round(tokens).toString();
}

function getVramColor(percent) {
    if (percent > 80) return 'llm-value-bad';
    if (percent > 50) return 'llm-value-warn';
    return 'llm-value-good';
}

function truncateModelName(name, maxLen) {
    if (!name) return 'No model';
    if (name.length <= maxLen) return name;
    const ext = name.endsWith('.gguf') ? 5 : 4;
    const suffix = name.endsWith('.gguf') ? '.gguf' : '';
    const available = maxLen - ext;
    if (available <= 0) return name.substring(0, maxLen);
    return name.substring(0, available) + '...' + suffix;
}

var LlmPanelItem = GObject.registerClass({
    GTypeName: 'LlmPanelItem',
}, class LlmPanelItem extends St.BoxLayout {
    _init() {
        super._init({
            style_class: 'llm-panel-item',
            orientation: Clutter.Orientation.HORIZONTAL,
        });
        this._icon = new St.Icon({
            style_class: 'llm-icon',
            icon_name: 'applications-science-symbolic',
            y_align: Clutter.ActorAlign.CENTER,
            y_expand: true,
        });
        this._label = new St.Label({
            style_class: 'llm-panel-label',
            text: '--',
            y_align: Clutter.ActorAlign.CENTER,
            y_expand: true,
        });
        this.add_child(this._icon);
        this.add_child(this._label);
    }

    setLabel(text) {
        this._label.text = text;
    }
});

var LlmManagerButton = GObject.registerClass({
    GTypeName: 'LlmManagerButton',
}, class LlmManagerButton extends PanelMenu.Button {
    _init(extensionObject) {
        super._init(Clutter.ActorAlign.FILL);

        this._extensionObject = extensionObject;
        this._settings = extensionObject.getSettings();
        this._settings.connect('changed::selected-metrics', () => this._updatePanel());

        this._currentMetrics = {};
        this._ws = null;
        this._soupSession = null;
        this._isConnecting = false;
        this._refreshTimeoutId = null;
        this._reconnectTimerId = null;

        this._panelItem = new LlmPanelItem();
        this.add_child(this._panelItem);

        this._metricLabels = {};
        this._metricContainers = {};
        this._metricBars = {};
        for (const m of WS_METRICS) {
            this._addMetricItem(m);
        }

        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        const settingsItem = new PopupMenu.PopupMenuItem(_('Settings'));
        settingsItem.connect('activate', () => {
            this._extensionObject.openPreferences();
        });
        this.menu.addMenuItem(settingsItem);

        this._settingsConnections = [];
        this._addSettingChangedSignal('position-in-panel', this._positionInPanelChanged.bind(this));
        this._addSettingChangedSignal('metrics-url', () => {
            this._connectWebSocket(true);
        });
        this._addSettingChangedSignal('update-time', this._updateTimeChanged.bind(this));
        this._addSettingChangedSignal('position-in-panel', this._positionInPanelChanged.bind(this));

        this._initializeTimer();
        this._connectWebSocket();
    }

    _addMetricItem(metric) {
        const container = new St.BoxLayout({
            style_class: 'llm-menu-item',
            orientation: Clutter.Orientation.HORIZONTAL,
        });

        const icon = new St.Icon({
            style_class: 'popup-menu-icon',
            icon_name: 'applications-science-symbolic',
        });

        const label = new St.Label({
            style_class: 'llm-metric-label',
            text: metric.label,
        });

        const valueBox = new St.BoxLayout({
            orientation: Clutter.Orientation.HORIZONTAL,
            x_expand: true,
            x_align: Clutter.ActorAlign.END,
        });

        const value = new St.Label({
            style_class: 'llm-metric-value',
            text: '---',
            y_align: Clutter.ActorAlign.CENTER,
        });

        const barContainer = new St.BoxLayout({
            style_class: 'llm-bar-container',
            visible: false,
            x_expand: true,
            y_align: Clutter.ActorAlign.END,
            height: 8,
        });

        const bar = new St.Bin({
            style_class: 'llm-bar',
            x_expand: true,
            height: 8,
        });

        const barInner = new St.Bin({
            style_class: 'llm-bar-inner',
            width: 0,
            height: 8,
        });

        bar.set_child(barInner);
        barContainer.add_child(bar);
        valueBox.add_child(value);

        container.add_child(icon);
        container.add_child(label);
        container.add_child(valueBox);
        container.add_child(barContainer);

        const item = new PopupMenu.PopupBaseMenuItem({ reactive: true });
        item.add_child(container);
        
        item.connect('activate', () => {
            this._selectMetric(metric.key);
        });

        this.menu.addMenuItem(item);
        this._metricLabels[metric.key] = value;
        this._metricContainers[metric.key] = { item, label, value };
        this._metricBars[metric.key] = { barContainer, bar, barInner };
    }

    _selectMetric(key) {
        let selectedKeys = this._settings.get_strv('selected-metrics') || [];
        if (selectedKeys.includes(key)) {
            selectedKeys = selectedKeys.filter(k => k !== key);
        } else {
            selectedKeys.push(key);
        }
        this._settings.set_strv('selected-metrics', selectedKeys);
    }

    _updatePanel() {
        const selectedKeys = this._settings.get_strv('selected-metrics') || [];

        if (selectedKeys.length > 0) {
            let parts = [];
            for (const key of selectedKeys) {
                const metric = WS_METRICS.find(m => m.key === key);
                if (!metric) continue;

                const displayValue = this._formatMetricValue(metric, this._currentMetrics);
                if (displayValue && displayValue !== 'N/A' && displayValue !== '--') {
                    parts.push(metric.label + ': ' + displayValue);
                }
            }
            if (parts.length > 0) {
                this._panelItem.setLabel(parts.join('  '));
                this._panelItem.visible = true;
                return;
            }
        }

        this._panelItem.visible = true;
        this._panelItem._label.text = '';
    }

    _formatMetricValue(metric, metrics) {
        switch (metric.type) {
            case 'text':
                let textVal = metrics[metric.key];
                if (textVal && metric.key === 'model_name') {
                    textVal = textVal.split('/').pop();
                }
                return textVal || '-';
            case 'badge':
                const loaded = metrics.loaded;
                return loaded ? 'Loaded' : 'Unloaded';
            case 'number':
                const numVal = metrics[metric.key];
                if (numVal === undefined || numVal === null || isNaN(numVal)) return 'N/A';
                return formatNumber(numVal, 1) + (metric.unit ? ' ' + metric.unit : '');
            case 'ratio':
                const used = metrics[metric.used];
                const max = metrics[metric.max];
                if (used === undefined || max === undefined) return 'N/A';
                if (metric.unit === 'tokens') return `${formatTokens(used)} / ${formatTokens(max)}`;
                return `${used} / ${max}`;
            case 'ratio_gb':
                const usedGb = metrics[metric.used];
                const totalGb = metrics[metric.total];
                if (usedGb === undefined || totalGb === undefined) return 'N/A';
                return `${formatGB(usedGb)} / ${formatGB(totalGb)}`;
            case 'gb':
                const gbKey = metric.field || metric.key || metric.used;
                const gbVal = metrics[gbKey];
                if (gbVal === undefined || gbVal === null) return 'N/A';
                return formatGB(gbVal);
            case 'percent':
                const pctKey = metric.field || metric.key || metric.used;
                const pctVal = metrics[pctKey];
                if (pctVal === undefined || pctVal === null || isNaN(pctVal)) return 'N/A';
                return Math.round(pctVal) + '%';
            default:
                return '-';
        }
    }

    _updateMetricsValues() {
        const selectedKeys = this._settings.get_strv('selected-metrics') || [];
        
        for (const metric of WS_METRICS) {
            const label = this._metricLabels[metric.key];
            if (!label) continue;

            const isSelected = selectedKeys.includes(metric.key);
            const barInfo = this._metricBars[metric.key];

            if (metric.type === 'ratio' || metric.type === 'ratio_gb') {
                let percent = 0;
                if (metric.type === 'ratio') {
                    const used = this._currentMetrics[metric.used];
                    const max = this._currentMetrics[metric.max];
                    if (max && max > 0) {
                        percent = (used / max) * 100;
                    }
                } else if (metric.type === 'ratio_gb') {
                    const used = this._currentMetrics[metric.used];
                    const total = this._currentMetrics[metric.total];
                    if (total && total > 0) {
                        percent = (used / total) * 100;
                    }
                }

                if (barInfo && barInfo.barContainer) {
                    barInfo.barContainer.visible = true;
                    barInfo.barInner.width = percent * barInfo.bar.width / 100;
                    
                    let colorClass = 'llm-bar-green';
                    if (percent > 80) {
                        colorClass = 'llm-bar-red';
                    } else if (percent > 50) {
                        colorClass = 'llm-bar-yellow';
                    }
                    barInfo.barInner.style_class = colorClass;
                }

                const displayValue = this._formatMetricValue(metric, this._currentMetrics);
                label.text = displayValue;
                label.style_class = isSelected ? 'llm-metric-value llm-metric-selected' : 'llm-metric-value';
            } else if (metric.type === 'gb' || metric.type === 'percent') {
                if (barInfo && barInfo.barContainer) {
                    barInfo.barContainer.visible = false;
                }
                const displayValue = this._formatMetricValue(metric, this._currentMetrics);
                label.text = displayValue;
                label.style_class = isSelected ? 'llm-metric-value llm-metric-selected' : 'llm-metric-value';
            } else {
                if (barInfo && barInfo.barContainer) {
                    barInfo.barContainer.visible = false;
                }
                const displayValue = this._formatMetricValue(metric, this._currentMetrics);
                label.text = displayValue;
                label.style_class = isSelected ? 'llm-metric-value llm-metric-selected' : 'llm-metric-value';
            }
        }

        this._updatePanel();
    }

    _connectWebSocket(force = false) {
        if (!force) {
            if (this._isConnecting) {
                return;
            }
            if (this._ws && this._ws.state === Soup.WebsocketState.OPEN) {
                return;
            }
        }

        this._isConnecting = true;

        if (this._ws) {
            try {
                this._ws.close(Soup.WebsocketCloseCode.NORMAL, 'Reconnecting');
            } catch (e) {}
            this._ws = null;
        }

        if (this._soupSession) {
            try {
                this._soupSession.abort();
            } catch (e) {}
            this._soupSession = null;
        }

        const metricsUrl = this._settings.get_string('metrics-url');
        const metricsSecret = this._settings.get_string('metrics-secret');
        const { wsUrl, hasAuth } = buildWsUrl(metricsUrl, metricsSecret);

        console.log(`[llm-manager] Connecting to WebSocket: ${wsUrl}`);

        try {
            this._soupSession = new Soup.Session();
            const message = Soup.Message.new('GET', wsUrl);

            // Bypass SSL/TLS self-signed certificate validation errors
            message.connect('accept-certificate', (msg, cert, errors) => {
                return true;
            });

            this._soupSession.websocket_connect_async(
                message,
                null, // origin
                null, // protocols
                GLib.PRIORITY_DEFAULT, // io_priority
                null, // cancellable
                (session, result) => {
                    try {
                        this._ws = session.websocket_connect_finish(result);
                        this._isConnecting = false;
                        console.log('[llm-manager] WebSocket connected');
                        
                        if (this._reconnectTimerId) {
                            GLib.Source.remove(this._reconnectTimerId);
                            this._reconnectTimerId = null;
                        }

                        this._ws.connect('message', (connection, type, data) => {
                            if (type === Soup.WebsocketDataType.TEXT) {
                                try {
                                    const decoder = new TextDecoder('utf-8');
                                    const text = decoder.decode(data.get_data());
                                    const m = JSON.parse(text);
                                    this._currentMetrics = m;
                                    this._updateMetricsValues();
                                } catch (e) {
                                    console.log(`[llm-manager] Failed to parse WebSocket message: ${e}`);
                                }
                            }
                        });

                        this._ws.connect('closed', () => {
                            console.log('[llm-manager] WebSocket disconnected');
                            this._ws = null;
                            this._scheduleReconnect();
                        });

                        this._ws.connect('error', (connection, error) => {
                            console.log(`[llm-manager] WebSocket connection error: ${error}`);
                        });

                    } catch (e) {
                        this._isConnecting = false;
                        console.log(`[llm-manager] Failed to complete WebSocket handshake: ${e}`);
                        this._scheduleReconnect();
                    }
                }
            );
        } catch (e) {
            this._isConnecting = false;
            console.log(`[llm-manager] Failed to create WebSocket session: ${e}`);
            this._scheduleReconnect();
        }
    }

    _scheduleReconnect() {
        if (this._reconnectTimerId) {
            return;
        }
        let update_time = this._settings.get_int('update-time');
        if (update_time < 1) {
            update_time = 2;
        }
        this._reconnectTimerId = GLib.timeout_add_seconds(
            GLib.PRIORITY_DEFAULT,
            update_time,
            () => {
                this._reconnectTimerId = null;
                this._connectWebSocket();
                return GLib.SOURCE_REMOVE;
            }
        );
    }

    _addToPanel() {
        this._positionInPanel();
    }

    _addSettingChangedSignal(key, callback) {
        const id = this._settings.connect('changed::' + key, callback);
        this._settingsConnections.push({ id });
    }

    _updateTimeChanged() {
        this._destroyTimer();
        this._initializeTimer();
    }

    _positionInPanel() {
        this.container.get_parent().remove_child(this.container);
        let position = this._settings.get_int('position-in-panel');
        let boxes = {
            0: Main.panel._leftBox,
            1: Main.panel._centerBox,
            2: Main.panel._rightBox,
            3: Main.panel._leftBox,
            4: Main.panel._rightBox,
        };
        let gravity = (position === 0 || position === 3) ? -1 : (position === 4 ? -1 : 0);
        let arrow_pos = (position === 0 || position === 3) ? 1 : (position === 4 ? 0 : 0.5);
        this.menu._arrowAlignment = arrow_pos;
        boxes[position].insert_child_at_index(this.container, gravity);
    }

    _positionInPanelChanged() {
        this._positionInPanel();
    }

    _initializeTimer() {
        let update_time = this._settings.get_int('update-time');
        this._refreshTimeoutId = GLib.timeout_add_seconds(
            GLib.PRIORITY_DEFAULT,
            update_time,
            () => {
                this._connectWebSocket();
                return GLib.SOURCE_CONTINUE;
            }
        );
    }

    _destroyTimer() {
        if (this._refreshTimeoutId !== null) {
            GLib.Source.remove(this._refreshTimeoutId);
            this._refreshTimeoutId = null;
        }
    }

    destroy() {
        this._destroyTimer();
        if (this._reconnectTimerId) {
            GLib.Source.remove(this._reconnectTimerId);
            this._reconnectTimerId = null;
        }
        if (this._ws) {
            try {
                this._ws.close(Soup.WebsocketCloseCode.NORMAL, 'Extension destroyed');
            } catch (e) {}
            this._ws = null;
        }
        if (this._soupSession) {
            try {
                this._soupSession.abort();
            } catch (e) {}
            this._soupSession = null;
        }
        if (this._settingsConnections) {
            for (const conn of this._settingsConnections) {
                this._settings.disconnect(conn.id);
            }
            this._settingsConnections = [];
        }
        super.destroy();
    }
});

export default class LlmManagerExtension extends Extension {
    enable() {
        this._settings = this.getSettings();
        this._button = new LlmManagerButton(this);
        let position = this._settings.get_int('position-in-panel');
        let gravity = (position === 0 || position === 3) ? -1 : (position === 4 ? -1 : 0);
        Main.panel.addToStatusArea('llm-manager', this._button, gravity, null);
        this._button._addToPanel();
    }

    disable() {
        if (this._button) {
            this._button.destroy();
            this._button = null;
        }
    }
}
