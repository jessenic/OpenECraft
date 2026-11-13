//! Files of the editor. Dialogs, the browser file picker and downloads are
//! implemented in `openecraft_ui::io`.

pub use openecraft_ui::io::short_name;
use openecraft_core::archive::{self, Conversion};
use openecraft_core::model::Project;
use openecraft_core::storage::folder;
use openecraft_i18n::tr;
use std::path::Path;
use openecraft_ui::io::FileKind;
use openecraft_ui::part_editor::ModelKind;

/// What a file is opened for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Project,
    /// Import into the parts database.
    Parts,
    /// 2D / 3D model of a part.
    Model(ModelKind),
    /// Macro library (JSON) to merge.
    Macros,
    /// A project file that starts a new project (project template).
    ProjectTemplate,
    /// PLC symbol table (CSV).
    PlcTable,
    /// Symbol library (JSON) to merge into the project.
    Symbols,
    /// Title blocks (JSON) to add to the project.
    Forms,
    /// A drawing (DXF, DWG) to import as a page.
    Drawing,
    /// A table (CSV) changed in a spreadsheet.
    Table(openecraft_core::tables::TableKind),
    /// Translation dictionary (CSV) of the project texts.
    Dictionary,
    /// Another project that gets report pages of this one.
    OutputTarget,
    /// PLC hardware configuration (AutomationML).
    Aml,
    /// Device descriptions of PLC hardware (GSDML, ESI).
    DeviceDescription,
    /// Structure template (JSON) to apply to the project.
    StructureTemplate,
    /// A file to embed as a document page.
    Document,
    /// A bitmap to place on a page.
    Image,
    /// A bitmap for the selected image element of a title block or report form.
    FormImage,
    /// The logo of the project.
    Logo,
    /// Page templates (JSON) to add to the project.
    PageTemplates,
    /// A project to compare with.
    Compare,
    /// Change requests (CSV, JSON) of another system.
    ChangeRequests,
    /// A PDF export of the project with comments of a reviewer.
    PdfComments,
    /// Pre-planning segments and devices (CSV, Excel).
    Preplanning,
    /// Check scheme (JSON): checks switched off and message levels.
    CheckScheme,
    /// Settings scheme (JSON) of a project.
    SettingsScheme,
    /// Customer rule sets (JSON) to add to the project.
    RuleSets,
    /// Property library (JSON) to merge into the project.
    PropertyLibrary,
    /// Edited subproject to insert into the project.
    Subproject,
    /// A file of an ERP, PDM, PLM, ECAD, MCAD, procurement system or wiring machine.
    Integration,
    /// Ribbon customisation (JSON) exported before.
    Ribbon,
    /// Favorites of the Insert center (JSON) exported before.
    Favorites,
}

impl FileKind for Purpose {
    fn filter(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Purpose::Project => (tr("OpenECraft project or archive"), &[PROJECT_EXT, "oecz", INDEX_EXT, "json"]),
            Purpose::ProjectTemplate => (tr("OpenECraft project template"), &[PROJECT_EXT, "oecz", INDEX_EXT, "json"]),
            Purpose::Compare | Purpose::Subproject => (tr("OpenECraft project"), &[PROJECT_EXT, INDEX_EXT, "json"]),
            Purpose::OutputTarget => (tr("OpenECraft project"), &[PROJECT_EXT, "json"]),
            Purpose::ChangeRequests => (tr("Change requests (CSV, JSON)"), &["csv", "json", "txt"]),
            Purpose::PdfComments => (tr("PDF with comments"), &["pdf"]),
            Purpose::Preplanning => (tr("Pre-planning (CSV, Excel)"), &["csv", "xlsx", "txt"]),
            Purpose::CheckScheme => (tr("Check scheme (JSON)"), &["json"]),
            Purpose::SettingsScheme => (tr("Settings scheme (JSON)"), &["json"]),
            Purpose::Parts => (tr("Parts (CSV, JSON, BMEcat, EPLAN XML, EDZ)"), &["csv", "json", "xml", "txt", "edz"]),
            Purpose::Model(kind) => kind.filter(),
            Purpose::Macros => (tr("Window macros (JSON, EMA, EDZ)"), &["json", "ema", "edz"]),
            Purpose::PlcTable => ("PLC symbol table (CSV)", &["csv", "txt"]),
            Purpose::Symbols => (tr("Symbol library (JSON, EMA, EDZ)"), &["json", "ema", "edz"]),
            Purpose::Forms => (tr("OpenECraft title blocks and report schemes (JSON)"), &["json"]),
            Purpose::StructureTemplate => (tr("OpenECraft structure template (JSON)"), &["json"]),
            Purpose::PageTemplates => (tr("OpenECraft page templates (JSON)"), &["json"]),
            Purpose::RuleSets => (tr("OpenECraft rule sets (JSON)"), &["json"]),
            Purpose::PropertyLibrary => (tr("OpenECraft property library (JSON)"), &["json"]),
            Purpose::Image | Purpose::FormImage | Purpose::Logo => (tr("Images (PNG, JPEG)"), &["png", "jpg", "jpeg"]),
            Purpose::Document => (tr("Documents"), &["pdf", "docx", "doc", "xlsx", "xls", "pptx", "odt", "ods", "png", "jpg", "jpeg", "txt", "*"]),
            Purpose::Table(_) => (tr("CSV or Excel"), &["csv", "txt", "xlsx"]),
            Purpose::Dictionary => ("CSV", &["csv", "txt"]),
            Purpose::Drawing => (tr("Drawing (DXF, DWG)"), &["dxf", "dwg"]),
            Purpose::Aml => ("AutomationML", &["aml", "xml"]),
            Purpose::Integration => (tr("Files of other systems (CSV, JSON)"), &["csv", "json", "txt"]),
            Purpose::DeviceDescription => (tr("Device description (GSDML, ESI)"), &["xml"]),
            Purpose::Ribbon => (tr("Ribbon (JSON)"), &["json"]),
            Purpose::Favorites => (tr("Favorites (JSON)"), &["json"]),
        }
    }
}

pub type FileIo = openecraft_ui::io::FileIo<Purpose>;
pub type Loaded = openecraft_ui::io::Loaded<Purpose>;

pub const PROJECT_EXT: &str = "oec";
pub use openecraft_core::storage::INDEX_EXT;

/// The path names the index file of a split project.
pub fn is_split(path: &str) -> bool {
    folder::is_index(Path::new(path))
}

/// The project of a file the user opened: a project file or an archive (`true`)
/// from its content, a split project from the folder of its index file.
pub fn read_project(name: &str, bytes: &[u8]) -> Result<(Project, Conversion, bool), String> {
    if !is_split(name) {
        let (prj, conv) = archive::read_project(bytes)?;
        return Ok((prj, conv, archive::is_archive(bytes)));
    }
    if !openecraft_ui::io::has_file_system() {
        return Err(tr("A split project is a folder of files: the browser cannot open it. Convert it to a project file (.oec) or an archive (.oecz) first.").into());
    }
    let (prj, conv) = folder::open(Path::new(name))?;
    Ok((prj, conv, false))
}

/// Writes the project to `path`: a project file, or the folder of a split project
/// for an index file.
pub fn write_project(io: &FileIo, path: &str, prj: &Project) -> Result<(), String> {
    if is_split(path) {
        folder::save(Path::new(path), prj).map(|_| ())
    } else {
        io.save_to(path, &prj.to_json())
    }
}

#[cfg(test)]
mod tests;
