use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use meetinglens_lib::data::{CreateMeeting, DocumentError, DocumentStatus, MeetingService};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use uuid::Uuid;

struct SmokeWorkspace {
    root: PathBuf,
    app_data: PathBuf,
    sources: PathBuf,
}

impl SmokeWorkspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "meetinglens-native-document-smoke-{}",
            Uuid::new_v4()
        ));
        let app_data = root.join("app-data");
        let sources = root.join("sources");
        std::fs::create_dir_all(&sources).expect("smoke source directory should create");
        Self {
            root,
            app_data,
            sources,
        }
    }

    fn source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.sources.join(name);
        std::fs::write(&path, bytes).expect("smoke source should write");
        path
    }

    fn ooxml(&self, name: &str, package_path: &str) -> PathBuf {
        let path = self.sources.join(name);
        let file = File::create(&path).expect("OOXML smoke file should create");
        let mut writer = zip::ZipWriter::new(file);
        for (entry, bytes) in [
            ("[Content_Types].xml", b"synthetic types".as_slice()),
            (package_path, b"synthetic package".as_slice()),
        ] {
            writer
                .start_file(entry, zip::write::SimpleFileOptions::default())
                .expect("OOXML smoke entry should start");
            writer
                .write_all(bytes)
                .expect("OOXML smoke entry should write");
        }
        writer.finish().expect("OOXML smoke file should finish");
        path
    }
}

impl Drop for SmokeWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn native_document_storage_lifecycle_smoke() {
    let workspace = SmokeWorkspace::new();
    let meeting_id;
    let retained_source;
    let first_document_id;
    {
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("storage should initialize");
        meeting_id = meetings
            .create(CreateMeeting {
                title: "Synthetic native storage smoke".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("smoke meeting should create")
            .id;
        retained_source = workspace.source("retained.txt", b"synthetic retained source\n");
        let sources = vec![
            retained_source.clone(),
            workspace.source("synthetic.pdf", b"%PDF-1.7\nsynthetic"),
            workspace.source("synthetic.png", b"\x89PNG\r\n\x1a\nsynthetic"),
            workspace.source("synthetic.jpg", b"\xff\xd8\xff\xe0synthetic"),
            workspace.source("synthetic.md", b"# synthetic markdown\n"),
            workspace.source("synthetic.csv", b"name,value\nsynthetic,1\n"),
            workspace.ooxml("synthetic.docx", "word/document.xml"),
            workspace.ooxml("synthetic.pptx", "ppt/presentation.xml"),
            workspace.ooxml("synthetic.xlsx", "xl/workbook.xml"),
        ];

        let documents = meetings
            .documents()
            .import_batch(&meeting_id, &sources)
            .expect("allowlisted smoke files should import");
        assert_eq!(documents.len(), 9);
        assert!(documents.iter().all(|document| {
            document.status == DocumentStatus::Staged
                && document
                    .storage_relative_path
                    .starts_with("meeting-data/meetings/")
                && !document
                    .storage_relative_path
                    .contains(&document.original_filename)
        }));
        for document in &documents {
            let committed = meetings
                .storage_paths()
                .resolve_relative(&document.storage_relative_path)
                .expect("committed smoke path should resolve");
            assert!(committed.starts_with(meetings.storage_paths().root()));
            assert!(committed.is_file());
            let digest = Sha256::digest(
                std::fs::read(&committed).expect("committed smoke file should read"),
            );
            assert_eq!(
                hex(&digest),
                document.sha256.as_deref().expect("hash should exist")
            );
        }
        first_document_id = documents[0].id.clone();

        let duplicate = workspace.source("duplicate-name.txt", b"synthetic retained source\n");
        assert_eq!(
            meetings.documents().import_document(&meeting_id, duplicate),
            Err(DocumentError::AlreadyImported {
                document_id: first_document_id.clone()
            })
        );
        assert_eq!(count_files(meetings.storage_paths().meetings()), 9);

        let database = Connection::open(meetings.storage_paths().database())
            .expect("smoke database should open");
        let leaked_source_paths: i64 = database
            .query_row(
                "SELECT COUNT(*) FROM documents WHERE instr(storage_relative_path, ':') > 0",
                [],
                |row| row.get(0),
            )
            .expect("stored paths should inspect");
        assert_eq!(leaked_source_paths, 0);
    }

    let reopened =
        MeetingService::initialize(&workspace.app_data).expect("storage should survive restart");
    assert_eq!(
        reopened
            .documents()
            .list(&meeting_id)
            .expect("staged documents should persist")
            .len(),
        9
    );
    reopened
        .documents()
        .delete(&first_document_id)
        .expect("single document should delete");
    assert!(retained_source.is_file());
    reopened
        .delete(&meeting_id)
        .expect("meeting and remaining documents should delete");
    assert_eq!(count_files(reopened.storage_paths().meetings()), 0);
    assert_eq!(count_files(reopened.storage_paths().staging()), 0);
    assert_eq!(count_files(reopened.storage_paths().deletion_trash()), 0);
    let database =
        Connection::open(reopened.storage_paths().database()).expect("smoke database should open");
    let integrity: String = database
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity check should run");
    assert_eq!(integrity, "ok");
}

fn count_files(root: &Path) -> usize {
    let mut count = 0;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).expect("smoke directory should read") {
            let path = entry.expect("smoke entry should read").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                count += 1;
            }
        }
    }
    count
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}
