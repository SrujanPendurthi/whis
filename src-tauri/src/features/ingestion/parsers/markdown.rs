//! Markdown reader that splits a document into header-delimited sections.
//!
//! Each ATX header line (`#`..`######`) starts a new section; the header and its
//! following body up to the next header form one section. Oversized sections are
//! further bounded downstream by [`crate::features::ingestion::chunker::split_text`].

/// Split markdown into sections, each beginning at a header line. Leading
/// content before the first header (if any) becomes its own section.
pub fn split_sections(text: &str) -> Vec<String> {
    let mut sections: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();

    for line in text.lines() {
        if is_atx_header(line) && !current.is_empty() {
            push_section(&mut sections, &current);
            current.clear();
        }
        current.push(line);
    }
    push_section(&mut sections, &current);
    sections
}

/// True for an ATX header: 1–6 leading `#` followed by a space, allowing up to
/// three leading spaces of indentation (per CommonMark).
fn is_atx_header(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return false;
    }
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ')
}

fn push_section(sections: &mut Vec<String>, lines: &[&str]) {
    let joined = lines.join("\n");
    if !joined.trim().is_empty() {
        sections.push(joined.trim().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_headers() {
        let md = "# Title\nintro\n\n## A\nbody a\n\n## B\nbody b";
        let s = split_sections(md);
        assert_eq!(s.len(), 3);
        assert!(s[0].starts_with("# Title"));
        assert!(s[1].starts_with("## A"));
        assert!(s[2].starts_with("## B"));
    }

    #[test]
    fn keeps_preamble_before_first_header() {
        let md = "loose intro text\n\n# Heading\nbody";
        let s = split_sections(md);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0], "loose intro text");
    }

    #[test]
    fn recognizes_header_levels_and_rejects_non_headers() {
        assert!(is_atx_header("# h1"));
        assert!(is_atx_header("###### h6"));
        assert!(is_atx_header("   ## indented"));
        assert!(!is_atx_header("#nospace"));
        assert!(!is_atx_header("####### seven"));
        assert!(!is_atx_header("    # four-space-indent-is-code"));
        assert!(!is_atx_header("text # midline"));
    }

    #[test]
    fn empty_input_yields_no_sections() {
        assert!(split_sections("").is_empty());
        assert!(split_sections("\n\n   \n").is_empty());
    }
}
