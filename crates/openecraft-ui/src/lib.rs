//! egui components shared by the OpenECraft applications (editor and parts manager).
//!
//! - [`faces`] – projected faces of the 3D views without spikes at sharp corners
//! - [`part_editor`] – part tree, tabbed part editor, 2D / 3D model previews
//! - [`client`] – client of the parts server (`openecraft-parts-server`)
//! - [`portal`] – data portal browser (faceted search, files, favorites, update check)
//! - [`io`] – file dialogs on desktop, file picker and downloads in the browser
//! - [`language`] – language menu (UI texts are translated with `openecraft-i18n`)
//! - [`nav`] – zoom and pan of the views by mouse wheel, trackpad and touch screen

pub mod client;
pub mod faces;
pub mod fonts;
pub mod icons;
pub mod io;
pub mod language;
pub mod logos;
pub mod nav;
pub mod part_editor;
pub mod portal;
