use std::path::{Component, Path, PathBuf};

use super::error::{DataError, DataResult};

const DATABASE_FILE_NAME: &str = "meetinglens.sqlite3";
const MEETING_DATA_DIRECTORY: &str = "meeting-data";
const STORAGE_LOCK_FILE_NAME: &str = ".meetinglens-storage.lock";

#[derive(Clone, Debug)]
pub struct StoragePaths {
    root: PathBuf,
    database: PathBuf,
    meeting_data: PathBuf,
    meetings: PathBuf,
    staging: PathBuf,
    deletion_trash: PathBuf,
    storage_lock: PathBuf,
}

impl StoragePaths {
    pub fn initialize(root: impl AsRef<Path>) -> DataResult<Self> {
        let requested_root = root.as_ref();
        if requested_root.as_os_str().is_empty() {
            return Err(DataError::UnsafePath);
        }

        std::fs::create_dir_all(requested_root).map_err(|_| DataError::StorageUnavailable)?;
        let root = requested_root
            .canonicalize()
            .map_err(|_| DataError::StorageUnavailable)?;
        let database = root.join(DATABASE_FILE_NAME);
        let meeting_data = root.join(MEETING_DATA_DIRECTORY);
        let meetings = meeting_data.join("meetings");
        let staging = meeting_data.join("staging");
        let deletion_trash = meeting_data.join("deletion-trash");
        for directory in [&meeting_data, &meetings, &staging, &deletion_trash] {
            std::fs::create_dir_all(directory).map_err(|_| DataError::StorageUnavailable)?;
        }
        let storage_lock = meeting_data.join(STORAGE_LOCK_FILE_NAME);

        Ok(Self {
            root,
            database,
            meeting_data,
            meetings,
            staging,
            deletion_trash,
            storage_lock,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database(&self) -> &Path {
        &self.database
    }

    pub fn meeting_data(&self) -> &Path {
        &self.meeting_data
    }

    pub fn meetings(&self) -> &Path {
        &self.meetings
    }

    pub fn staging(&self) -> &Path {
        &self.staging
    }

    pub fn deletion_trash(&self) -> &Path {
        &self.deletion_trash
    }

    pub(crate) fn storage_lock(&self) -> &Path {
        &self.storage_lock
    }

    pub(crate) fn document_relative(
        &self,
        meeting_id: &str,
        document_id: &str,
        extension: &str,
    ) -> DataResult<String> {
        validate_uuid(meeting_id)?;
        validate_uuid(document_id)?;
        validate_extension(extension)?;
        Ok(format!(
            "meeting-data/meetings/{meeting_id}/documents/{document_id}.{extension}"
        ))
    }

    pub(crate) fn staging_relative(&self, import_id: &str) -> DataResult<String> {
        validate_uuid(import_id)?;
        Ok(format!("meeting-data/staging/{import_id}.tmp"))
    }

    pub(crate) fn trash_relative(&self, deletion_id: &str) -> DataResult<String> {
        validate_uuid(deletion_id)?;
        Ok(format!("meeting-data/deletion-trash/{deletion_id}"))
    }

    pub(crate) fn meeting_relative(&self, meeting_id: &str) -> DataResult<String> {
        validate_uuid(meeting_id)?;
        Ok(format!("meeting-data/meetings/{meeting_id}"))
    }

    pub fn resolve_relative(&self, relative: impl AsRef<Path>) -> DataResult<PathBuf> {
        let relative = relative.as_ref();
        if relative.as_os_str().is_empty()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::CurDir
                        | Component::ParentDir
                        | Component::RootDir
                        | Component::Prefix(_)
                ) || matches!(component, Component::Normal(value) if value.to_string_lossy().contains(':'))
            })
        {
            return Err(DataError::UnsafePath);
        }

        let candidate = self.root.join(relative);
        let existing_ancestor = candidate
            .ancestors()
            .find(|ancestor| ancestor.exists())
            .ok_or(DataError::UnsafePath)?;
        let canonical_ancestor = existing_ancestor
            .canonicalize()
            .map_err(|_| DataError::UnsafePath)?;

        if !canonical_ancestor.starts_with(&self.root) {
            return Err(DataError::UnsafePath);
        }

        Ok(candidate)
    }
}

fn validate_uuid(value: &str) -> DataResult<()> {
    let parsed = uuid::Uuid::parse_str(value).map_err(|_| DataError::UnsafePath)?;
    if parsed.to_string() != value {
        return Err(DataError::UnsafePath);
    }
    Ok(())
}

fn validate_extension(value: &str) -> DataResult<()> {
    if matches!(
        value,
        "pdf" | "docx" | "pptx" | "xlsx" | "txt" | "md" | "csv" | "png" | "jpg" | "jpeg"
    ) {
        Ok(())
    } else {
        Err(DataError::UnsafePath)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("meetinglens-paths-{}", Uuid::new_v4()));
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn keeps_relative_paths_inside_the_private_root() {
        let directory = TestDirectory::new();
        let paths =
            StoragePaths::initialize(&directory.0).expect("storage paths should initialize");

        let resolved = paths
            .resolve_relative("meetings/synthetic-meeting/notes.txt")
            .expect("safe relative path should resolve");

        assert!(resolved.starts_with(paths.root()));
    }

    #[test]
    fn rejects_parent_and_absolute_paths() {
        let directory = TestDirectory::new();
        let paths =
            StoragePaths::initialize(&directory.0).expect("storage paths should initialize");

        assert_eq!(
            paths.resolve_relative("../outside.txt"),
            Err(DataError::UnsafePath)
        );
        assert_eq!(
            paths.resolve_relative(std::env::temp_dir().join("outside.txt")),
            Err(DataError::UnsafePath)
        );
        assert_eq!(
            paths.resolve_relative("meeting-data/meetings/file.txt:secret"),
            Err(DataError::UnsafePath)
        );
    }

    #[test]
    fn only_builds_uuid_and_allowlisted_extension_storage_paths() {
        let directory = TestDirectory::new();
        let paths =
            StoragePaths::initialize(&directory.0).expect("storage paths should initialize");
        let meeting_id = Uuid::new_v4().to_string();
        let document_id = Uuid::new_v4().to_string();

        assert!(
            paths
                .document_relative(&meeting_id, &document_id, "pdf")
                .expect("controlled path should build")
                .ends_with(&format!("/{document_id}.pdf"))
        );
        assert_eq!(
            paths.document_relative("../outside", &document_id, "pdf"),
            Err(DataError::UnsafePath)
        );
        assert_eq!(
            paths.document_relative(&meeting_id, &document_id, "exe"),
            Err(DataError::UnsafePath)
        );
    }
}
