//! The QML-facing object: a thin shell over [`Core`]. Every invokable runs a core action, then
//! re-publishes the JSON; a failure becomes the `toast` text.
//!
//! `desktopKeys` changes only when the set of sticky windows does, and `revision` counts the
//! changes a sticky window has to re-read (a colour, a change made elsewhere): typing a title or
//! text republishes `stickies` (for the cards) but neither, so a window being typed in is never
//! rebuilt.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::core::Core;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, stickies)]
        #[qproperty(QString, links)]
        #[qproperty(QString, palette)]
        #[qproperty(QString, desktop_keys, cxx_name = "desktopKeys")]
        #[qproperty(QString, default_color, cxx_name = "defaultColor")]
        #[qproperty(bool, autostart)]
        #[qproperty(QString, toast)]
        #[qproperty(i32, revision)]
        type Backend = super::BackendRust;

        /// Start: read the settings and the stickies.
        #[qinvokable]
        fn init(self: Pin<&mut Backend>);
        /// Look at the watched files' times; re-read what changed. Called on a timer.
        #[qinvokable]
        fn poll(self: Pin<&mut Backend>);
        /// Whether the main window opens at start.
        #[qinvokable]
        #[cxx_name = "opensManager"]
        fn opens_manager(self: &Backend) -> bool;
        /// Whether nothing is left to show (`managerOpen`: the main window is).
        #[qinvokable]
        #[cxx_name = "shouldExit"]
        fn should_exit(self: &Backend, manager_open: bool) -> bool;
        /// One sticky (JSON), by its key.
        #[qinvokable]
        fn sticky(self: &Backend, key: &QString) -> QString;
        /// The value after `--name` on the command line, or empty (development flags).
        #[qinvokable]
        fn arg(self: &Backend, name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "setTitle"]
        fn set_title(self: Pin<&mut Backend>, key: &QString, title: &QString);
        #[qinvokable]
        #[cxx_name = "setBody"]
        fn set_body(self: Pin<&mut Backend>, key: &QString, body: &QString);
        /// Paint a sticky; an empty colour goes back to the app's colour or the default paper.
        #[qinvokable]
        #[cxx_name = "setColor"]
        fn set_color(self: Pin<&mut Backend>, key: &QString, hex: &QString);
        #[qinvokable]
        #[cxx_name = "setDesktop"]
        fn set_desktop(self: Pin<&mut Backend>, key: &QString, on: bool);
        #[qinvokable]
        #[cxx_name = "setLinked"]
        fn set_linked(self: Pin<&mut Backend>, app_id: &QString, on: bool);
        #[qinvokable]
        #[cxx_name = "newIn"]
        fn new_in(self: Pin<&mut Backend>, app_id: &QString);
        #[qinvokable]
        #[cxx_name = "enableAutostart"]
        fn enable_autostart(self: Pin<&mut Backend>, on: bool);
        #[qinvokable]
        #[cxx_name = "chooseDefaultColor"]
        fn choose_default_color(self: Pin<&mut Backend>, hex: &QString);
        #[qinvokable]
        #[cxx_name = "setPaletteColor"]
        fn set_palette_color(self: Pin<&mut Backend>, index: i32, hex: &QString);
        #[qinvokable]
        #[cxx_name = "addPaletteColor"]
        fn add_palette_color(self: Pin<&mut Backend>);
        #[qinvokable]
        #[cxx_name = "removePaletteColor"]
        fn remove_palette_color(self: Pin<&mut Backend>, index: i32);
        #[qinvokable]
        #[cxx_name = "resetPalette"]
        fn reset_palette(self: Pin<&mut Backend>);
    }
}

pub struct BackendRust {
    core: Option<Core>,
    stickies: QString,
    links: QString,
    palette: QString,
    desktop_keys: QString,
    default_color: QString,
    autostart: bool,
    toast: QString,
    revision: i32,
}

impl Default for BackendRust {
    fn default() -> Self {
        Self {
            core: None,
            stickies: QString::from("[]"),
            links: QString::from("[]"),
            palette: QString::from("[]"),
            desktop_keys: QString::from("[]"),
            default_color: QString::from("#fff59d"),
            autostart: false,
            toast: QString::default(),
            revision: 0,
        }
    }
}

fn arg_value(name: &str) -> String {
    let flag = format!("--{name}");
    std::env::args()
        .skip_while(|a| *a != flag)
        .nth(1)
        .unwrap_or_default()
}

impl qobject::Backend {
    pub fn init(mut self: Pin<&mut Self>) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let background = organiser_sticky::discover::wants_background(&args);
        self.as_mut().rust_mut().core = Some(Core::open(background));
        self.publish(true);
    }

    /// Show the core's state: `windows` when sticky windows have to re-read their sticky.
    fn publish(mut self: Pin<&mut Self>, windows: bool) {
        let Some(core) = self.core.as_ref() else {
            return;
        };
        let stickies = QString::from(core.stickies_json());
        let links = QString::from(core.links_json());
        let palette = QString::from(core.palette_json());
        let keys = QString::from(serde_json::json!(core.desktop_keys()).to_string());
        let default = QString::from(core.default_color_hex());
        let autostart = core.autostart;
        let notice = self
            .as_mut()
            .rust_mut()
            .core
            .as_mut()
            .and_then(|c| c.notice.take());
        self.as_mut().set_stickies(stickies);
        self.as_mut().set_links(links);
        self.as_mut().set_palette(palette);
        self.as_mut().set_desktop_keys(keys);
        self.as_mut().set_default_color(default);
        self.as_mut().set_autostart(autostart);
        if windows {
            let r = *self.revision();
            self.as_mut().set_revision(r.wrapping_add(1));
        }
        if let Some(n) = notice {
            // Cleared first, so the same text twice is still a change.
            self.as_mut().set_toast(QString::default());
            self.as_mut().set_toast(QString::from(n));
        }
    }

    /// Run `f` on the core, then re-publish.
    fn act(mut self: Pin<&mut Self>, windows: bool, f: impl FnOnce(&mut Core) -> bool) {
        if let Some(core) = self.as_mut().rust_mut().core.as_mut() {
            f(core);
        }
        self.publish(windows);
    }

    pub fn poll(mut self: Pin<&mut Self>) {
        let changed = self
            .as_mut()
            .rust_mut()
            .core
            .as_mut()
            .is_some_and(Core::poll);
        if changed {
            self.publish(true);
        }
    }

    pub fn opens_manager(&self) -> bool {
        self.core.as_ref().is_none_or(Core::opens_manager)
    }

    pub fn should_exit(&self, manager_open: bool) -> bool {
        self.core
            .as_ref()
            .is_some_and(|c| c.should_exit(manager_open))
    }

    pub fn sticky(&self, key: &QString) -> QString {
        QString::from(
            self.core
                .as_ref()
                .map_or_else(|| "{}".into(), |c| c.sticky_json(&key.to_string())),
        )
    }

    pub fn arg(&self, name: &QString) -> QString {
        QString::from(arg_value(&name.to_string()))
    }

    pub fn set_title(self: Pin<&mut Self>, key: &QString, title: &QString) {
        let (k, t) = (key.to_string(), title.to_string());
        self.act(false, |c| c.set_title(&k, &t));
    }

    pub fn set_body(self: Pin<&mut Self>, key: &QString, body: &QString) {
        let (k, b) = (key.to_string(), body.to_string());
        self.act(false, |c| c.set_body(&k, &b));
    }

    pub fn set_color(self: Pin<&mut Self>, key: &QString, hex: &QString) {
        let (k, h) = (key.to_string(), hex.to_string());
        self.act(true, |c| {
            c.set_color(&k, (!h.is_empty()).then_some(h.as_str()))
        });
    }

    pub fn set_desktop(self: Pin<&mut Self>, key: &QString, on: bool) {
        let k = key.to_string();
        self.act(true, |c| c.set_desktop(&k, on));
    }

    pub fn set_linked(self: Pin<&mut Self>, app_id: &QString, on: bool) {
        let a = app_id.to_string();
        self.act(true, |c| c.set_linked(&a, on));
    }

    pub fn new_in(self: Pin<&mut Self>, app_id: &QString) {
        let a = app_id.to_string();
        self.act(false, |c| c.new_in(&a));
    }

    pub fn enable_autostart(self: Pin<&mut Self>, on: bool) {
        self.act(false, |c| c.set_autostart(on));
    }

    pub fn choose_default_color(self: Pin<&mut Self>, hex: &QString) {
        let h = hex.to_string();
        self.act(true, |c| c.set_default_color(&h));
    }

    pub fn set_palette_color(self: Pin<&mut Self>, index: i32, hex: &QString) {
        let h = hex.to_string();
        let i = usize::try_from(index).unwrap_or(usize::MAX);
        self.act(true, |c| c.set_palette_color(i, &h));
    }

    pub fn add_palette_color(self: Pin<&mut Self>) {
        self.act(true, Core::add_palette_color);
    }

    pub fn remove_palette_color(self: Pin<&mut Self>, index: i32) {
        let i = usize::try_from(index).unwrap_or(usize::MAX);
        self.act(true, |c| c.remove_palette_color(i));
    }

    pub fn reset_palette(self: Pin<&mut Self>) {
        self.act(true, Core::reset_palette);
    }
}
