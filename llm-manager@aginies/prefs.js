import Gtk from 'gi://Gtk';
import GObject from 'gi://GObject';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

const WS_METRICS = [
    { key: 'model_name', label: 'Model' },
    { key: 'state', label: 'State' },
    { key: 'tps', label: 'TPS' },
    { key: 'prompt_tps', label: 'Prompt TPS' },
    { key: 'gen_tps', label: 'Gen TPS' },
    { key: 'ctx', label: 'Context' },
    { key: 'vram', label: 'VRAM' },
    { key: 'ram', label: 'RAM' },
    { key: 'cpu', label: 'CPU' },
    { key: 'decoded_tokens', label: 'Decoded Tokens' },
    { key: 'prompt_tokens', label: 'Prompt Tokens' },
];

export default class LlmManagerPreferences extends ExtensionPreferences {
    getPreferencesWidget() {
        const frame = new Gtk.Frame({
            label: _('LLM Manager Settings'),
            visible: true,
        });

        const grid = new Gtk.Grid({
            column_spacing: 12,
            row_spacing: 12,
            visible: true,
            margin_top: 12,
            margin_bottom: 12,
            margin_start: 12,
            margin_end: 12,
        });

        const schema = 'org.gnome.shell.extensions.llm-manager';
        let row = 0;

        // metrics-url
        const urlLabel = new Gtk.Label({
            label: _('Metrics URL'),
            xalign: 0,
            visible: true,
        });
        grid.attach(urlLabel, 0, row, 1, 1);
        row++;

        const urlBox = new Gtk.Box({
            orientation: Gtk.Orientation.VERTICAL,
            spacing: 4,
            visible: true,
        });

        const entryBox = new Gtk.Box({
            orientation: Gtk.Orientation.HORIZONTAL,
            spacing: 6,
            visible: true,
        });

        const urlEntry = new Gtk.Entry({
            visible: true,
            primary_icon_name: 'network-server-symbolic',
            hexpand: true,
        });
        urlEntry.set_width_chars(32);
        entryBox.append(urlEntry);

        const testBtn = new Gtk.Button({
            label: _('Test'),
            visible: true,
        });
        entryBox.append(testBtn);

        urlBox.append(entryBox);

        const testStatusLabel = new Gtk.Label({
            use_markup: true,
            xalign: 0,
            visible: false,
        });
        urlBox.append(testStatusLabel);

        grid.attach(urlBox, 1, row, 1, 1);
        row++;

        this.getSettings().bind(
            'metrics-url', urlEntry, 'text',
            Gio.SettingsBindFlags.DEFAULT
        );

        testBtn.connect('clicked', () => {
            const url = urlEntry.text;
            testStatusLabel.set_markup(`<span color="#3584e4">${_('Testing WebSocket connection...')}</span>`);
            testStatusLabel.set_visible(true);
            testBtn.set_sensitive(false);

            const subprocess = Gio.Subprocess.new(
                ['curl', '-s', '-I', '-k', '--max-time', '5', url],
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE
            );

            subprocess.communicate_utf8_async(null, null, (source, result) => {
                testBtn.set_sensitive(true);
                try {
                    const [success, stdout, stderr] = source.communicate_utf8_finish(result);
                    if (success && source.get_successful()) {
                        const hasHeaders = stdout.includes('HTTP') || stdout.includes('Switching');
                        const hasMetrics = (stdout || '').includes('llamacpp:') || (stdout || '').includes('# HELP');
                        if (hasHeaders || hasMetrics) {
                            testStatusLabel.set_markup(`<span color="#2ec27e"><b>${_('Success: Server reachable!')}</b></span>`);
                        } else {
                            testStatusLabel.set_markup(`<span color="#e01b24"><b>${_('Warning: Server responds but not LLM server')}</b></span>`);
                        }
                    } else {
                        const errStr = stderr ? stderr.trim() : `${_('Exit status')} ${source.get_exit_status()}`;
                        testStatusLabel.set_markup(`<span color="#e01b24"><b>${_('Connection failed:')}</b> ${errStr}</span>`);
                    }
                } catch (e) {
                    testStatusLabel.set_markup(`<span color="#e01b24"><b>${_('Error:')}</b> ${e.message}</span>`);
                }
            });
        });

        // update-time
        const updateLabel = new Gtk.Label({
            label: _('Reconnect Interval (seconds)'),
            xalign: 0,
            visible: true,
        });
        grid.attach(updateLabel, 0, row, 1, 1);
        row++;

        const updateSpin = new Gtk.SpinButton({
            visible: true,
            adjustment: new Gtk.Adjustment({
                lower: 1,
                upper: 60,
                step_increment: 1,
                page_increment: 5,
            }),
        });
        updateSpin.value = this.getSettings().get_int('update-time');
        grid.attach(updateSpin, 1, row, 1, 1);
        row++;

        this.getSettings().bind(
            'update-time', updateSpin, 'value',
            Gio.SettingsBindFlags.DEFAULT
        );

        // ws-auth-enabled
        const authCheck = new Gtk.CheckButton({
            label: _('Enable WebSocket auth from URL'),
            visible: true,
        });
        authCheck.active = this.getSettings().get_boolean('ws-auth-enabled');
        grid.attach(authCheck, 0, row, 2, 1);
        row++;

        this.getSettings().bind(
            'ws-auth-enabled', authCheck, 'active',
            Gio.SettingsBindFlags.DEFAULT
        );

        // position
        const positionLabel = new Gtk.Label({
            label: _('Panel Position'),
            xalign: 0,
            visible: true,
        });
        grid.attach(positionLabel, 0, row, 1, 1);
        row++;

        const positionCombo = new Gtk.ComboBoxText({
            visible: true,
        });
        positionCombo.append('0', _('Left'));
        positionCombo.append('1', _('Center'));
        positionCombo.append('2', _('Right'));
        positionCombo.append('3', _('Far Left'));
        positionCombo.append('4', _('Far Right'));
        positionCombo.active = this.getSettings().get_int('position-in-panel');
        grid.attach(positionCombo, 1, row, 1, 1);
        row++;

        this.getSettings().bind(
            'position-in-panel', positionCombo, 'active',
            Gio.SettingsBindFlags.DEFAULT
        );

        // Metrics selection
        const metricsLabel = new Gtk.Label({
            label: _('Metrics to Display'),
            xalign: 0,
            visible: true,
        });
        grid.attach(metricsLabel, 0, row, 1, 1);
        row++;

        const metricsFrame = new Gtk.Frame({
            label: _('Selected Metrics'),
            visible: true,
            margin_start: 6,
            margin_end: 6,
            margin_top: 6,
            margin_bottom: 6,
        });

        const metricsGrid = new Gtk.Grid({
            column_spacing: 12,
            row_spacing: 6,
            visible: true,
            margin_start: 12,
            margin_end: 12,
            margin_top: 12,
            margin_bottom: 12,
        });

        const checkButtons = [];
        const selectedKeys = this.getSettings().get_strv('selected-metrics') || [];

        let col = 0;
        let r = 0;
        for (const m of WS_METRICS) {
            const check = new Gtk.CheckButton({
                label: m.label,
                active: selectedKeys.includes(m.key),
                visible: true,
            });
            checkButtons.push({ check, key: m.key });
            
            metricsGrid.attach(check, col, r, 1, 1);
            col++;
            if (col >= 3) {
                col = 0;
                r++;
            }
        }

        metricsFrame.set_child(metricsGrid);
        grid.attach(metricsFrame, 0, row, 2, 1);
        row++;

        for (const { check, key } of checkButtons) {
            check.connect('toggled', () => {
                let keys = this.getSettings().get_strv('selected-metrics') || [];
                if (check.active) {
                    if (!keys.includes(key)) {
                        keys.push(key);
                    }
                } else {
                    keys = keys.filter(k => k !== key);
                }
                this.getSettings().set_strv('selected-metrics', keys);
            });
        }

        // debug log
        this._setupDebugLog(grid, row);

        frame.set_child(grid);
        return frame;
    }

    _setupDebugLog(grid, startRow) {
        const logLabel = new Gtk.Label({
            label: _('Debug Log'),
            xalign: 0,
            visible: true,
        });
        grid.attach(logLabel, 0, startRow, 1, 1);

        const clearBtn = new Gtk.Button({
            label: _('Clear'),
            visible: true,
        });
        clearBtn.connect('clicked', () => {
            this._debugLogBuffer.set_text('', -1);
        });
        grid.attach(clearBtn, 1, startRow, 1, 1);

        const logView = new Gtk.TextView({
            editable: false,
            cursor_visible: false,
            monospace: true,
            wrap_mode: Gtk.WrapMode.WORD,
            visible: true,
        });
        logView.get_style_context().add_class('debug-log');
        this._debugLogBuffer = logView.get_buffer();
        this._debugLogView = logView;

        const scrolled = new Gtk.ScrolledWindow({
            hscrollbar_policy: Gtk.PolicyType.NEVER,
            vscrollbar_policy: Gtk.PolicyType.AUTOMATIC,
            visible: true,
            height_request: 150,
        });
        scrolled.set_child(logView);
        grid.attach(scrolled, 0, startRow + 1, 2, 1);

        this._startDebugLogPolling();
    }

    _startDebugLogPolling() {
        this._debugLogPolling = true;

        this._debugLogPollId = GLib.timeout_add_seconds(
            GLib.PRIORITY_DEFAULT,
            2,
            () => {
                if (!this._debugLogPolling) return GLib.SOURCE_REMOVE;

                let [success, stdout, stderr, exit_status] = [false, null, null, 0];
                try {
                    [success, stdout, stderr, exit_status] = GLib.spawn_command_line_sync(
                        'journalctl -n 1000 -g "\\[llm-manager\\]" 2>/dev/null || true'
                    );
                } catch (e) {
                    success = false;
                }

                let text = '';
                if (success && stdout) {
                    try {
                        const decoded = new TextDecoder().decode(stdout);
                        const lines = decoded.split('\n');
                        const last1000 = lines.slice(-1000);
                        text = last1000.join('\n');
                    } catch (e) {
                        text = `(error decoding logs: ${e.message})`;
                    }
                } else {
                    text = '(error reading logs)';
                }

                this._debugLogBuffer.set_text(text, -1);

                GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                    const vadj = this._debugLogView.get_vadjustment();
                    vadj.set_value(vadj.get_upper() - vadj.get_page_size());
                    return GLib.SOURCE_REMOVE;
                });

                return GLib.SOURCE_CONTINUE;
            }
        );
    }

    _destroyDebugLogPolling() {
        this._debugLogPolling = false;
        if (this._debugLogPollId) {
            GLib.Source.remove(this._debugLogPollId);
            this._debugLogPollId = null;
        }
    }
}
