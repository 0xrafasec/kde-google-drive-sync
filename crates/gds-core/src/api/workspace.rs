//! Google Workspace MIME type detection and export routing (Docs, Sheets, Slides, etc.).

/// Google Docs.
pub const MIME_GOOGLE_DOCUMENT: &str = "application/vnd.google-apps.document";
/// Google Sheets.
pub const MIME_GOOGLE_SPREADSHEET: &str = "application/vnd.google-apps.spreadsheet";
/// Google Slides.
pub const MIME_GOOGLE_PRESENTATION: &str = "application/vnd.google-apps.presentation";
/// Google Drawings.
pub const MIME_GOOGLE_DRAWING: &str = "application/vnd.google-apps.drawing";
/// Google Apps Script.
pub const MIME_GOOGLE_SCRIPT: &str = "application/vnd.google-apps.script";

/// Export MIME type for Google Docs (Word).
pub const EXPORT_DOCX: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
/// Export MIME type for Google Sheets (Excel).
pub const EXPORT_XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
/// Export MIME type for Google Slides (PowerPoint).
pub const EXPORT_PPTX: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation";
/// Export MIME type for Google Drawings.
pub const EXPORT_SVG: &str = "image/svg+xml";
/// Export MIME type for Google Apps Script.
pub const EXPORT_SCRIPT_JSON: &str = "application/vnd.google-apps.script+json";

/// Returns the export MIME type for Google Workspace files that must be exported (not downloaded as binary).
/// Returns `None` for non–Google Workspace types or regular files (they use `alt=media`).
pub fn export_mime_type(drive_mime: &str) -> Option<&'static str> {
    match drive_mime {
        MIME_GOOGLE_DOCUMENT => Some(EXPORT_DOCX),
        MIME_GOOGLE_SPREADSHEET => Some(EXPORT_XLSX),
        MIME_GOOGLE_PRESENTATION => Some(EXPORT_PPTX),
        MIME_GOOGLE_DRAWING => Some(EXPORT_SVG),
        MIME_GOOGLE_SCRIPT => Some(EXPORT_SCRIPT_JSON),
        _ => None,
    }
}

/// Returns true if the Drive MIME type is a Google Workspace native type (Docs, Sheets, Slides, etc.).
pub fn is_google_workspace_file(drive_mime: &str) -> bool {
    export_mime_type(drive_mime).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_export_docx() {
        assert_eq!(export_mime_type(MIME_GOOGLE_DOCUMENT), Some(EXPORT_DOCX));
    }

    #[test]
    fn sheet_export_xlsx() {
        assert_eq!(export_mime_type(MIME_GOOGLE_SPREADSHEET), Some(EXPORT_XLSX));
    }

    #[test]
    fn slides_export_pptx() {
        assert_eq!(
            export_mime_type(MIME_GOOGLE_PRESENTATION),
            Some(EXPORT_PPTX)
        );
    }

    #[test]
    fn drawing_export_svg() {
        assert_eq!(export_mime_type(MIME_GOOGLE_DRAWING), Some(EXPORT_SVG));
    }

    #[test]
    fn script_export_json() {
        assert_eq!(
            export_mime_type(MIME_GOOGLE_SCRIPT),
            Some(EXPORT_SCRIPT_JSON)
        );
    }

    #[test]
    fn plain_file_no_export() {
        assert_eq!(export_mime_type("text/plain"), None);
        assert_eq!(export_mime_type("application/pdf"), None);
    }

    #[test]
    fn is_workspace() {
        assert!(is_google_workspace_file(MIME_GOOGLE_DOCUMENT));
        assert!(!is_google_workspace_file("application/pdf"));
    }
}
