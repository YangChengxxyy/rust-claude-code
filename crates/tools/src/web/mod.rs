pub mod fetch;
pub mod search;

pub use fetch::{truncate_text, WebCache, WebPage};
pub use search::{SearchBackend, SearchResult};
