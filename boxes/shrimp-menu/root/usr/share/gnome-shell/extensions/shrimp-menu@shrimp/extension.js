// Shrimp Menu: the Shrimp logo on the left of the top bar opens the system menu
// (about this computer, settings, Shrimp Apps, Cocktail updates, Panko system restore, system tools).
// Sleep, restart, power off, lock and log out stay only in GNOME's own system menu (top right): two
// menus with the same buttons were confusing (owner's review, 2026-11-13). Texts: GNOME's own
// translations where GNOME has the words, otherwise lang/_keys.tsv + lang/<tag>.txt (same format as i18n/).
//
// The extension also keeps two small desktop rules of Shrimp:
//  - the app grid (Overview) is sorted alphabetically, folders first, and stays that way;
//  - an installer downloaded from the internet (.exe, .msi, .rpm, .deb, .AppImage, .dmg) gets a short
//    note that Shrimp Apps is the better place for programs (`shrimp-apps --hint download FILE`).
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
import * as AppDisplay from 'resource:///org/gnome/shell/ui/appDisplay.js';

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

        // System tools. No Extensions app: Shrimp does not offer extensions to the user.
        const tools = [
            ['org.gnome.SystemMonitor.desktop', 'gnome-system-monitor.desktop'],
            ['org.gnome.DiskUtility.desktop', 'gnome-disks.desktop'],
            ['org.shrimp.Cherry.desktop', 'org.gnome.Console.desktop', 'org.gnome.Terminal.desktop'],
        ].filter(ids => findApp(ids));
        if (tools.length)
            this._separator();
        tools.forEach(ids => this._appItem(null, ids));
    }
});

// --- app grid: alphabetical, folders first ---
// GNOME keeps a manual order in org.gnome.shell app-picker-layout; with an empty layout every icon has no
// saved position and the grid is ordered by AppDisplay._compareItems. We compare folders before apps,
// then by name in the user's language, and clear the layout again whenever GNOME saves one (drag & drop).
class AlphabeticalGrid {
    enable() {
        const proto = AppDisplay.AppDisplay?.prototype;
        if (!proto || typeof proto._compareItems !== 'function') {
            console.warn('Shrimp Menu: AppDisplay._compareItems not found, app grid order unchanged');
            return;
        }
        this._proto = proto;
        this._orig = proto._compareItems;
        const collator = new Intl.Collator(undefined, {sensitivity: 'base', numeric: true});
        proto._compareItems = function (a, b) {
            const fa = a instanceof AppDisplay.FolderIcon, fb = b instanceof AppDisplay.FolderIcon;
            if (fa !== fb)
                return fa ? -1 : 1;
            return collator.compare(a.name ?? '', b.name ?? '');
        };
        this._settings = new Gio.Settings({schema_id: 'org.gnome.shell'});
        this._changed = this._settings.connect('changed::app-picker-layout', () => this._resort());
        this._resort();
    }

    _resort() {
        if (this._settings.get_value('app-picker-layout').n_children() > 0)
            this._settings.set_value('app-picker-layout', new GLib.Variant('aa{sv}', []));
        // Redisplay after GNOME has finished its own handling of the layout change.
        if (this._idle)
            return;
        this._idle = GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            this._idle = 0;
            try {
                Main.overview._overview?._controls?._appDisplay?._redisplay();
            } catch (e) {
                console.warn(`Shrimp Menu: app grid: ${e.message}`);
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    disable() {
        if (this._idle)
            GLib.source_remove(this._idle);
        this._idle = 0;
        if (this._settings && this._changed)
            this._settings.disconnect(this._changed);
        this._settings = null;
        if (this._proto)
            this._proto._compareItems = this._orig;
        this._proto = null;
    }
}

// --- installers downloaded from the internet ---
// Shrimp installs programs from Shrimp Apps (Flatpak). A Windows or other-distribution installer in
// Downloads will not work; Shrimp Apps shows a short note with a search for the program in the store.
const INSTALLER = /\.(exe|msi|msix|appx|rpm|deb|appimage|dmg|pkg|snap)$/i;
const PARTIAL = /\.(part|crdownload|download|tmp)$/i;

class DownloadsWatcher {
    enable() {
        const dir = GLib.get_user_special_dir(GLib.UserDirectory.DIRECTORY_DOWNLOAD);
        if (!dir || !GLib.find_program_in_path('shrimp-apps'))
            return;
        this._seen = new Set();
        this._timeouts = new Set();
        try {
            this._monitor = Gio.File.new_for_path(dir).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
        } catch (e) {
            console.warn(`Shrimp Menu: downloads: ${e.message}`);
            return;
        }
        this._monitor.connect('changed', (_m, file, other, event) => {
            // Browsers write NAME.part / NAME.crdownload and rename it at the end.
            let f = null;
            if (event === Gio.FileMonitorEvent.RENAMED || event === Gio.FileMonitorEvent.MOVED_IN)
                f = other ?? file;
            else if (event === Gio.FileMonitorEvent.CHANGES_DONE_HINT || event === Gio.FileMonitorEvent.CREATED)
                f = file;
            const path = f?.get_path();
            const name = f?.get_basename() ?? '';
            if (!path || PARTIAL.test(name) || !INSTALLER.test(name) || this._seen.has(path))
                return;
            this._seen.add(path);
            // One note per file, a moment after the download ends (the browser may still rename it).
            const id = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
                this._timeouts.delete(id);
                if (GLib.file_test(path, GLib.FileTest.EXISTS))
                    spawn(['shrimp-apps', '--hint', 'download', path]);
                return GLib.SOURCE_REMOVE;
            });
            this._timeouts.add(id);
        });
    }

    disable() {
        for (const id of this._timeouts ?? [])
            GLib.source_remove(id);
        this._timeouts = null;
        this._monitor?.cancel();
        this._monitor = null;
        this._seen = null;
    }
}

export default class ShrimpMenuExtension extends Extension {
    enable() {
        this._button = new ShrimpMenuButton(this.path);
        // Index 0 on the left; Activities is hidden by Just Perfection (the dock has the app grid).
        Main.panel.addToStatusArea(this.uuid, this._button, 0, 'left');
        this._grid = new AlphabeticalGrid();
        this._grid.enable();
        this._downloads = new DownloadsWatcher();
        this._downloads.enable();
    }

    disable() {
        this._downloads?.disable();
        this._downloads = null;
        this._grid?.disable();
        this._grid = null;
        this._button?.destroy();
        this._button = null;
    }
}
