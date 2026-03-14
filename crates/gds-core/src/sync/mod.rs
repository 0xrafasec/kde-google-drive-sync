//! Sync engine: diff, queue, and executor.

mod change;
mod diff;
mod executor;
mod fs;
mod path;
mod queue;
mod workspace_stub;

pub use change::{is_conflict, SyncAction, SyncActionKind};
pub use diff::{parse_drive_modified, DiffEngine};
pub use executor::SyncExecutor;
pub use fs::{DirEntry, LocalFileMeta, LocalFs, TokioLocalFs};
pub use path::{conflict_copy_path, safe_local_path, CONFLICT_SUFFIX_FORMAT};
pub use queue::SyncQueue;
pub use workspace_stub::{
    gdoc_stub_content, gsheet_stub_content, gslides_stub_content, stub_content_for_mime,
    workspace_file_url,
};
