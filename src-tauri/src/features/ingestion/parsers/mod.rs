//! Document text extractors. Each parser turns a source buffer into plain text
//! (or, for markdown, header-delimited sections); chunking happens in the
//! parent module.

pub mod markdown;
pub mod pdf;
pub mod pptx;
