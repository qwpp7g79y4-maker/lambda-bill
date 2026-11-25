use std::fmt;
use std::path::Path;

/// An input or usage problem. The message never includes a log line or a price
/// value. Exit code is 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Text without the `lambda-bill:` prefix.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// True for characters that can change how a terminal shows text: control
/// characters, line and paragraph separators, bidi marks, embeddings,
/// overrides and isolates, and invisible or blank-looking characters
/// (soft hyphen, zero-width, word joiner, Hangul filler, tag characters).
#[must_use]
pub fn is_unsafe_display_char(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{2028}' | '\u{2029}' | '\u{061C}' | '\u{200E}' | '\u{200F}'
                | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
                | '\u{00AD}' | '\u{034F}' | '\u{180E}' | '\u{200B}'..='\u{200D}'
                | '\u{2060}'..='\u{2065}' | '\u{206A}'..='\u{206F}' | '\u{2800}'
                | '\u{3164}' | '\u{FE0F}' | '\u{FEFF}' | '\u{E0000}'..='\u{E007F}'
                | '\u{115F}' | '\u{1160}' | '\u{17B4}' | '\u{17B5}'
                | '\u{180B}'..='\u{180D}' | '\u{FE00}'..='\u{FE0E}' | '\u{FFA0}'
                | '\u{FFF9}'..='\u{FFFB}'
        )
}

/// Text for errors with each unsafe display character written as `\u{..}`.
#[must_use]
pub fn escape_unsafe(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if is_unsafe_display_char(ch) {
            out.push_str(&format!("\\u{{{:x}}}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

/// The text to print for a usage error that clap rejected.
///
/// `escaped_render` is clap's message for the same command line after every
/// argument went through `escape_unsafe`, so each line break in it is clap's
/// own and the layout stays readable. Without it (the escaped arguments parsed
/// after all), the whole message becomes one escaped line. For an argument that
/// is not UTF-8 the caller passes clap's own message, which does not repeat
/// the argument.
#[must_use]
pub fn safe_clap_message(real_render: &str, escaped_render: Option<&str>) -> String {
    let Some(own) = escaped_render else {
        return escape_unsafe(real_render);
    };
    let mut out = String::with_capacity(own.len());
    for ch in own.chars() {
        if ch != '\n' && is_unsafe_display_char(ch) {
            out.push_str(&format!("\\u{{{:x}}}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Path text for errors and warnings. Each unsafe display character (see
/// `is_unsafe_display_char`) becomes U+FFFD so a file name cannot change the
/// terminal. A path that is not valid UTF-8 is shown as the `escape_ascii`
/// form of its raw bytes, so no byte turns into U+FFFD.
#[must_use]
pub fn show_path(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let Some(raw) = path.to_str() else {
        return path
            .as_os_str()
            .as_encoded_bytes()
            .escape_ascii()
            .to_string();
    };
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if is_unsafe_display_char(ch) {
            out.push('\u{FFFD}');
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{escape_unsafe, safe_clap_message, show_path};

    #[test]
    fn clap_message_keeps_its_own_lines_and_escapes_the_rest() {
        let own = "error: bad '\\u{1b}[31m\\u{a}FAKE'\n\nUsage: x\n";
        assert_eq!(safe_clap_message("ignored", Some(own)), own);
        // A raw control character in clap's text is escaped, a line break is kept.
        let shown = safe_clap_message("ignored", Some("a\u{1b}b\nc"));
        assert_eq!(shown, "a\\u{1b}b\nc\n");
        // Without an escaped render the whole text is one escaped line.
        let one = safe_clap_message("a\u{1b}b\nc\n", None);
        assert_eq!(one, escape_unsafe("a\u{1b}b\nc\n"));
        assert_eq!(one.lines().count(), 1);
    }

    #[test]
    fn added_invisible_characters_are_unsafe_and_replaced() {
        let mut chars = vec!['\u{115F}', '\u{1160}', '\u{17B4}', '\u{17B5}', '\u{FFA0}'];
        chars.extend('\u{180B}'..='\u{180D}');
        chars.extend('\u{FE00}'..='\u{FE0E}');
        chars.extend('\u{FFF9}'..='\u{FFFB}');
        for ch in chars {
            assert!(super::is_unsafe_display_char(ch), "U+{:04X}", ch as u32);
            let shown = show_path(Path::new(&format!("a{ch}b.log")));
            assert_eq!(shown, "a\u{FFFD}b.log", "U+{:04X}", ch as u32);
        }
    }

    #[test]
    fn all_bidi_and_separator_characters_are_replaced() {
        for ch in [
            '\u{2028}', '\u{2029}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2069}', '\u{200F}',
            '\u{200E}', '\u{202A}', '\u{061C}',
        ] {
            let shown = show_path(Path::new(&format!("a{ch}b.log")));
            assert_eq!(shown, "a\u{FFFD}b.log", "U+{:04X}", ch as u32);
        }
    }

    #[test]
    fn control_characters_are_replaced() {
        let shown = show_path(Path::new("bad\u{1b}name.log"));
        assert_eq!(shown, "bad\u{FFFD}name.log");
        assert!(!shown.chars().any(char::is_control));
    }

    #[test]
    fn line_separator_and_bidi_override_are_replaced() {
        let shown = show_path(Path::new("bad\u{2028}\u{202E}name.log"));
        assert_eq!(shown, "bad\u{FFFD}\u{FFFD}name.log");
        assert!(!shown.contains('\u{2028}'));
        assert!(!shown.contains('\u{202E}'));
        assert_eq!(show_path(Path::new("notes.log")), "notes.log");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_path_is_shown_as_byte_escape() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = Path::new(OsStr::from_bytes(b"bad\xffname.log"));
        let shown = show_path(path);
        assert_eq!(shown, "bad\\xffname.log");
        assert!(!shown.contains('\u{FFFD}'));
    }
}
