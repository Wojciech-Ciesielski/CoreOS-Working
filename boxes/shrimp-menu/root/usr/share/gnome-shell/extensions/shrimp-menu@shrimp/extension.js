// Shrimp Menu: the Shrimp logo on the left of the top bar opens the system menu
// (about this computer, settings, Shrimp Apps, Cocktail updates, Panko system restore, system tools,
// sleep, restart, power off, lock, log out). Texts: GNOME's own translations where GNOME has the
// words, otherwise lang/_keys.tsv + lang/<tag>.txt (same format as i18n/).
import GObject from 'gi://GObject';
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import St from 'gi://St';
import Shell from 'gi://Shell';
import Gettext from 'gettext';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const decoder = new TextDecoder();

function readText(path) {
    try {
        const [ok, bytes] = GLib.file_get_contents(path);
        return ok ? decoder.decode(bytes) : null;
    } catch {
        return null;
    }
}

function parseTsv(text, cols) {
    const out = [];
    for (const line of (text ?? '').split(/\r?\n/)) {
        if (!line || line.startsWith('#'))
            continue;
        const parts = line.split('\t');
        if (parts.length >= cols)
            out.push(parts);
    }
    return out;
}

// Locale name (pl_PL.UTF-8, sr_RS@latin, zh_TW) -> candidate tags of our lang/ files,
// most specific first (same tags as boxes/i18n-core/langs.ts).
const MODIFIERS = {latin: 'latn', cyrillic: 'cyrl', valencia: 'valencia'};
const ALIASES = {
    'zh-cn': 'zh-hans', 'zh-sg': 'zh-hans', 'zh': 'zh-hans',
    'zh-tw': 'zh-hant', 'zh-hk': 'zh-hant', 'zh-mo': 'zh-hant',
    'pt': 'pt-pt', 'no': 'nb', 'sr': 'sr-cyrl',
};

function candidateTags() {
    const tags = [];
    const add = t => {
        if (t && !tags.includes(t))
            tags.push(t);
        if (ALIASES[t] && !tags.includes(ALIASES[t]))
            tags.push(ALIASES[t]);
    };
    for (const name of GLib.get_language_names()) {
        if (name === 'C' || name === 'POSIX')
            continue;
        const mod = (name.match(/@(\w+)/) ?? [])[1];
        const base = name.replace(/\..*?(?=@|$)/, '').replace(/@.*$/, '').replace(/_/g, '-').toLowerCase();
        const lang = base.split('-')[0];
        if (mod && MODIFIERS[mod])
            add(`${lang}-${MODIFIERS[mod]}`);
        add(base);
        add(lang);
    }
    return tags;
}

// Our own texts (lang/), only for what GNOME does not translate itself. English fallback.
function loadStrings(dir) {
    const strings = {};
    const byId = {};
    for (const [id, key, en] of parseTsv(readText(`${dir}/lang/_keys.tsv`), 3)) {
        strings[key] = en;
        byId[id] = key;
    }
    for (const tag of candidateTags()) {
        const text = readText(`${dir}/lang/${tag}.txt`);
        if (text === null)
            continue;
        for (const [id, value] of parseTsv(text, 2)) {
            if (byId[id])
                strings[byId[id]] = value;
        }
        break;
    }
    return strings;
}

// The same words GNOME Shell uses in its own power menu, in every language GNOME has.
const shell = msgid => Gettext.dgettext('gnome-shell', msgid);

function findApp(ids) {
    const appSystem = Shell.AppSystem.get_default();
    for (const id of ids) {
        const app = appSystem.lookup_app(id);
        if (app)
            return app;
    }
    return null;
}

function spawn(argv) {
    try {
        GLib.spawn_async(null, argv, null, GLib.SpawnFlags.SEARCH_PATH, null);
    } catch (e) {
        console.error(`Shrimp Menu: ${argv.join(' ')}: ${e.message}`);
    }
}

const ShrimpMenuButton = GObject.registerClass(
class ShrimpMenuButton extends PanelMenu.Button {
    _init(dir) {
        super._init(0.0, 'Shrimp', false);
        this._t = loadStrings(dir);
        this.add_style_class_name('shrimp-menu-button');
        this.accessible_name = 'Shrimp';

        this.add_child(new St.Icon({
            gicon: Gio.ThemedIcon.new_with_default_fallbacks('shrimp-logo-symbolic'),
            style_class: 'system-status-icon shrimp-menu-icon',
        }));

        // Applications can be installed or removed while the session runs.
        this.menu.connect('open-state-changed', (_menu, open) => {
            if (open)
                this._build();
        });
        this._build();
    }

    _item(label, callback) {
        const item = new PopupMenu.PopupMenuItem(label);
        item.connect('activate', () => {
            this.menu.close();
            callback();
        });
        this.menu.addMenuItem(item);
        return item;
    }

    // label: our text, or null for the program's own (translated) name.
    _appItem(label, ids) {
        const app = findApp(ids);
        if (app)
            this._item(label ?? app.get_name(), () => app.activate());
        return app;
    }

    _separator() {
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
    }

    _build() {
        this.menu.removeAll();
        const t = this._t;

        this._item(t['menu.about'], () => spawn(['gnome-control-center', 'system', 'about']));
        this._appItem(shell('Settings'), ['org.gnome.Settings.desktop', 'gnome-control-center.desktop']);
        this._separator();

        // Shrimp Apps is the store (GNOME Software is not in the image). Updates = Cocktail (the whole
        // system: packages, drivers, Flatpaks), built into the image by its box (boxes/cocktail); without
        // it, the "Installed" page of Shrimp Apps, which updates the Flatpaks.
        const store = this._appItem(t['menu.software'], ['org.shrimp.Apps.desktop']);
        if (!this._appItem(t['menu.updates'], ['org.shrimp.Cocktail.desktop']) && store)
            this._item(t['menu.updates'], () => store.activate());
        this._appItem(t['menu.restore'], ['org.shrimp.Panko.desktop']);
        this._separator();

        const tools = [
            ['org.gnome.SystemMonitor.desktop', 'gnome-system-monitor.desktop'],
            ['org.gnome.DiskUtility.desktop', 'gnome-disks.desktop'],
            ['org.gnome.Console.desktop', 'org.gnome.Terminal.desktop'],
            ['org.gnome.Extensions.desktop', 'com.mattjakeman.ExtensionManager.desktop'],
        ];
        if (tools.map(ids => this._appItem(null, ids)).some(Boolean))
            this._separator();

        this._item(shell('Suspend'), () => spawn(['systemctl', 'suspend']));
        this._item(shell('Restart…'), () => spawn(['gnome-session-quit', '--reboot']));
        this._item(shell('Power Off…'), () => spawn(['gnome-session-quit', '--power-off']));
        this._separator();

        this._item(Gettext.dpgettext('gnome-shell', 'action', 'Lock Screen'), () => spawn(['loginctl', 'lock-session']));
        this._item(shell('Log Out…'), () => spawn(['gnome-session-quit', '--logout']));
    }
});

export default class ShrimpMenuExtension extends Extension {
    enable() {
        this._button = new ShrimpMenuButton(this.path);
        // Index 0 on the left; Activities is hidden by Just Perfection (the dock has the app grid).
        Main.panel.addToStatusArea(this.uuid, this._button, 0, 'left');
    }

    disable() {
        this._button?.destroy();
        this._button = null;
    }
}
