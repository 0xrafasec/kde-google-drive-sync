//! Google Workspace stub files: .gdoc, .gsheet, .gslides (shortcut with URL).

/// MIME types for Google Workspace (must match api::workspace).
const MIME_GOOGLE_DOCUMENT: &str = "application/vnd.google-apps.document";
const MIME_GOOGLE_SPREADSHEET: &str = "application/vnd.google-apps.spreadsheet";
const MIME_GOOGLE_PRESENTATION: &str = "application/vnd.google-apps.presentation";

/// URL prefix for Google Docs.
pub const DOCS_URL_PREFIX: &str = "https://docs.google.com/document/d/";
/// URL prefix for Google Sheets.
pub const SHEETS_URL_PREFIX: &str = "https://docs.google.com/spreadsheets/d/";
/// URL prefix for Google Slides.
pub const SLIDES_URL_PREFIX: &str = "https://docs.google.com/presentation/d/";
const URL_SUFFIX: &str = "/edit";

/// Returns the browser URL for a Google Workspace file (Docs, Sheets, Slides).
pub fn workspace_file_url(drive_file_id: &str, mime_type: &str) -> Option<String> {
    let prefix = match mime_type {
        MIME_GOOGLE_DOCUMENT => DOCS_URL_PREFIX,
        MIME_GOOGLE_SPREADSHEET => SHEETS_URL_PREFIX,
        MIME_GOOGLE_PRESENTATION => SLIDES_URL_PREFIX,
        _ => return None,
    };
    Some(format!("{}{}{}", prefix, drive_file_id, URL_SUFFIX))
}

/// Content for a .gdoc stub file (one line: URL).
pub fn gdoc_stub_content(drive_file_id: &str) -> String {
    format!("{}{}{}\n", DOCS_URL_PREFIX, drive_file_id, URL_SUFFIX)
}

/// Content for a .gsheet stub file.
pub fn gsheet_stub_content(drive_file_id: &str) -> String {
    format!("{}{}{}\n", SHEETS_URL_PREFIX, drive_file_id, URL_SUFFIX)
}

/// Content for a .gslides stub file.
pub fn gslides_stub_content(drive_file_id: &str) -> String {
    format!("{}{}{}\n", SLIDES_URL_PREFIX, drive_file_id, URL_SUFFIX)
}

/// Returns stub file content for the given MIME type, or None for non-Workspace types.
pub fn stub_content_for_mime(drive_file_id: &str, mime_type: &str) -> Option<String> {
    let url = workspace_file_url(drive_file_id, mime_type)?;
    Some(format!("{}\n", url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gdoc_url() {
        let u = workspace_file_url("abc123", "application/vnd.google-apps.document").unwrap();
        assert_eq!(u, "https://docs.google.com/document/d/abc123/edit");
    }

    #[test]
    fn gdoc_stub() {
        let c = gdoc_stub_content("xyz");
        assert!(c.contains("docs.google.com/document/d/xyz/edit"));
    }

    #[test]
    fn plain_file_no_stub() {
        assert!(stub_content_for_mime("id", "text/plain").is_none());
    }
}
