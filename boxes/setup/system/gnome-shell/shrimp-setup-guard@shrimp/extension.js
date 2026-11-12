// Shrimp Setup Guard, the simplest protection that looks good: while Shrimp is being installed
// (live session) or set up for the first time (after the first restart), a black screen with the
// Shrimp logo covers the desktop until the Setup window is drawn, and shell shortcuts (Alt+Tab,
// Super, Alt+F2, ...) do nothing. That is all. It never touches the Overview, the touchpad gestures
// or the pointer: the cover does not react to input, so clicks always reach Setup.
//
// Active when:
//   /etc/xdg/autostart/shrimp-setup-install.desktop exists  -> live medium (install)
//   /var/lib/shrimp-setup/first-boot exists                  -> first configuration
// Off when a drive labelled SHRIMP_TEST is connected (QEMU tests in CI, support: format a small
// FAT32 stick with that label and plug it in to get the normal desktop and a terminal).
//
// Safety net: the cover goes away by itself COVER_MAX_S after login even if no window ever shows up,
// and the shortcuts work again as soon as Setup is gone, so nobody is locked out of the computer.
import GLib from 'gi://GLib';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const LIVE_AUTOSTART = '/etc/xdg/autostart/shrimp-setup-install.desktop';
const FIRST_BOOT_MARKER = '/var/lib/shrimp-setup/first-boot';
const NOGUARD = '/dev/disk/by-label/SHRIMP_TEST';
// Keyboard layout switching stays available (the account and Wi-Fi password fields need it).
const ALLOWED_KEYBINDINGS = new Set(['switch-input-source', 'switch-input-source-backward']);
const COVER_MAX_S = 90;

function log(msg) {
    console.log(`shrimp-setup-guard: ${msg}`);
}

function exists(path) {
    return GLib.file_test(path, GLib.FileTest.EXISTS);
}

function guardMode() {
    if (exists(NOGUARD))
        return null;
    if (exists(LIVE_AUTOSTART))
        return 'install';
    if (exists(FIRST_BOOT_MARKER))
        return 'first-boot';
    return null;
}

export default class SetupGuardExtension extends Extension {
    enable() {
        this._mode = guardMode();
        if (!this._mode)
            return;
        this._signals = [];
        this._sources = new Set();
        this._window = null;
        log(`active (${this._mode}), session mode ${Main.sessionMode.currentMode}, shell starting up: ${Boolean(Main.layoutManager._startingUp)}`);

        // GNOME opens the Overview at login; Setup would then be a thumbnail. Do not start there.
        if (Main.layoutManager.startInOverview !== undefined)
            Main.layoutManager.startInOverview = false;

        this._showCover();
        this._connect(global.window_manager, 'filter-keybinding',
            (_wm, binding) => !ALLOWED_KEYBINDINGS.has(binding.get_name()));
        this._connect(global.display, 'window-created', (_d, win) => this._onWindow(win));
        for (const actor of global.get_window_actors())
            this._onWindow(actor.meta_window);
        // If the startup animation is still running, leave the Overview alone until it ends.
        if (Main.layoutManager._startingUp)
            this._connect(Main.layoutManager, 'startup-complete', () => this._closeOverview());
        this._timeout(COVER_MAX_S, () => {
            log(`no Setup window after ${COVER_MAX_S} s: cover removed`);
            this._hideCover(true);
        });
        // The SHRIMP_TEST drive may show up a moment after login.
        for (const s of [5, 20, 60]) {
            this._timeout(s, () => {
                if (exists(NOGUARD))
                    this._deactivate();
            });
        }
    }

    disable() {
        this._deactivate();
    }

    _deactivate() {
        if (!this._signals)
            return;
        for (const id of this._sources)
            GLib.source_remove(id);
        this._sources.clear();
        for (const [obj, id] of this._signals) {
            try {
                obj.disconnect(id);
            } catch (_e) {
                // the object is already gone
            }
        }
        this._signals = null;
        this._hideCover(false);
        this._window = null;
        this._mode = null;
    }

    _connect(obj, signal, fn) {
        this._signals.push([obj, obj.connect(signal, fn)]);
    }

    _timeout(seconds, fn) {
        const id = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            this._sources.delete(id);
            fn();
            return GLib.SOURCE_REMOVE;
        });
        this._sources.add(id);
    }

    // The only normal window in this session is Setup: the first one that gets drawn removes the cover.
    _onWindow(win) {
        if (!win || this._window || win.get_window_type() !== Meta.WindowType.NORMAL)
            return;
        this._window = win;
        log(`window: class ${win.get_wm_class?.()}, title ${win.get_title?.()}`);
        win.make_above();
        this._connect(win, 'unmanaged', () => {
            if (this._window !== win)
                return;
            // Setup is gone (finished or crashed): normal desktop and shortcuts again.
            log('Setup window closed: guard off');
            this._deactivate();
        });
        const actor = win.get_compositor_private();
        if (actor?.mapped && actor.visible)
            this._onDrawn();
        else if (actor)
            this._connect(actor, 'first-frame', () => this._onDrawn());
        else
            this._timeout(2, () => this._onDrawn());
    }

    _onDrawn() {
        // Desktop icons (DING, first configuration) draw their own normal window: that one is not Setup.
        const win = this._window;
        const id = `${win?.get_wm_class?.() ?? ''} ${win?.get_gtk_application_id?.() ?? ''} ${win?.get_title?.() ?? ''}`;
        if (/ding|desktop icons/i.test(id)) {
            log(`not Setup: ${id}`);
            win.unmake_above();
            this._window = null;
            return;
        }
        log(`Setup drawn: ${id}`);
        this._closeOverview();
        this._hideCover(true);
    }

    _closeOverview() {
        if (Main.layoutManager._startingUp || !this._window)
            return;
        if (Main.overview.visible)
            Main.overview.hide();
    }

    // --- cover ---

    _showCover() {
        if (this._cover)
            return;
        const cover = new St.Widget({
            // Never catches the pointer: clicks go to whatever is below.
            reactive: false,
            style: 'background-color: #000000;',
            layout_manager: new Clutter.BinLayout(),
        });
        cover.add_child(new St.Icon({
            icon_name: 'shrimp-logo-symbolic',
            icon_size: 96,
            style: 'color: rgba(255, 255, 255, 0.92);',
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
            x_expand: true,
            y_expand: true,
        }));
        cover.add_constraint(new Clutter.BindConstraint({source: global.stage, coordinate: Clutter.BindCoordinate.ALL}));
        Main.layoutManager.addTopChrome(cover);
        this._cover = cover;
    }

    _hideCover(animate) {
        const cover = this._cover;
        if (!cover)
            return;
        this._cover = null;
        const done = () => {
            Main.layoutManager.removeChrome(cover);
            cover.destroy();
        };
        if (!animate) {
            done();
            return;
        }
        cover.ease({opacity: 0, duration: 250, mode: Clutter.AnimationMode.EASE_OUT_QUAD, onComplete: done});
    }
}
