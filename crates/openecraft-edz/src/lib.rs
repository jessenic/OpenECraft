//! EDZ import of OpenECraft: EPLAN Data Portal archives. An EDZ file is a 7-Zip
//! archive with a `manifest.xml` and the items of its packages below `items/<type>/`:
//! EPLAN parts XML (part, connection points, drilling pattern, manufacturer), pictures
//! and macros. The parts are read with [`openecraft_parts::from_eplan_documents`];
//! pictures and macros are returned as files of their parts. The parts carry the
//! decoded 2D macro representations alongside their original EMA attachments and a
//! small copy of their picture ([`picture`]).

mod archive;
pub mod folder;
pub mod picture;

pub use archive::{import, is_edz, read, text as decode_text, Edz, EdzFile};
