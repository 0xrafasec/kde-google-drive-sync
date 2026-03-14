//! Integration tests for Drive API client against wiremock.

use gds_core::api::{
    CreateFileMetadata, DriveClient, UpdateFileMetadata, CHANGES_FIELDS, FILE_FIELDS,
};
use gds_core::model::Config;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config() -> Config {
    Config::default()
}

async fn client_for(server: &MockServer) -> DriveClient {
    let base = server.uri().trim_end_matches('/').to_string();
    DriveClient::with_base_url(base, &test_config()).unwrap()
}

const TOKEN: &str = "test-access-token";

#[tokio::test]
async fn files_list_returns_files_and_next_page_token() {
    let server = MockServer::start().await;
    let list_body = serde_json::json!({
        "nextPageToken": "next-abc",
        "files": [
            {
                "id": "f1",
                "name": "a.txt",
                "mimeType": "text/plain",
                "md5Checksum": null,
                "size": "10",
                "modifiedTime": "2024-01-01T00:00:00.000Z",
                "parents": ["root"],
                "trashed": false
            }
        ]
    });
    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .and(query_param("fields", FILE_FIELDS))
        .respond_with(ResponseTemplate::new(200).set_body_json(list_body))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let res = client
        .files_list(TOKEN, None, None, None, None, FILE_FIELDS)
        .await
        .unwrap();
    assert_eq!(res.next_page_token.as_deref(), Some("next-abc"));
    assert_eq!(res.files.len(), 1);
    assert_eq!(res.files[0].id, "f1");
    assert_eq!(res.files[0].name, "a.txt");
}

#[tokio::test]
async fn files_get_returns_metadata() {
    let server = MockServer::start().await;
    let file_body = serde_json::json!({
        "id": "f99",
        "name": "doc.pdf",
        "mimeType": "application/pdf",
        "md5Checksum": "abc123",
        "size": "1024",
        "modifiedTime": "2024-06-01T12:00:00.000Z",
        "parents": ["folder1"],
        "trashed": false
    });
    Mock::given(method("GET"))
        .and(path("/drive/v3/files/f99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(file_body))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let file = client.files_get(TOKEN, "f99", FILE_FIELDS).await.unwrap();
    assert_eq!(file.id, "f99");
    assert_eq!(file.name, "doc.pdf");
    assert_eq!(file.md5_checksum.as_deref(), Some("abc123"));
}

#[tokio::test]
async fn files_get_media_streams_to_writer() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drive/v3/files/f42"))
        .and(query_param("alt", "media"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file content here"))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let mut buf = Vec::new();
    client
        .files_get_media(TOKEN, "f42", &mut buf)
        .await
        .unwrap();
    assert_eq!(buf, b"file content here");
}

#[tokio::test]
async fn files_export_streams_exported_content() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drive/v3/files/doc1/export"))
        .and(query_param(
            "mimeType",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"docx content"))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let mut buf = Vec::new();
    client
        .files_export(
            TOKEN,
            "doc1",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            &mut buf,
        )
        .await
        .unwrap();
    assert_eq!(buf, b"docx content");
}

#[tokio::test]
async fn files_create_simple_returns_created_file() {
    let server = MockServer::start().await;
    let created = serde_json::json!({
        "id": "new-id",
        "name": "new.txt",
        "mimeType": "text/plain",
        "md5Checksum": null,
        "size": "5",
        "modifiedTime": "2024-01-01T00:00:00.000Z",
        "parents": ["folder1"],
        "trashed": false
    });
    Mock::given(method("POST"))
        .and(path("/upload/drive/v3/files"))
        .and(query_param("uploadType", "multipart"))
        .respond_with(ResponseTemplate::new(200).set_body_json(created))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let meta = CreateFileMetadata {
        name: Some("new.txt".to_string()),
        mime_type: Some("text/plain".to_string()),
        parents: Some(vec!["folder1".to_string()]),
    };
    let file = client
        .files_create_simple(TOKEN, &meta, b"hello", "text/plain")
        .await
        .unwrap();
    assert_eq!(file.id, "new-id");
    assert_eq!(file.name, "new.txt");
}

#[tokio::test]
async fn files_create_resumable_init_then_chunks_then_200() {
    let server = MockServer::start().await;
    let created = serde_json::json!({
        "id": "resumable-id",
        "name": "big.bin",
        "mimeType": "application/octet-stream",
        "md5Checksum": null,
        "size": "100",
        "modifiedTime": "2024-01-01T00:00:00.000Z",
        "parents": [],
        "trashed": false
    });

    Mock::given(method("POST"))
        .and(path("/upload/drive/v3/files"))
        .and(query_param("uploadType", "resumable"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Location", format!("{}/upload/session/xyz", server.uri())),
        )
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/upload/session/xyz"))
        .respond_with(ResponseTemplate::new(201).set_body_json(created))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let meta = CreateFileMetadata {
        name: Some("big.bin".to_string()),
        mime_type: Some("application/octet-stream".to_string()),
        parents: None,
    };
    let mut content: &[u8] = b"small content for test";
    let file = client
        .files_create_resumable(
            TOKEN,
            &meta,
            21,
            "application/octet-stream",
            &mut content,
            None,
        )
        .await
        .unwrap();
    assert_eq!(file.id, "resumable-id");
}

#[tokio::test]
async fn files_update_metadata_returns_updated_file() {
    let server = MockServer::start().await;
    let updated = serde_json::json!({
        "id": "f1",
        "name": "renamed.txt",
        "mimeType": "text/plain",
        "md5Checksum": null,
        "size": "0",
        "modifiedTime": "2024-01-01T00:00:00.000Z",
        "parents": ["other"],
        "trashed": false
    });
    Mock::given(method("PATCH"))
        .and(path("/drive/v3/files/f1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(updated))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let meta = UpdateFileMetadata {
        name: Some("renamed.txt".to_string()),
        parents: Some(vec!["other".to_string()]),
        trashed: None,
    };
    let file = client
        .files_update_metadata(TOKEN, "f1", &meta, FILE_FIELDS)
        .await
        .unwrap();
    assert_eq!(file.name, "renamed.txt");
}

#[tokio::test]
async fn files_delete_returns_204() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/drive/v3/files/f1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    client.files_delete(TOKEN, "f1").await.unwrap();
}

#[tokio::test]
async fn files_copy_returns_copied_file() {
    let server = MockServer::start().await;
    let copied = serde_json::json!({
        "id": "copy-id",
        "name": "copy.txt",
        "mimeType": "text/plain",
        "md5Checksum": null,
        "size": "0",
        "modifiedTime": "2024-01-01T00:00:00.000Z",
        "parents": ["dest"],
        "trashed": false
    });
    Mock::given(method("POST"))
        .and(path("/drive/v3/files/src1/copy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(copied))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let file = client
        .files_copy(TOKEN, "src1", Some("copy.txt"), Some(&["dest".to_string()]))
        .await
        .unwrap();
    assert_eq!(file.id, "copy-id");
    assert_eq!(file.name, "copy.txt");
}

#[tokio::test]
async fn changes_get_start_page_token_returns_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drive/v3/changes/startPageToken"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "startPageToken": "token-123" })),
        )
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let token = client
        .changes_get_start_page_token(TOKEN, None)
        .await
        .unwrap();
    assert_eq!(token, "token-123");
}

#[tokio::test]
async fn changes_list_returns_change_set() {
    let server = MockServer::start().await;
    let body = serde_json::json!({
        "nextPageToken": "page2",
        "newStartPageToken": null,
        "changes": [
            {
                "changeType": "file",
                "fileId": "f1",
                "file": {
                    "id": "f1",
                    "name": "x.txt",
                    "mimeType": "text/plain",
                    "md5Checksum": null,
                    "size": null,
                    "modifiedTime": null,
                    "parents": null,
                    "trashed": null
                },
                "removed": false
            }
        ]
    });
    Mock::given(method("GET"))
        .and(path("/drive/v3/changes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let set = client
        .changes_list(TOKEN, "page1", None, CHANGES_FIELDS, false, false)
        .await
        .unwrap();
    assert_eq!(set.next_page_token.as_deref(), Some("page2"));
    assert_eq!(set.changes.len(), 1);
    assert_eq!(set.changes[0].file_id, "f1");
}

#[tokio::test]
async fn about_get_returns_quota() {
    let server = MockServer::start().await;
    let body = serde_json::json!({
        "user": { "displayName": "Test", "emailAddress": "test@example.com" },
        "storageQuota": { "limit": "10737418240", "usage": "1024", "usageInDrive": "512" }
    });
    Mock::given(method("GET"))
        .and(path("/drive/v3/about"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let about = client.about_get(TOKEN, "user,storageQuota").await.unwrap();
    assert_eq!(
        about.user.as_ref().and_then(|u| u.email_address.as_deref()),
        Some("test@example.com")
    );
    assert_eq!(
        about
            .storage_quota
            .as_ref()
            .and_then(|q| q.usage.as_deref()),
        Some("1024")
    );
}

#[tokio::test]
async fn drive_list_stub_returns_empty() {
    let server = MockServer::start().await;
    let client = client_for(&server).await;
    let res = client.drive_list(TOKEN, None, None).await.unwrap();
    assert!(res.drives.is_empty());
    assert!(res.next_page_token.is_none());
}

#[tokio::test]
async fn retry_on_429_then_success() {
    let server = MockServer::start().await;
    let file_body = serde_json::json!({
        "id": "f1",
        "name": "a.txt",
        "mimeType": "text/plain",
        "md5Checksum": null,
        "size": null,
        "modifiedTime": null,
        "parents": null,
        "trashed": null
    });

    Mock::given(method("GET"))
        .and(path("/drive/v3/files/f1"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files/f1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(file_body))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let file = client.files_get(TOKEN, "f1", FILE_FIELDS).await.unwrap();
    assert_eq!(file.id, "f1");
}

#[tokio::test]
async fn resumable_upload_resume_after_308() {
    let server = MockServer::start().await;
    let created = serde_json::json!({
        "id": "resumed-id",
        "name": "resumed.bin",
        "mimeType": "application/octet-stream",
        "md5Checksum": null,
        "size": "20",
        "modifiedTime": "2024-01-01T00:00:00.000Z",
        "parents": [],
        "trashed": false
    });

    Mock::given(method("POST"))
        .and(path("/upload/drive/v3/files"))
        .and(query_param("uploadType", "resumable"))
        .respond_with(ResponseTemplate::new(200).insert_header(
            "Location",
            format!("{}/upload/session/resume", server.uri()),
        ))
        .mount(&server)
        .await;

    // First PUT: 308 Resume Incomplete (server received bytes 0-9); match once so second PUT gets 201
    Mock::given(method("PUT"))
        .and(path("/upload/session/resume"))
        .respond_with(ResponseTemplate::new(308).insert_header("Range", "bytes=0-9"))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second PUT: 201 with body
    Mock::given(method("PUT"))
        .and(path("/upload/session/resume"))
        .respond_with(ResponseTemplate::new(201).set_body_json(created))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let meta = CreateFileMetadata {
        name: Some("resumed.bin".to_string()),
        mime_type: Some("application/octet-stream".to_string()),
        parents: None,
    };
    let mut content: &[u8] = b"0123456789abcdefghij"; // 20 bytes
    let file = client
        .files_create_resumable(
            TOKEN,
            &meta,
            20,
            "application/octet-stream",
            &mut content,
            None,
        )
        .await
        .unwrap();
    assert_eq!(file.id, "resumed-id");
}
