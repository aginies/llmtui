import Gtk from 'gi://Gtk';
import GObject from 'gi://GObject';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

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

        // metrics-url
        const urlLabel = new Gtk.Label({
            label: _('Metrics URL'),
            xalign: 0,
            visible: true,
        });
        grid.attach(urlLabel, 0, 0, 1, 1);

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
            text: this.getSettings().get_string('metrics-url'),
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

        grid.attach(urlBox, 1, 0, 1, 1);

        this.getSettings().bind(
            'metrics-url', urlEntry, 'text',
            Gio.SettingsBindFlags.DEFAULT
        );

        testBtn.connect('clicked', () => {
            const url = urlEntry.text;
            testStatusLabel.set_markup(`<span color="#3584e4">${_('Testing connection...')}</span>`);
            testStatusLabel.set_visible(true);
            testBtn.set_sensitive(false);

            const subprocess = Gio.Subprocess.new(
                ['curl', '-i', '-s', '--max-time', '5', url],
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE
            );

            subprocess.communicate_utf8_async(null, null, (source, result) => {
                testBtn.set_sensitive(true);
                try {
                    const [success, stdout, stderr] = source.communicate_utf8_finish(result);
                    if (success && source.get_successful()) {
                        const lines = stdout.split('\r\n');
                        const statusLine = lines[0] || 'HTTP OK';
                        
                        if (stdout.includes('llamacpp:') || stdout.includes('# HELP')) {
                            testStatusLabel.set_markup(`<span color="#2ec27e"><b>${_('Success: Connected!')}</b> (${statusLine})</span>`);
                        } else {
                            testStatusLabel.set_markup(`<span color="#e01b24"><b>${_('Warning: Connected, but llama.cpp metrics not found.')}</b> (${statusLine})</span>`);
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
            label: _('Update Interval (seconds)'),
            xalign: 0,
            visible: true,
        });
        grid.attach(updateLabel, 0, 1, 1, 1);

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
        grid.attach(updateSpin, 1, 1, 1, 1);

        this.getSettings().bind(
            'update-time', updateSpin, 'value',
            Gio.SettingsBindFlags.DEFAULT
        );

        // show-throughput
        const throughputCheck = new Gtk.CheckButton({
            label: _('Show throughput in panel'),
            visible: true,
        });
        throughputCheck.active = this.getSettings().get_boolean('show-throughput');
        grid.attach(throughputCheck, 0, 2, 2, 1);

        this.getSettings().bind(
            'show-throughput', throughputCheck, 'active',
            Gio.SettingsBindFlags.DEFAULT
        );

        // show-requests
        const requestsCheck = new Gtk.CheckButton({
            label: _('Show active requests in panel'),
            visible: true,
        });
        requestsCheck.active = this.getSettings().get_boolean('show-requests');
        grid.attach(requestsCheck, 0, 3, 2, 1);

        this.getSettings().bind(
            'show-requests', requestsCheck, 'active',
            Gio.SettingsBindFlags.DEFAULT
        );

        // precision
        const precisionLabel = new Gtk.Label({
            label: _('Decimal Precision (0-3)'),
            xalign: 0,
            visible: true,
        });
        grid.attach(precisionLabel, 0, 4, 1, 1);

        const precisionSpin = new Gtk.SpinButton({
            visible: true,
            adjustment: new Gtk.Adjustment({
                lower: 0,
                upper: 3,
                step_increment: 1,
                page_increment: 1,
            }),
        });
        precisionSpin.value = this.getSettings().get_int('precision');
        grid.attach(precisionSpin, 1, 4, 1, 1);

        this.getSettings().bind(
            'precision', precisionSpin, 'value',
            Gio.SettingsBindFlags.DEFAULT
        );

        // position
        const positionLabel = new Gtk.Label({
            label: _('Panel Position'),
            xalign: 0,
            visible: true,
        });
        grid.attach(positionLabel, 0, 5, 1, 1);

        const positionCombo = new Gtk.ComboBoxText({
            visible: true,
        });
        positionCombo.append('0', _('Left'));
        positionCombo.append('1', _('Center'));
        positionCombo.append('2', _('Right'));
        positionCombo.append('3', _('Far Left'));
        positionCombo.append('4', _('Far Right'));
        positionCombo.active = this.getSettings().get_int('position-in-panel');
        grid.attach(positionCombo, 1, 5, 1, 1);

        this.getSettings().bind(
            'position-in-panel', positionCombo, 'active',
            Gio.SettingsBindFlags.DEFAULT
        );

        // debug log
        this._setupDebugLog(grid);

        frame.connect('destroy', () => {
            this._destroyDebugLogPolling();
        });

        frame.set_child(grid);
        return frame;
    }

    _setupDebugLog(grid) {
        const logLabel = new Gtk.Label({
            label: _('Debug Log'),
            xalign: 0,
            visible: true,
        });
        grid.attach(logLabel, 0, 6, 1, 1);

        const clearBtn = new Gtk.Button({
            label: _('Clear'),
            visible: true,
        });
        clearBtn.connect('clicked', () => {
            this._debugLogBuffer.set_text('', -1);
        });
        grid.attach(clearBtn, 1, 6, 1, 1);

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
        grid.attach(scrolled, 0, 7, 2, 1);

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
