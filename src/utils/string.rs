//! String utilities
//!
//! Contains helper functions for safe string manipulation.

/// Safely truncate a string at a character boundary
///
/// This function truncates a string to at most `max_chars` characters,
/// ensuring the truncation happens at a valid UTF-8 character boundary.
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_chars` - Maximum number of characters to keep
///
/// # Returns
/// A string slice containing at most `max_chars` characters
///
/// # Example
/// ```
/// use anthropic_bedrock_proxy::utils::truncate_str;
///
/// let text = "Hello, 世界!";
/// assert_eq!(truncate_str(text, 8), "Hello, 世");
/// assert_eq!(truncate_str(text, 100), "Hello, 世界!");
/// ```
pub fn truncate_str(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Safely truncate a string and append a suffix if truncated
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_chars` - Maximum number of characters to keep (not including suffix)
/// * `suffix` - Suffix to append if string was truncated
///
/// # Returns
/// A new String, either the original (if short enough) or truncated with suffix
///
/// # Example
/// ```
/// use anthropic_bedrock_proxy::utils::truncate_with_suffix;
///
/// let text = "Hello, World!";
/// assert_eq!(truncate_with_suffix(text, 5, "..."), "Hello...");
/// assert_eq!(truncate_with_suffix("Hi", 5, "..."), "Hi");
/// ```
pub fn truncate_with_suffix(s: &str, max_chars: usize, suffix: &str) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}{}", truncate_str(s, max_chars), suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_str_ascii() {
        let text = "Hello, World!";
        assert_eq!(truncate_str(text, 5), "Hello");
        assert_eq!(truncate_str(text, 100), "Hello, World!");
    }

    #[test]
    fn test_truncate_str_unicode() {
        let text = "Hello, 世界!";
        assert_eq!(truncate_str(text, 7), "Hello, ");
        assert_eq!(truncate_str(text, 8), "Hello, 世");
        assert_eq!(truncate_str(text, 9), "Hello, 世界");
    }

    #[test]
    fn test_truncate_str_emoji() {
        let text = "Hello 👋🏻 World";
        // 👋🏻 is actually 2 characters (emoji + skin tone modifier)
        assert_eq!(truncate_str(text, 6), "Hello ");
        assert_eq!(truncate_str(text, 7), "Hello 👋");
    }

    #[test]
    fn test_truncate_str_special_chars() {
        // The arrow character used in the error message
        let text = "     1→{";
        assert_eq!(truncate_str(text, 6), "     1");
        assert_eq!(truncate_str(text, 7), "     1→");
    }

    #[test]
    fn test_truncate_with_suffix() {
        let text = "Hello, World!";
        assert_eq!(truncate_with_suffix(text, 5, "..."), "Hello...");
        assert_eq!(truncate_with_suffix("Hi", 5, "..."), "Hi");
    }

    #[test]
    fn test_truncate_empty() {
        assert_eq!(truncate_str("", 10), "");
        assert_eq!(truncate_with_suffix("", 10, "..."), "");
    }
}
