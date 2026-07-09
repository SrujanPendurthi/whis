//! Shared helpers for XML-based extractors (pptx slides, youtube timedtext).
//!
//! quick-xml 0.41 surfaces entity references as their own `GeneralRef` events
//! rather than resolving them inside text. This resolves the five predefined
//! XML entities and numeric character references to their character.

use quick_xml::events::BytesRef;

/// Resolve an entity reference event (`&amp;`, `&#39;`, `&#x27;`) to a char.
/// Returns `None` for unknown named entities (they are simply skipped).
pub fn resolve_entity(e: &BytesRef) -> Option<char> {
    if e.is_char_ref() {
        return e.resolve_char_ref().ok().flatten();
    }
    match e.decode().ok()?.as_ref() {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => None,
    }
}
