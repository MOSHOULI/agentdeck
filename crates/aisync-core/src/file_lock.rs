//! Operation-scoped ownership of an already acquired OS file lock.
use std::fs::File;

/// Owns a successfully acquired shared or exclusive lock. Do not expose or clone
/// its file: the lock must end with this operation, not with its last descriptor.
pub(crate) struct FileLock(File);
impl FileLock {
    pub(crate) fn acquired(file: File) -> Self {
        Self(file)
    }
}
impl Drop for FileLock {
    fn drop(&mut self) {
        // On Unix an unrelated concurrent fork can inherit the open-file
        // description until exec, even with CLOEXEC. Closing only this file can
        // leave a completed operation locked. Explicitly unlock on return and
        // unwind, then close. Never unlink the persistent coordination inode.
        // Abrupt process termination still depends on the OS and last handle.
        let _ = self.0.unlock();
    }
}
