mod collections;
mod document;

pub use crate::collections::dir_based_collection::DirBasedCollection;
pub use crate::collections::file_based_collection::FileBasedCollection;
pub use crate::collections::in_memory_collection::InMemoryCollection;
pub use crate::document::Document;
