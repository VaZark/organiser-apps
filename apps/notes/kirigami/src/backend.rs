//! The QML-facing object: a thin shell over [`Core`]. Every invokable runs a core action, then
//! re-publishes what the page shows; a failure becomes the `toast` text. No decision is made here.

use core::pin::Pin;
use std::path::PathBuf;

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
        /// The page: JSON from `Core::page_json`.
        #[qproperty(QString, page)]
        /// Every folder as a tree list: JSON from `Core::folders_json`.
        #[qproperty(QString, folders)]
        #[qproperty(QString, tags)]
        #[qproperty(QString, filters)]
        #[qproperty(QString, prefs)]
        #[qproperty(QString, toast)]
        #[qproperty(QString, scope)]
        #[qproperty(i64, folder_id, cxx_name = "folderId")]
        #[qproperty(QString, sort)]
        type Backend = super::BackendRust;

        #[qinvokable]
        fn init(self: Pin<&mut Backend>);
        #[qinvokable]
        fn reload(self: Pin<&mut Backend>);
        /// The value after `--name` on the command line, or empty (development flags).
        #[qinvokable]
        fn arg(self: &Backend, name: &QString) -> QString;

        // What the page shows.
        #[qinvokable]
        fn show(self: Pin<&mut Backend>, name: &QString, folder: i64);
        #[qinvokable]
        #[cxx_name = "setSearch"]
        fn set_search(self: Pin<&mut Backend>, text: &QString);
        #[qinvokable]
        #[cxx_name = "sortBy"]
        fn sort_by(self: Pin<&mut Backend>, name: &QString);
        #[qinvokable]
        #[cxx_name = "toggleFilter"]
        fn toggle_filter(self: Pin<&mut Backend>, field: &QString, value: i64);
        #[qinvokable]
        #[cxx_name = "toggleTagFilter"]
        fn toggle_tag_filter(self: Pin<&mut Backend>, name: &QString);
        #[qinvokable]
        #[cxx_name = "clearFilters"]
        fn clear_filters(self: Pin<&mut Backend>);

        // Notes.
        #[qinvokable]
        fn note(self: &Backend, id: i64) -> QString;
        #[qinvokable]
        #[cxx_name = "openNote"]
        fn open_note(self: Pin<&mut Backend>, id: i64);
        /// Create a note from a draft (JSON); returns its id, or -1 (and a toast) on failure.
        #[qinvokable]
        #[cxx_name = "createNote"]
        fn create_note(self: Pin<&mut Backend>, draft: &QString) -> i64;
        #[qinvokable]
        #[cxx_name = "editNote"]
        fn edit_note(self: Pin<&mut Backend>, id: i64, title: &QString, body: &QString);
        #[qinvokable]
        #[cxx_name = "setNoteColor"]
        fn set_note_color(self: Pin<&mut Backend>, id: i64, color: &QString);
        #[qinvokable]
        #[cxx_name = "setPinned"]
        fn set_pinned(self: Pin<&mut Backend>, id: i64, on: bool);
        #[qinvokable]
        #[cxx_name = "setFavorite"]
        fn set_favorite(self: Pin<&mut Backend>, id: i64, on: bool);
        #[qinvokable]
        #[cxx_name = "setRating"]
        fn set_rating(self: Pin<&mut Backend>, id: i64, stars: i64);
        #[qinvokable]
        #[cxx_name = "setArchived"]
        fn set_archived(self: Pin<&mut Backend>, id: i64, on: bool);
        #[qinvokable]
        #[cxx_name = "trashNote"]
        fn trash_note(self: Pin<&mut Backend>, id: i64);
        #[qinvokable]
        #[cxx_name = "restoreNote"]
        fn restore_note(self: Pin<&mut Backend>, id: i64);
        #[qinvokable]
        #[cxx_name = "purgeNote"]
        fn purge_note(self: Pin<&mut Backend>, id: i64);
        #[qinvokable]
        #[cxx_name = "emptyTrash"]
        fn empty_trash(self: Pin<&mut Backend>);
        /// `tags` is a comma-separated list.
        #[qinvokable]
        #[cxx_name = "setNoteTags"]
        fn set_note_tags(self: Pin<&mut Backend>, id: i64, tags: &QString);
        #[qinvokable]
        #[cxx_name = "moveNote"]
        fn move_note(self: Pin<&mut Backend>, id: i64, folder: i64);
        /// The directory of a note's file (empty without a notes folder).
        #[qinvokable]
        #[cxx_name = "noteDir"]
        fn note_dir(self: &Backend, id: i64) -> QString;

        // Folders.
        #[qinvokable]
        fn folder(self: &Backend, id: i64) -> QString;
        #[qinvokable]
        #[cxx_name = "createFolder"]
        fn create_folder(self: Pin<&mut Backend>, parent: i64) -> i64;
        #[qinvokable]
        #[cxx_name = "setFolderTitle"]
        fn set_folder_title(self: Pin<&mut Backend>, id: i64, title: &QString);
        #[qinvokable]
        #[cxx_name = "setFolderNotes"]
        fn set_folder_notes(self: Pin<&mut Backend>, id: i64, notes: &QString);
        #[qinvokable]
        #[cxx_name = "setFolderRated"]
        fn set_folder_rated(self: Pin<&mut Backend>, id: i64, rated: bool);
        #[qinvokable]
        #[cxx_name = "setFolderIcon"]
        fn set_folder_icon(self: Pin<&mut Backend>, id: i64, name: &QString);
        #[qinvokable]
        #[cxx_name = "setFolderColor"]
        fn set_folder_color(self: Pin<&mut Backend>, id: i64, color: &QString);
        #[qinvokable]
        #[cxx_name = "moveFolderInto"]
        fn move_folder_into(self: Pin<&mut Backend>, id: i64, parent: i64);
        #[qinvokable]
        #[cxx_name = "deleteFolder"]
        fn delete_folder(self: Pin<&mut Backend>, id: i64, with_notes: bool);

        // Tags.
        #[qinvokable]
        #[cxx_name = "createTag"]
        fn create_tag(self: Pin<&mut Backend>, name: &QString, parent: &QString);
        #[qinvokable]
        #[cxx_name = "setTagColor"]
        fn set_tag_color(self: Pin<&mut Backend>, name: &QString, color: &QString);
        #[qinvokable]
        #[cxx_name = "setTagParent"]
        fn set_tag_parent(self: Pin<&mut Backend>, name: &QString, parent: &QString);
        #[qinvokable]
        #[cxx_name = "deleteTag"]
        fn delete_tag(self: Pin<&mut Backend>, name: &QString, with_children: bool);

        // Preferences and the notes folder.
        #[qinvokable]
        #[cxx_name = "setPref"]
        fn set_pref(self: Pin<&mut Backend>, key: &QString, on: bool);
        #[qinvokable]
        #[cxx_name = "setTrashDays"]
        fn set_trash_days(self: Pin<&mut Backend>, days: i32);
        /// Choose the notes folder (empty: none).
        #[qinvokable]
        #[cxx_name = "setVault"]
        fn set_vault(self: Pin<&mut Backend>, dir: &QString);
        #[qinvokable]
        #[cxx_name = "useDefaultVault"]
        fn use_default_vault(self: Pin<&mut Backend>);
        #[qinvokable]
        #[cxx_name = "syncVault"]
        fn sync_vault(self: Pin<&mut Backend>);
        /// `urls` is a JSON array of file or folder paths (or `file://` URLs).
        #[qinvokable]
        #[cxx_name = "importFiles"]
        fn import_files(self: Pin<&mut Backend>, urls: &QString, link: bool);
    }
}

pub struct BackendRust {
    core: Option<Core>,
    page: QString,
    folders: QString,
    tags: QString,
    filters: QString,
    prefs: QString,
    toast: QString,
    scope: QString,
    folder_id: i64,
    sort: QString,
}

impl Default for BackendRust {
    fn default() -> Self {
        Self {
            core: None,
            page: QString::from("{}"),
            folders: QString::from("[]"),
            tags: QString::from("[]"),
            filters: QString::from("{}"),
            prefs: QString::from("{}"),
            toast: QString::default(),
            scope: QString::from("notes"),
            folder_id: -1,
            sort: QString::from("recent"),
        }
    }
}

fn opt(id: i64) -> Option<i64> {
    (id >= 0).then_some(id)
}

/// A path from the QML file dialogs, which give `file://` URLs.
fn path_of(s: &str) -> PathBuf {
    PathBuf::from(s.strip_prefix("file://").unwrap_or(s))
}

impl qobject::Backend {
    pub fn init(mut self: Pin<&mut Self>) {
        let args: Vec<String> = std::env::args().collect();
        let after = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let db = after("--db").map_or_else(
            || organiser_core::paths::db_path(&notes_schema::META),
            PathBuf::from,
        );
        // `--vault DIR` picks a notes folder for a trial; `--vault ''` means none.
        let vault = after("--vault").map(PathBuf::from);
        match Core::open(&db, vault, None) {
            Ok(core) => {
                self.as_mut().rust_mut().core = Some(core);
                self.reload();
            }
            Err(e) => self.set_toast(QString::from(format!("cannot open the database: {e}"))),
        }
    }

    /// Run `f` on the core, then write the files and re-publish; an error is the toast.
    fn act<T>(
        mut self: Pin<&mut Self>,
        f: impl FnOnce(&mut Core) -> organiser_engine::Result<T>,
    ) -> Option<T> {
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let core = rust.core.as_mut()?;
            f(core).and_then(|v| core.flush().map(|()| v))
        };
        let out = match result {
            Ok(v) => Some(v),
            Err(e) => {
                self.as_mut().set_toast(QString::default());
                self.as_mut().set_toast(QString::from(e.to_string()));
                None
            }
        };
        self.reload();
        out
    }

    pub fn reload(mut self: Pin<&mut Self>) {
        let published = {
            let mut rust = self.as_mut().rust_mut();
            let Some(core) = rust.core.as_mut() else {
                return;
            };
            let _ = core.refresh();
            (
                core.notice.take(),
                core.scope_name(),
                core.scope_folder(),
                core.sort_name(),
                core.page_json()
                    .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
                core.folders_json(),
                core.tags_json().unwrap_or_else(|_| "[]".into()),
                core.filters_json(),
                core.prefs_json(),
            )
        };
        let (notice, scope, folder, sort, page, folders, tags, filters, prefs) = published;
        if let Some(n) = notice {
            self.as_mut().set_toast(QString::default());
            self.as_mut().set_toast(QString::from(n));
        }
        self.as_mut().set_scope(QString::from(scope));
        self.as_mut().set_folder_id(folder);
        self.as_mut().set_sort(QString::from(sort));
        self.as_mut().set_page(QString::from(page));
        self.as_mut().set_folders(QString::from(folders));
        self.as_mut().set_tags(QString::from(tags));
        self.as_mut().set_filters(QString::from(filters));
        self.as_mut().set_prefs(QString::from(prefs));
    }

    pub fn arg(&self, name: &QString) -> QString {
        let flag = format!("--{name}");
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|a| *a == flag)
            .and_then(|i| args.get(i + 1))
            .map_or_else(QString::default, |v| QString::from(v.as_str()))
    }

    pub fn show(self: Pin<&mut Self>, name: &QString, folder: i64) {
        let name = name.to_string();
        self.act(|c| {
            c.set_scope(&name, folder);
            Ok(())
        });
    }

    pub fn set_search(self: Pin<&mut Self>, text: &QString) {
        let text = text.to_string();
        self.act(|c| {
            c.text = text;
            Ok(())
        });
    }

    pub fn sort_by(self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        self.act(|c| {
            c.set_sort(&name);
            Ok(())
        });
    }

    pub fn toggle_filter(self: Pin<&mut Self>, field: &QString, value: i64) {
        let field = field.to_string();
        self.act(|c| {
            c.toggle_filter(&field, value);
            Ok(())
        });
    }

    pub fn toggle_tag_filter(self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        self.act(|c| {
            c.toggle_tag_filter(&name);
            Ok(())
        });
    }

    pub fn clear_filters(self: Pin<&mut Self>) {
        self.act(|c| {
            c.clear_filters();
            Ok(())
        });
    }

    pub fn note(&self, id: i64) -> QString {
        self.core
            .as_ref()
            .and_then(|c| c.note_json(id).ok())
            .map_or_else(|| QString::from("{}"), QString::from)
    }

    pub fn open_note(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.open_note(id));
    }

    pub fn create_note(self: Pin<&mut Self>, draft: &QString) -> i64 {
        let draft = draft.to_string();
        self.act(|c| c.create_note(&draft)).unwrap_or(-1)
    }

    pub fn edit_note(self: Pin<&mut Self>, id: i64, title: &QString, body: &QString) {
        let (t, b) = (title.to_string(), body.to_string());
        self.act(|c| c.edit_note(id, &t, &b));
    }

    pub fn set_note_color(self: Pin<&mut Self>, id: i64, color: &QString) {
        let v = color.to_string();
        self.act(|c| c.set_note_color(id, &v));
    }

    pub fn set_pinned(self: Pin<&mut Self>, id: i64, on: bool) {
        self.act(|c| c.set_pinned(id, on));
    }

    pub fn set_favorite(self: Pin<&mut Self>, id: i64, on: bool) {
        self.act(|c| c.set_favorite(id, on));
    }

    pub fn set_rating(self: Pin<&mut Self>, id: i64, stars: i64) {
        self.act(|c| c.set_rating(id, stars));
    }

    pub fn set_archived(self: Pin<&mut Self>, id: i64, on: bool) {
        self.act(|c| c.set_archived(id, on));
    }

    pub fn trash_note(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.trash(id));
    }

    pub fn restore_note(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.restore(id));
    }

    pub fn purge_note(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.purge(id));
    }

    pub fn empty_trash(self: Pin<&mut Self>) {
        self.act(|c| c.empty_trash());
    }

    pub fn set_note_tags(self: Pin<&mut Self>, id: i64, tags: &QString) {
        let list: Vec<String> = tags.to_string().split(',').map(String::from).collect();
        self.act(|c| c.set_tags(id, &list));
    }

    pub fn move_note(self: Pin<&mut Self>, id: i64, folder: i64) {
        self.act(|c| c.move_note(id, opt(folder)));
    }

    pub fn note_dir(&self, id: i64) -> QString {
        self.core
            .as_ref()
            .and_then(|c| c.note_dir(id))
            .map_or_else(QString::default, |p| QString::from(p.display().to_string()))
    }

    pub fn folder(&self, id: i64) -> QString {
        self.core
            .as_ref()
            .and_then(|c| c.folder_json(id).ok())
            .map_or_else(|| QString::from("{}"), QString::from)
    }

    pub fn create_folder(self: Pin<&mut Self>, parent: i64) -> i64 {
        self.act(|c| c.create_folder(opt(parent))).unwrap_or(-1)
    }

    pub fn set_folder_title(self: Pin<&mut Self>, id: i64, title: &QString) {
        let v = title.to_string();
        self.act(|c| c.set_folder_title(id, &v));
    }

    pub fn set_folder_notes(self: Pin<&mut Self>, id: i64, notes: &QString) {
        let v = notes.to_string();
        self.act(|c| c.set_folder_notes(id, &v));
    }

    pub fn set_folder_rated(self: Pin<&mut Self>, id: i64, rated: bool) {
        self.act(|c| c.set_folder_rated(id, rated));
    }

    pub fn set_folder_icon(self: Pin<&mut Self>, id: i64, name: &QString) {
        let v = name.to_string();
        self.act(|c| c.set_folder_icon(id, &v));
    }

    pub fn set_folder_color(self: Pin<&mut Self>, id: i64, color: &QString) {
        let v = color.to_string();
        self.act(|c| c.set_folder_color(id, &v));
    }

    pub fn move_folder_into(self: Pin<&mut Self>, id: i64, parent: i64) {
        self.act(|c| c.move_folder_into(id, opt(parent)));
    }

    pub fn delete_folder(self: Pin<&mut Self>, id: i64, with_notes: bool) {
        self.act(|c| c.delete_folder(id, with_notes));
    }

    pub fn create_tag(self: Pin<&mut Self>, name: &QString, parent: &QString) {
        let (n, p) = (name.to_string(), parent.to_string());
        self.act(|c| c.create_tag(&n, &p));
    }

    pub fn set_tag_color(self: Pin<&mut Self>, name: &QString, color: &QString) {
        let (n, col) = (name.to_string(), color.to_string());
        self.act(|c| c.set_tag_color(&n, &col));
    }

    pub fn set_tag_parent(self: Pin<&mut Self>, name: &QString, parent: &QString) {
        let (n, p) = (name.to_string(), parent.to_string());
        self.act(|c| c.set_tag_parent(&n, &p));
    }

    pub fn delete_tag(self: Pin<&mut Self>, name: &QString, with_children: bool) {
        let n = name.to_string();
        self.act(|c| c.delete_tag(&n, with_children));
    }

    pub fn set_pref(self: Pin<&mut Self>, key: &QString, on: bool) {
        let key = key.to_string();
        self.act(|c| {
            c.set_pref_bool(&key, on)
                .map_err(organiser_engine::Error::Invalid)
        });
    }

    pub fn set_trash_days(self: Pin<&mut Self>, days: i32) {
        self.act(|c| {
            c.set_trash_days(days.max(1) as u32)
                .map_err(organiser_engine::Error::Invalid)
        });
    }

    pub fn set_vault(self: Pin<&mut Self>, dir: &QString) {
        let dir = path_of(&dir.to_string());
        let dir = (!dir.as_os_str().is_empty()).then_some(dir);
        self.act(|c| c.set_vault(dir));
    }

    pub fn use_default_vault(self: Pin<&mut Self>) {
        self.act(|c| c.set_vault(Some(Core::default_vault_dir())));
    }

    pub fn sync_vault(self: Pin<&mut Self>) {
        self.act(|c| c.sync_vault());
    }

    pub fn import_files(self: Pin<&mut Self>, urls: &QString, link: bool) {
        let list: Vec<String> = serde_json::from_str(&urls.to_string()).unwrap_or_default();
        let paths: Vec<PathBuf> = list.iter().map(|s| path_of(s)).collect();
        self.act(|c| {
            let folder = match c.scope {
                notes_schema::views::Scope::Folder(id) => Some(id),
                _ => None,
            };
            c.import(&paths, link, folder)
        });
    }
}
