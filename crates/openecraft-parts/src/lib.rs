//! Shared parts model of OpenECraft, used by the editor and by the parts server.
//!
//! - [`Part`], [`FunctionTemplate`], [`PartsDb`] … – the part data (EPLAN style
//!   product groups, commercial / technical / mounting data, function templates)
//! - part tree filter and grouping ([`PartFilter`])
//! - CSV / JSON import and export ([`to_csv`], [`from_csv`], [`import`]), BMEcat
//!   catalogs with ETIM / eCl@ss features ([`from_bmecat`]), EPLAN parts XML
//!   ([`from_eplan_xml`], [`to_eplan_xml`])
//! - 2D / 3D models of parts with SVG, STL, OBJ and IFC import ([`Model2d`], [`Mesh3d`])
//! - the generic example catalog ([`default_catalog`]) and the manufacturer catalogs
//!   shipped with OpenECraft ([`bundled`])
//! - role based access control of the servers ([`access`])
//! - the HTTP API types of the parts server ([`api`]), its data portal model
//!   (catalogs, lifecycle, attachments, users, faceted search, update check: [`portal`])
//! - accessories that fit other parts ([`accessory`])
//! - the EPLAN product groups with their exchange numbers ([`productgroups`])
//! - pin layouts and mating faces of plug connectors ([`connector`])

pub mod access;
pub mod accessory;
pub mod api;
pub mod base64;
mod bmecat;
pub mod bundled;
mod catalog;
mod connection;
pub mod connector;
mod csv;
mod eplan;
mod filter;
mod graphics;
pub mod mechanics;
mod ifc;
mod mounting;
mod part;
pub mod portal;
pub mod productgroups;
mod shared;
mod text;
pub mod vec3;
pub mod xml;

pub use accessory::AccessoryMount;
pub use bmecat::{from_bmecat, is_bmecat};
pub use catalog::{add_generated_models, default_catalog, nozzles, shaft, Nozzle, Shaft, GLAND_GAP};
pub use connection::{default_connection_points, points_from_text, points_to_text, upper_side, ConnectionPoint, Exit};
pub use csv::{csv_escape, from_csv, import, parse_csv, to_csv};
pub use eplan::{from_eplan_documents, from_eplan_xml, is_eplan_xml, language_text, to_eplan_xml};
pub use filter::{distinct_values, Grouping, PartFilter};
pub use ifc::parse_ifc;
pub use mounting::{cutout_outline, cutouts_text, parse_cutouts, Cutout, MountingData, PlacedCutout, RailMount, MOUNTING_KEYS};
pub use graphics::{parse_mesh, parse_obj, parse_stl, parse_svg, Mesh3d, MeshBuilder, Model2d, Prim2d, MAX_TRIANGLES};
pub use part::*;
pub use shared::Shared;
pub use text::{date_from_unix, natural_cmp, unix_from_date, wildcard};
