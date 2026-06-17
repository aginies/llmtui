import Gtk from 'gi://Gtk';
import GObject from 'gi://GObject';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

const WS_METRICS = [
    { key: 'model_name', label: 'Model' },
    { key: 'tps', label: 'TPS' },
    { key: 'prompt_tps', label: 'Prompt TPS' },
    { key: 'gen_tps', label: 'Gen TPS' },
    { key: 'ctx', label: 'Ctx' },
    { key: 'vram', label: 'VRAM' },
    { key: 'ram', label: 'RAM' },
    { key: 'cpu', label: 'CPU' },
    { key: 'decoded_tokens', label: 'Decoded Tokens' },
];

export default class LlmManagerPreferences extends ExtensionPreferences {
    getPreferencesWidget() {
        const frame = new Gtk.Frame({
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
        const urlBox = new Gtk.Box({
            orientation: Gtk.Orientation.VERTICAL,
            spacing: 4,
            visible: true,
        });

        const urlLabel = new Gtk.Label({
            label: _('Metrics URL'),
            xalign: 0,
            use_markup: true,
            visible: true,
        });
        urlLabel.set_markup(`<b>${_('Metrics URL')}</b>`);
        urlBox.append(urlLabel);

        const entryBox = new Gtk.Box({
            orientation: Gtk.Orientation.HORIZONTAL,
            spacing: 6,
            hexpand: true,
            visible: true,
        });

        const urlEntry = new Gtk.Entry({
            visible: true,
            primary_icon_name: 'network-server-symbolic',
            hexpand: true,
        });
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

        grid.attach(urlBox, 0, row, 2, 1);
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

        // secret
        const secretLabel = new Gtk.Label({
            label: _('Secret'),
            xalign: 0,
            visible: true,
        });
        grid.attach(secretLabel, 0, row, 1, 1);

        const secretBox = new Gtk.Box({
            orientation: Gtk.Orientation.HORIZONTAL,
            spacing: 6,
            hexpand: true,
            visible: true,
        });

        const secretEntry = new Gtk.Entry({
            visible: true,
            visibility: false,
            primary_icon_name: 'dialog-password',
            hexpand: true,
        });
        secretBox.append(secretEntry);

        grid.attach(secretBox, 1, row, 1, 1);
        row++;

        this.getSettings().bind(
            'metrics-secret', secretEntry, 'text',
            Gio.SettingsBindFlags.DEFAULT
        );

        // update-time
        const updateLabel = new Gtk.Label({
            label: _('Second between updates'),
            xalign: 0,
            visible: true,
        });
        grid.attach(updateLabel, 0, row, 1, 1);

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

        // position
        const positionLabel = new Gtk.Label({
            label: _('Panel Position'),
            xalign: 0,
            visible: true,
        });
        grid.attach(positionLabel, 0, row, 1, 1);

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
        const metricsFrame = new Gtk.Frame({
            label: _('Selected Metrics to Display'),
            visible: true,
            margin_start: 6,
            margin_end: 6,
            margin_top: 6,
            margin_bottom: 6,
        });
        const metricsFrameLabel = new Gtk.Label({
            label: `<b>${_('Selected Metrics to Display')}</b>`,
            use_markup: true,
            xalign: 0,
            visible: true,
        });
        metricsFrame.set_label_widget(metricsFrameLabel);

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

        // About
        const aboutFrame = new Gtk.Frame({
            label: _('About'),
            visible: true,
            margin_start: 6,
            margin_end: 6,
            margin_top: 6,
            margin_bottom: 6,
        });
        const aboutFrameLabel = new Gtk.Label({
            label: `<b>${_('About')}</b>`,
            use_markup: true,
            xalign: 0,
            visible: true,
        });
        aboutFrame.set_label_widget(aboutFrameLabel);

        const aboutBox = new Gtk.Box({
            orientation: Gtk.Orientation.VERTICAL,
            spacing: 6,
            visible: true,
            margin_start: 12,
            margin_end: 12,
            margin_top: 12,
            margin_bottom: 12,
        });

        const linkBtn = new Gtk.LinkButton({
            uri: 'https://github.com/aginies/llm-manager',
            label: 'https://github.com/aginies/llm-manager',
            visible: true,
        });
        aboutBox.append(linkBtn);

        aboutFrame.set_child(aboutBox);
        grid.attach(aboutFrame, 0, row, 2, 1);
        row++;

        frame.set_child(grid);
        return frame;
    }
}
