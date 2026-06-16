import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

const PROMETHEUS_METRICS = [
    { key: 'prompt_tokens_total',       label: 'Prompt Tokens',       type: 'counter',  unit: '' },
    { key: 'prompt_seconds_total',      label: 'Prompt Seconds',      type: 'counter',  unit: 's' },
    { key: 'tokens_predicted_total',    label: 'Predicted Tokens',    type: 'counter',  unit: '' },
    { key: 'tokens_predicted_seconds_total', label: 'Predict Seconds', type: 'counter', unit: 's' },
    { key: 'n_decode_total',            label: 'Decode Calls',        type: 'counter',  unit: '' },
    { key: 'n_tokens_max',              label: 'Max Tokens',          type: 'counter',  unit: '' },
    { key: 'prompt_tokens_seconds',     label: 'Prompt TPS',          type: 'gauge',    unit: 't/s' },
    { key: 'predicted_tokens_seconds',  label: 'Predict TPS',         type: 'gauge',    unit: 't/s' },
    { key: 'requests_processing',       label: 'Active Requests',     type: 'gauge',    unit: '' },
    { key: 'requests_deferred',         label: 'Deferred Requests',   type: 'gauge',    unit: '' },
    { key: 'n_busy_slots_per_decode',   label: 'Busy Slots/Decode',   type: 'gauge',    unit: '' },
];

function parsePrometheusMetrics(text) {
    const metrics = {};
    const lines = text.split('\n');
    for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed.startsWith('#') || trimmed === '') continue;
        const match = trimmed.match(/^(\S+)\s+(.+)$/);
        if (match) {
            const [, name, value] = match;
            const cleanName = name.replace('llamacpp:', '');
            metrics[cleanName] = parseFloat(value);
        }
    }
    console.log(`[llm-manager] parsePrometheusMetrics: matched ${Object.keys(metrics).length} metrics`);
    return metrics;
}

function formatNumber(value, decimals) {
    if (value === undefined || value === null || isNaN(value)) return 'N/A';
    return value.toFixed(decimals);
}

function formatCounter(value, unit) {
    if (value === undefined || value === null || isNaN(value)) return 'N/A';
    if (value >= 1e6) return (value / 1e6).toFixed(2) + 'M' + unit;
    if (value >= 1e3) return (value / 1e3).toFixed(1) + 'K' + unit;
    return Math.round(value).toString() + unit;
}

function getMetricColor(value, key) {
    if (key === 'requests_processing') {
        if (value >= 4) return 'llm-value-bad';
        if (value >= 2) return 'llm-value-warn';
        return 'llm-value-good';
    }
    if (key === 'n_busy_slots_per_decode') {
        if (value < 0.5) return 'llm-value-bad';
        if (value < 0.8) return 'llm-value-warn';
        return 'llm-value-good';
    }
    return '';
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

        this._currentMetrics = {};
        this._refreshTimeoutId = null;

        this._panelItem = new LlmPanelItem();
        this.add_child(this._panelItem);

        // Add metric items directly to this.menu
        this._metricLabels = {};
        this._metricContainers = {};
        this._history = {};
        for (const m of PROMETHEUS_METRICS) {
            this._history[m.key] = { value: 0, diff: 0 };
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
        this._addSettingChangedSignal('metrics-url', this._pollMetrics.bind(this));
        this._addSettingChangedSignal('update-time', this._updateTimeChanged.bind(this));
        this._addSettingChangedSignal('show-throughput', this._updatePanel.bind(this));
        this._addSettingChangedSignal('show-requests', this._updatePanel.bind(this));
        this._addSettingChangedSignal('precision', this._updatePanel.bind(this));
        this._addSettingChangedSignal('selected-metrics', () => {
            this._updatePanel();
            this._updateSelectionHighlights();
        });

        this._updateSelectionHighlights();
        this._initializeTimer();
        this._pollMetrics();
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

        const value = new St.Label({
            style_class: 'llm-metric-value',
            text: '---',
            x_align: Clutter.ActorAlign.END,
            x_expand: true,
        });

        container.add_child(icon);
        container.add_child(label);
        container.add_child(value);

        const item = new PopupMenu.PopupBaseMenuItem({ reactive: true });
        item.add_child(container);
        
        item.connect('activate', () => {
            this._selectMetric(metric.key);
        });

        this.menu.addMenuItem(item);
        this._metricLabels[metric.key] = value;
        this._metricContainers[metric.key] = { item, label, value };
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

    _updateSelectionHighlights() {
        const selectedKeys = this._settings.get_strv('selected-metrics') || [];
        for (const metric of PROMETHEUS_METRICS) {
            const containerInfo = this._metricContainers[metric.key];
            if (!containerInfo) continue;

            const isSelected = selectedKeys.includes(metric.key);
            if (isSelected) {
                containerInfo.label.add_style_class_name('llm-metric-selected');
                containerInfo.value.add_style_class_name('llm-metric-selected');
            } else {
                containerInfo.label.remove_style_class_name('llm-metric-selected');
                containerInfo.value.remove_style_class_name('llm-metric-selected');
            }
        }
    }

    _updateMetricsValues() {
        const selectedKeys = this._settings.get_strv('selected-metrics') || [];
        for (const metric of PROMETHEUS_METRICS) {
            const label = this._metricLabels[metric.key];
            if (!label) continue;

            const rawValue = this._currentMetrics[metric.key];
            const history = this._history[metric.key];
            history.diff = rawValue !== undefined ? rawValue - history.value : 0;
            history.value = rawValue ?? 0;

            let displayValue;
            if (rawValue === undefined || rawValue === null || isNaN(rawValue)) {
                displayValue = 'N/A';
            } else if (metric.type === 'counter') {
                displayValue = formatCounter(rawValue, metric.unit);
            } else {
                displayValue = formatNumber(rawValue, 1) + (metric.unit ? ' ' + metric.unit : '');
            }

            const colorClass = getMetricColor(rawValue, metric.key);
            const isSelected = selectedKeys.includes(metric.key);
            
            let classes = ['llm-metric-value'];
            if (colorClass) {
                classes.push(colorClass);
            }
            if (isSelected) {
                classes.push('llm-metric-selected');
            }
            
            label.style_class = classes.join(' ');
            label.text = displayValue;
        }
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
                this._pollMetrics();
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

    _pollMetrics() {
        const url = this._settings.get_string('metrics-url');
        console.log(`[llm-manager] Fetching metrics from: ${url}`);
        const subprocess = Gio.Subprocess.new(
            ['curl', '-s', '--max-time', '5', url],
            Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE
        );
        subprocess.communicate_utf8_async(null, null, (source, result) => {
            try {
                const [success, stdout, stderr] = source.communicate_utf8_finish(result);
                if (success && source.get_successful()) {
                    console.log(`[llm-manager] Raw response (${stdout ? stdout.length : 0} bytes):`);
                    console.log(stdout);
                    this._currentMetrics = parsePrometheusMetrics(stdout || '');
                    console.log(`[llm-manager] Parsed metrics:`, this._currentMetrics);
                    this._updatePanel();
                    this._updateMetricsValues();
                } else {
                    const errStr = stderr ? stderr : '';
                    const exit_status = source.get_exit_status();
                    console.log(`[llm-manager] Fetch failed, exit_status: ${exit_status}, stderr: ${errStr.trim()}`);
                }
            } catch (e) {
                global.logError(e);
            }
        });
    }

    _updatePanel() {
        const selectedKeys = this._settings.get_strv('selected-metrics') || [];
        const precision = this._settings.get_int('precision');

        console.log(`[llm-manager] _updatePanel: selectedKeys=${JSON.stringify(selectedKeys)}, metrics=${JSON.stringify(this._currentMetrics)}`);
        
        if (selectedKeys.length > 0) {
            let parts = [];
            for (const key of selectedKeys) {
                const metric = PROMETHEUS_METRICS.find(m => m.key === key);
                if (metric) {
                    const value = this._currentMetrics[key];
                    let displayValue;
                    if (value === undefined || value === null || isNaN(value)) {
                        displayValue = '--';
                    } else if (metric.type === 'counter') {
                        displayValue = formatCounter(value, metric.unit);
                    } else {
                        displayValue = formatNumber(value, precision) + (metric.unit ? ' ' + metric.unit : '');
                    }
                    parts.push(`${metric.label}: ${displayValue}`);
                }
            }
            if (parts.length > 0) {
                this._panelItem.setLabel(parts.join(' | '));
                this._panelItem.visible = true;
                return;
            }
        }

        const showThroughput = this._settings.get_boolean('show-throughput');
        const showRequests = this._settings.get_boolean('show-requests');

        let parts = [];

        if (showRequests && this._currentMetrics['requests_processing'] !== undefined) {
            const req = this._currentMetrics['requests_processing'];
            parts.push(`${req}r`);
        }

        if (showThroughput) {
            if (this._currentMetrics['prompt_tokens_seconds'] !== undefined) {
                parts.push(`${formatNumber(this._currentMetrics['prompt_tokens_seconds'], precision)}p`);
            }
            if (this._currentMetrics['predicted_tokens_seconds'] !== undefined) {
                parts.push(`${formatNumber(this._currentMetrics['predicted_tokens_seconds'], precision)}d`);
            }
        }

        if (parts.length > 0) {
            this._panelItem.setLabel(parts.join(' '));
            this._panelItem.visible = true;
        } else {
            this._panelItem.visible = false;
        }
    }

    destroy() {
        this._destroyTimer();
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
