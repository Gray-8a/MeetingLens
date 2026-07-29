use std::path::{Component, Path, PathBuf};

use super::error::{DataError, DataResult};

const DATABASE_FILE_NAME: &str = "meetinglens.sqlite3";

#[derive(Clone, Debug)]
pub struct StoragePaths {
    root: PathBuf,
    database: PathBuf,
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

        Ok(Self { root, database })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database(&self) -> &Path {
        &self.database
    }

    pub fn resolve_relative(&self, relative: impl AsRef<Path>) -> DataResult<PathBuf> {
        let relative = relative.as_ref();
        if relative.as_os_str().is_empty()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
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
    }
}
