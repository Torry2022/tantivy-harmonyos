//! MmapDirectory's lock implementation opens lock files for writing and may create
//! them. Readers need only the existing metadata lock, opened with read access.
use std::{fs::File, io, path::Path, sync::Arc};
use tantivy::directory::{
    error::{DeleteError, LockError, OpenReadError, OpenWriteError},
    Directory, DirectoryLock, FileHandle, Lock, MmapDirectory, WatchCallback, WatchHandle,
    WritePtr, META_LOCK,
};

#[derive(Clone, Debug)]
pub struct ReadOnlyDirectory {
    inner: MmapDirectory,
    path: std::path::PathBuf,
}

fn denied() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "read-only index")
}

impl ReadOnlyDirectory {
    pub fn open(path: &str) -> super::Result<Self> {
        Ok(Self {
            inner: MmapDirectory::open(path).map_err(|e| e.to_string())?,
            path: path.into(),
        })
    }
}

impl Directory for ReadOnlyDirectory {
    fn get_file_handle(&self, path: &Path) -> Result<Arc<dyn FileHandle>, OpenReadError> {
        self.inner.get_file_handle(path)
    }
    fn exists(&self, path: &Path) -> Result<bool, OpenReadError> {
        self.inner.exists(path)
    }
    fn atomic_read(&self, path: &Path) -> Result<Vec<u8>, OpenReadError> {
        self.inner.atomic_read(path)
    }
    fn delete(&self, path: &Path) -> Result<(), DeleteError> {
        Err(DeleteError::IoError {
            io_error: Arc::new(denied()),
            filepath: path.into(),
        })
    }
    fn open_write(&self, path: &Path) -> Result<WritePtr, OpenWriteError> {
        Err(OpenWriteError::IoError {
            io_error: Arc::new(denied()),
            filepath: path.into(),
        })
    }
    fn atomic_write(&self, _: &Path, _: &[u8]) -> io::Result<()> {
        Err(denied())
    }
    fn sync_directory(&self) -> io::Result<()> {
        Err(denied())
    }
    fn acquire_lock(&self, lock: &Lock) -> Result<DirectoryLock, LockError> {
        if lock.filepath != META_LOCK.filepath {
            return Err(LockError::wrap_io_error(denied()));
        }
        let file = File::open(self.path.join(&lock.filepath)).map_err(LockError::wrap_io_error)?;
        file.lock().map_err(LockError::wrap_io_error)?;
        Ok(DirectoryLock::from(Box::new(file)))
    }
    fn watch(&self, _: WatchCallback) -> tantivy::Result<WatchHandle> {
        Err(denied().into())
    }
}
