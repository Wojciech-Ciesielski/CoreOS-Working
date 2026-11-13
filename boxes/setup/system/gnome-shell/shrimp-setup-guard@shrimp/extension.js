// Shrimp Setup Guard: while Shrimp is being installed (live session) or set up for the first time (after
// the first restart), Setup is the only thing on screen.
//  - A black screen with the Shrimp logo covers the desktop until the Setup window is drawn.
//  - Shell shortcuts (Alt+Tab, Super, Alt+F2, ...) do nothing.
//  - The Overview cannot open: not with Super, not with the hot corner, the dock or the touchpad gesture
//    (three or four fingers up), and workspaces cannot be swiped. Beta 5 on the owner's laptop: the gesture
//    opened the Overview and the desktop around Setup (2026-11-13).
//  - Only the Setup window counts (application id org.shrimp.Setup). Other normal windows are minimised
//    and Setup comes back to the front.
//  - First configuration: if the Setup window disappears before the configuration is finished (crash),
//    Setup is started again; onboarding cannot be left half way.
// The cover never reacts to input, so clicks always reach Setup.
//
// Active when:
//   /etc/xdg/autostart/shrimp-setup-install.desktop exists  -> live medium (install)
//   /var/lib/shrimp-setup/first-boot exists                  -> first configuration
// Off when a drive labelled SHRIMP_TEST is connected (QEMU tests in CI, support: format a small
// FAT32 stick with that label and plug it in to get the normal desktop and a terminal).
//
// Safety net: the cover goes away by itself COVER_MAX_S after login even if no window ever shows up, the
// shortcuts work again as soon as Setup is finished (or, on the live medium, gone), and a Setup that keeps
// crashing is restarted at most RESTARTS_MAX times, so nobody is locked out of the computer.
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
const SETUP_ID = 'org.shrimp.Setup';
// Without an application id (should not happen) the first normal window after this long is taken as Setup.
const ADOPT_ANY_S = 20;
const RESTARTS_MAX = 3;

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

        this._restarts = 0;
        this._adoptAny = false;
        this._showCover();
        this._blockOverview();
        this._connect(global.window_manager, 'filter-keybinding',
            (_wm, binding) => !ALLOWED_KEYBINDINGS.has(binding.get_name()));
        this._connect(global.display, 'window-created', (_d, win) => this._onWindow(win));
        for (const actor of global.get_window_actors())
            this._onWindow(actor.meta_window);
        // If the startup animation is still running, leave the Overview alone until it ends.
        if (Main.layoutManager._startingUp)
            this._connect(Main.layoutManager, 'startup-complete', () => this._closeOverview());
        this._timeout(ADOPT_ANY_S, () => {
            if (this._window)
                return;
            this._adoptAny = true;
            for (const actor of global.get_window_actors())
                this._onWindow(actor.meta_window);
        });
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
        this._unblockOverview();
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

    _isSetup(win) {
        const ids = [win.get_gtk_application_id?.(), win.get_wm_class?.(), win.get_sandboxed_app_id?.()];
        return ids.some(id => id === SETUP_ID) || ids.some(id => /shrimp-setup/i.test(id ?? ''));
    }

    _onWindow(win) {
        if (!win || win.get_window_type() !== Meta.WindowType.NORMAL)
            return;
        // Desktop icons (DING) draw their own normal window: that one is never Setup and stays as it is.
        const id = `${win.get_wm_class?.() ?? ''} ${win.get_gtk_application_id?.() ?? ''} ${win.get_title?.() ?? ''}`;
        if (/ding|desktop icons/i.test(id))
            return;
        if (this._window) {
            if (win !== this._window && !win.get_transient_for() && !this._isSetup(win))
                this._foreign(win);
            return;
        }
        if (!this._isSetup(win) && !this._adoptAny) {
            // A window that is not Setup (yet): it may be Setup without its id set yet, so look again when drawn.
            const actor = win.get_compositor_private();
            if (actor && !actor.mapped)
                this._connect(actor, 'first-frame', () => this._onWindow(win));
            return;
        }
        this._window = win;
        log(`window: class ${win.get_wm_class?.()}, app ${win.get_gtk_application_id?.()}, title ${win.get_title?.()}`);
        win.make_above();
        this._connect(win, 'unmanaged', () => {
            if (this._window !== win)
                return;
            this._window = null;
            this._onSetupGone();
        });
        const actor = win.get_compositor_private();
        if (actor?.mapped && actor.visible)
            this._onDrawn();
        else if (actor)
            this._connect(actor, 'first-frame', () => this._onDrawn());
        else
            this._timeout(2, () => this._onDrawn());
    }

    // Another program's window while Setup is on screen: minimised, Setup back in front.
    _foreign(win) {
        log(`other window minimised: ${win.get_wm_class?.()} ${win.get_title?.()}`);
        try {
            win.minimize();
            this._window?.activate(global.get_current_time());
        } catch (e) {
            log(`other window: ${e.message}`);
        }
    }

    _onSetupGone() {
        // Live medium: Setup is gone (finished, rebooting or crashed): normal desktop and shortcuts again,
        // the "Install Shrimp" shortcut starts it again.
        // First configuration: finished = the marker is gone. Otherwise Setup crashed: start it again.
        if (this._mode === 'first-boot' && exists(FIRST_BOOT_MARKER) && this._restarts < RESTARTS_MAX) {
            this._restarts++;
            log(`Setup window closed before the configuration finished: starting Setup again (${this._restarts}/${RESTARTS_MAX})`);
            this._timeout(2, () => {
                try {
                    GLib.spawn_async(null, ['shrimp-setup', '--first-boot'], null, GLib.SpawnFlags.SEARCH_PATH, null);
                } catch (e) {
                    log(`cannot start Setup: ${e.message}`);
                    this._deactivate();
                }
            });
            return;
        }
        log('Setup window closed: guard off');
        this._deactivate();
    }

    _onDrawn() {
        const win = this._window;
        if (!win)
            return;
        log(`Setup drawn: ${win.get_wm_class?.()} ${win.get_gtk_application_id?.()}`);
        this._closeOverview();
        this._hideCover(true);
    }

    // --- Overview, hot corner, gestures ---

    _blockOverview() {
        const ov = Main.overview;
        this._saved = {show: ov.show, toggle: ov.toggle, showApps: ov.showApps};
        const no = name => () => log(`Overview blocked (${name})`);
        ov.show = no('show');
        ov.toggle = no('toggle');
        ov.showApps = no('showApps');
        // Touchpad and touchscreen: Overview gesture and workspace swipe.
        this._trackers = [ov._swipeTracker, ov._overview?._controls?._workspacesDisplay?._swipeTracker,
            Main.wm?._workspaceAnimation?._swipeTracker].filter(t => t && 'enabled' in t);
        this._trackerState = this._trackers.map(t => t.enabled);
        this._trackers.forEach(t => {
            t.enabled = false;
        });
        // Anything else that still opens it (a gesture already in progress) closes it right away.
        // Not during the startup animation: closing it then left an invisible layer that swallowed clicks
        // (Beta 3.2); startup-complete closes it instead (enable()).
        this._connect(ov, 'showing', () => {
            if (Main.layoutManager._startingUp)
                return;
            log('Overview opened by something else: closing');
            GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                ov.hide();
                return GLib.SOURCE_REMOVE;
            });
        });
    }

    _unblockOverview() {
        if (!this._saved)
            return;
        const ov = Main.overview;
        ov.show = this._saved.show;
        ov.toggle = this._saved.toggle;
        ov.showApps = this._saved.showApps;
        this._trackers?.forEach((t, i) => {
            t.enabled = this._trackerState[i];
        });
        this._trackers = null;
        this._saved = null;
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
