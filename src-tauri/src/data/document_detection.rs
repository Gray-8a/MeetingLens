use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use super::document_error::{DocumentError, DocumentResult};
use super::document_model::ALLOWED_DOCUMENT_EXTENSIONS;

const MAX_OOXML_ENTRIES: usize = 512;
const MAX_OOXML_CENTRAL_DIRECTORY_BYTES: u64 = 256 * 1024;
const MAX_OOXML_ENTRY_NAME_BYTES: usize = 260;
const MAX_OOXML_TOTAL_NAME_BYTES: usize = 64 * 1024;
const MAX_ZIP_EOCD_SEARCH_BYTES: u64 = 65_557;

pub(crate) struct SourceName {
    pub original_filename: String,
    pub safe_extension: String,
}

pub(crate) fn validate_source_name(
    path: &Path,
    max_filename_bytes: usize,
) -> DocumentResult<SourceName> {
    let original_filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(DocumentError::InvalidType)?
        .to_owned();
    if original_filename.is_empty()
        || original_filename.len() > max_filename_bytes
        || original_filename.chars().any(char::is_control)
    {
        return Err(DocumentError::FilenameTooLong);
    }

    let safe_extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|value| is_allowed_extension(value))
        .ok_or(DocumentError::InvalidType)?;

    if has_disguised_double_extension(path) {
        return Err(DocumentError::InvalidType);
    }

    Ok(SourceName {
        original_filename,
        safe_extension,
    })
}

pub(crate) fn detect_media_type(path: &Path, extension: &str) -> DocumentResult<&'static str> {
    match extension {
        "pdf" => detect_signature(path, b"%PDF-", "application/pdf"),
        "png" => detect_signature(path, b"\x89PNG\r\n\x1a\n", "image/png"),
        "jpg" | "jpeg" => detect_jpeg(path),
        "docx" | "pptx" | "xlsx" => detect_ooxml(path, extension),
        "txt" => detect_utf8_text(path, "text/plain"),
        "md" => detect_utf8_text(path, "text/markdown"),
        "csv" => detect_utf8_text(path, "text/csv"),
        _ => Err(DocumentError::InvalidType),
    }
}

fn detect_signature(
    path: &Path,
    expected: &[u8],
    media_type: &'static str,
) -> DocumentResult<&'static str> {
    let mut file = File::open(path).map_err(|_| DocumentError::StorageUnavailable)?;
    let mut actual = vec![0_u8; expected.len()];
    file.read_exact(&mut actual)
        .map_err(|_| DocumentError::TypeMismatch)?;
    if actual == expected {
        Ok(media_type)
    } else {
        Err(DocumentError::TypeMismatch)
    }
}

fn detect_jpeg(path: &Path) -> DocumentResult<&'static str> {
    let mut file = File::open(path).map_err(|_| DocumentError::StorageUnavailable)?;
    let mut signature = [0_u8; 3];
    file.read_exact(&mut signature)
        .map_err(|_| DocumentError::TypeMismatch)?;
    if signature[0] == 0xff && signature[1] == 0xd8 && signature[2] == 0xff {
        Ok("image/jpeg")
    } else {
        Err(DocumentError::TypeMismatch)
    }
}

fn detect_ooxml(path: &Path, extension: &str) -> DocumentResult<&'static str> {
    let mut file = File::open(path).map_err(|_| DocumentError::StorageUnavailable)?;
    validate_zip_directory_bounds(&mut file)?;
    let reader = BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader).map_err(|_| DocumentError::TypeMismatch)?;
    if archive.is_empty() || archive.len() > MAX_OOXML_ENTRIES {
        return Err(DocumentError::TypeMismatch);
    }

    let required_prefix = match extension {
        "docx" => "word/",
        "pptx" => "ppt/",
        "xlsx" => "xl/",
        _ => return Err(DocumentError::InvalidType),
    };
    let mut has_content_types = false;
    let mut has_required_prefix = false;
    let mut total_name_bytes = 0_usize;

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| DocumentError::TypeMismatch)?;
        let name = entry.name();
        total_name_bytes = total_name_bytes.saturating_add(name.len());
        if name.len() > MAX_OOXML_ENTRY_NAME_BYTES
            || total_name_bytes > MAX_OOXML_TOTAL_NAME_BYTES
            || is_unsafe_zip_name(name)
            || name.to_ascii_lowercase().ends_with("vbaproject.bin")
        {
            return Err(DocumentError::TypeMismatch);
        }
        has_content_types |= name == "[Content_Types].xml";
        has_required_prefix |= name.starts_with(required_prefix);
    }

    if !has_content_types || !has_required_prefix {
        return Err(DocumentError::TypeMismatch);
    }

    Ok(match extension {
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => unreachable!("extension was validated above"),
    })
}

fn validate_zip_directory_bounds(file: &mut File) -> DocumentResult<()> {
    let file_len = file
        .metadata()
        .map_err(|_| DocumentError::StorageUnavailable)?
        .len();
    let search_len = file_len.min(MAX_ZIP_EOCD_SEARCH_BYTES);
    if search_len < 22 {
        return Err(DocumentError::TypeMismatch);
    }
    file.seek(SeekFrom::End(-(search_len as i64)))
        .map_err(|_| DocumentError::TypeMismatch)?;
    let mut tail = vec![0_u8; search_len as usize];
    file.read_exact(&mut tail)
        .map_err(|_| DocumentError::TypeMismatch)?;
    let eocd = tail
        .windows(4)
        .rposition(|window| window == b"PK\x05\x06")
        .ok_or(DocumentError::TypeMismatch)?;
    if eocd + 22 > tail.len() {
        return Err(DocumentError::TypeMismatch);
    }
    let disk_number = u16::from_le_bytes([tail[eocd + 4], tail[eocd + 5]]);
    let directory_disk = u16::from_le_bytes([tail[eocd + 6], tail[eocd + 7]]);
    let entries_on_disk = u16::from_le_bytes([tail[eocd + 8], tail[eocd + 9]]);
    let entries = u16::from_le_bytes([tail[eocd + 10], tail[eocd + 11]]);
    let directory_size = u64::from(u32::from_le_bytes([
        tail[eocd + 12],
        tail[eocd + 13],
        tail[eocd + 14],
        tail[eocd + 15],
    ]));
    let directory_offset = u64::from(u32::from_le_bytes([
        tail[eocd + 16],
        tail[eocd + 17],
        tail[eocd + 18],
        tail[eocd + 19],
    ]));
    let comment_len = usize::from(u16::from_le_bytes([tail[eocd + 20], tail[eocd + 21]]));
    if disk_number != 0
        || directory_disk != 0
        || entries_on_disk != entries
        || entries == u16::MAX
        || usize::from(entries) > MAX_OOXML_ENTRIES
        || directory_size > MAX_OOXML_CENTRAL_DIRECTORY_BYTES
        || directory_offset.saturating_add(directory_size) > file_len
        || eocd + 22 + comment_len != tail.len()
    {
        return Err(DocumentError::TypeMismatch);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| DocumentError::TypeMismatch)?;
    Ok(())
}

fn detect_utf8_text(path: &Path, media_type: &'static str) -> DocumentResult<&'static str> {
    let file = File::open(path).map_err(|_| DocumentError::StorageUnavailable)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 64 * 1024];
    let mut pending = Vec::with_capacity(4);
    let mut first = true;

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| DocumentError::IntegrityFailed)?;
        if read == 0 {
            break;
        }
        let mut bytes = Vec::with_capacity(pending.len() + read);
        bytes.extend_from_slice(&pending);
        bytes.extend_from_slice(&buffer[..read]);
        pending.clear();

        let bytes = if first && bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            &bytes[3..]
        } else {
            &bytes[..]
        };
        first = false;

        match std::str::from_utf8(bytes) {
            Ok(text) => validate_text_characters(text)?,
            Err(error) if error.error_len().is_none() => {
                let valid = &bytes[..error.valid_up_to()];
                validate_text_characters(
                    std::str::from_utf8(valid).map_err(|_| DocumentError::TypeMismatch)?,
                )?;
                pending.extend_from_slice(&bytes[error.valid_up_to()..]);
                if pending.len() > 3 {
                    return Err(DocumentError::TypeMismatch);
                }
            }
            Err(_) => return Err(DocumentError::TypeMismatch),
        }
    }

    if !pending.is_empty() {
        return Err(DocumentError::TypeMismatch);
    }
    Ok(media_type)
}

fn validate_text_characters(text: &str) -> DocumentResult<()> {
    if text
        .chars()
        .any(|value| value == '\0' || (value.is_control() && !matches!(value, '\t' | '\r' | '\n')))
    {
        Err(DocumentError::TypeMismatch)
    } else {
        Ok(())
    }
}

fn is_allowed_extension(value: &str) -> bool {
    ALLOWED_DOCUMENT_EXTENSIONS.contains(&value)
}

fn has_disguised_double_extension(path: &Path) -> bool {
    const DENIED: &[&str] = &[
        "bat", "cmd", "com", "dll", "doc", "docm", "exe", "htm", "html", "js", "msi", "pif", "ppt",
        "pptm", "ps1", "rar", "scr", "vbs", "xls", "xlsm", "zip", "7z",
    ];
    path.file_stem()
        .and_then(|value| value.to_str())
        .and_then(|value| value.rsplit_once('.'))
        .is_some_and(|(_, extension)| DENIED.contains(&extension.to_ascii_lowercase().as_str()))
}

fn is_unsafe_zip_name(name: &str) -> bool {
    name.is_empty()
        || name.starts_with('/')
        || name.starts_with('\\')
        || name.contains(':')
        || name.split(['/', '\\']).any(|component| component == "..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use uuid::Uuid;

    struct TestFile(std::path::PathBuf);

    impl TestFile {
        fn new(extension: &str, bytes: &[u8]) -> Self {
            let path = std::env::temp_dir().join(format!(
                "meetinglens-detection-{}.{}",
                Uuid::new_v4(),
                extension
            ));
            std::fs::write(&path, bytes).expect("synthetic file should write");
            Self(path)
        }
    }

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn accepts_utf8_with_optional_bom_and_safe_controls() {
        let plain = TestFile::new("txt", "hello\tworld\r\n".as_bytes());
        let bom = TestFile::new("md", b"\xef\xbb\xbf# synthetic\n");
        assert_eq!(detect_media_type(&plain.0, "txt"), Ok("text/plain"));
        assert_eq!(detect_media_type(&bom.0, "md"), Ok("text/markdown"));
    }

    #[test]
    fn rejects_invalid_utf8_nul_and_unsafe_controls() {
        for bytes in [
            b"\xff".as_slice(),
            b"caf\xe9",
            b"\xff\xfeh\0",
            b"\xff\xfe\0\0h\0\0\0",
            b"hello\0world",
            b"hello\x07world",
        ] {
            let file = TestFile::new("txt", bytes);
            assert_eq!(
                detect_media_type(&file.0, "txt"),
                Err(DocumentError::TypeMismatch)
            );
        }
    }

    #[test]
    fn rejects_disguised_double_extensions() {
        let directory = std::env::temp_dir();
        let path = directory.join("synthetic.exe.pdf");
        assert!(matches!(
            validate_source_name(&path, 255),
            Err(DocumentError::InvalidType)
        ));
        assert!(validate_source_name(&directory.join("synthetic.v2.pdf"), 255).is_ok());
    }

    #[test]
    fn filename_limit_is_measured_in_utf8_bytes() {
        let name = format!("{}.txt", "文".repeat(84));
        assert_eq!(name.chars().count(), 88);
        assert_eq!(name.len(), 256);
        assert!(matches!(
            validate_source_name(Path::new(&name), 255),
            Err(DocumentError::FilenameTooLong)
        ));
    }

    #[test]
    fn detects_pdf_png_and_jpeg_signatures() {
        let pdf = TestFile::new("pdf", b"%PDF-1.7\nsynthetic");
        let png = TestFile::new("png", b"\x89PNG\r\n\x1a\nsynthetic");
        let jpeg = TestFile::new("jpg", b"\xff\xd8\xff\xe0synthetic");
        assert_eq!(detect_media_type(&pdf.0, "pdf"), Ok("application/pdf"));
        assert_eq!(detect_media_type(&png.0, "png"), Ok("image/png"));
        assert_eq!(detect_media_type(&jpeg.0, "jpg"), Ok("image/jpeg"));
    }

    #[test]
    fn rejects_signature_mismatches() {
        let fake = TestFile::new("pdf", b"not a PDF");
        assert_eq!(
            detect_media_type(&fake.0, "pdf"),
            Err(DocumentError::TypeMismatch)
        );
    }

    #[test]
    fn inspects_ooxml_central_directory_without_extracting_payloads() {
        let valid = TestFile::new("docx", b"");
        write_zip(
            &valid.0,
            &[
                ("[Content_Types].xml", b"synthetic"),
                ("word/document.xml", b"synthetic document"),
            ],
        );
        assert_eq!(
            detect_media_type(&valid.0, "docx"),
            Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );

        for entries in [
            vec![
                ("[Content_Types].xml", b"synthetic".as_slice()),
                ("word/vbaProject.bin", b"macro".as_slice()),
            ],
            vec![
                ("[Content_Types].xml", b"synthetic".as_slice()),
                ("../word/document.xml", b"unsafe".as_slice()),
            ],
            vec![
                ("[Content_Types].xml", b"synthetic".as_slice()),
                ("ppt/presentation.xml", b"wrong package".as_slice()),
            ],
        ] {
            let invalid = TestFile::new("docx", b"");
            write_zip(&invalid.0, &entries);
            assert_eq!(
                detect_media_type(&invalid.0, "docx"),
                Err(DocumentError::TypeMismatch)
            );
        }
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(path).expect("zip should create");
        let mut writer = zip::ZipWriter::new(file);
        for (name, bytes) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .expect("entry should start");
            writer.write_all(bytes).expect("entry should write");
        }
        writer.finish().expect("zip should finish");
    }
}
