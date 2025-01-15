//! Indexing for vim buffer.

// Re-export
pub use crate::buf::widx::cidx::ColIndex;
pub use crate::buf::widx::lidx::LineIndex;

pub mod cidx;
pub mod lidx;
