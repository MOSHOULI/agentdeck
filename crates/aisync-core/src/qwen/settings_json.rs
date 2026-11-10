//! Qwen read-only settings syntax only. Never use for manifests or owned writes.
use super::{InspectionIssue, LIMITS};
use crate::inspection::parse;
use serde_json::Value;

pub(super) fn parse_settings(bytes: &[u8]) -> Result<Value, InspectionIssue> {
    // Charge original bytes, including BOM and comments, before allocating.
    if bytes.len() > LIMITS.max_manifest_bytes {
        return Err(InspectionIssue::FileTooLarge);
    }
    std::str::from_utf8(bytes).map_err(|_| InspectionIssue::UnsupportedJson)?;
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let mut clean = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                loop {
                    match bytes.get(i) {
                        Some(b'"') => {
                            i += 1;
                            break;
                        }
                        Some(b'\\') => {
                            i += 2;
                        }
                        Some(_) => {
                            i += 1;
                        }
                        None => return Err(InspectionIssue::UnsupportedJson),
                    }
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                // Match strip-json-comments line termination (LF or CRLF);
                // a lone CR does not end the comment.
                while i < bytes.len() && bytes[i] != b'\n' {
                    if bytes[i] != b'\r' {
                        clean[i] = b' ';
                    }
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                clean[i..i + 2].fill(b' ');
                i += 2;
                loop {
                    if i >= bytes.len() {
                        return Err(InspectionIssue::UnsupportedJson);
                    }
                    if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        clean[i..i + 2].fill(b' ');
                        i += 2;
                        break;
                    }
                    if !matches!(bytes[i], b'\r' | b'\n') {
                        clean[i] = b' ';
                    }
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    // Whitespace replacement cannot join tokens or remove trailing commas.
    // Retain all shared depth/entry/string/duplicate-key protections.
    parse::parse_json(&clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_bom_and_string_delimiters_are_handled_without_changing_values() {
        for input in [
            "\u{feff}//head\r\n{\"$version\":4,/*块*/\"general\":{}}//eof",
            "{\"$version\"/**/:/**/4,\"general\":{}}",
            "/* \" // */{\"$version\":4,\"general\":{}}\r\n",
        ] {
            assert_eq!(
                parse_settings(input.as_bytes()).unwrap(),
                serde_json::json!({"$version":4,"general":{}})
            );
        }
        let value = serde_json::json!({"url":"https://host/a/*b*/", "quote":"a\\\"//b", "slash":"\\", "unicode":"中文", "bom":"\u{feff}"});
        let input = format!("/*outside*/{}//tail", value);
        assert_eq!(parse_settings(input.as_bytes()).unwrap(), value);
    }

    #[test]
    fn rejects_incomplete_syntax_utf8_and_non_json_extensions() {
        for input in [
            b"/*".as_slice(),
            b"//comment\r{\"$version\":4}",
            b"/**",
            b"{}/*",
            b"\"unterminated",
            b"\"escape\\",
            b"{\"a\":1,/*tail*/}",
            b"[1,//tail\n]",
            b"1/**/2",
            b"tr/**/ue",
            b"{unquoted:1}",
            b"{'a':1}",
            b"{} / stray",
            b"/*outer /*nested*/ */{}",
            b"\xef\xbb\xbf\xef\xbb\xbf{}",
            b" \xef\xbb\xbf{}",
            b"//\xff\n{}",
            b"/*\xff*/{}",
            b"{\"x\":\"line\n//still string\"}",
        ] {
            assert_eq!(
                parse_settings(input),
                Err(InspectionIssue::UnsupportedJson),
                "{input:?}"
            );
        }
    }

    #[test]
    fn shared_budgets_and_duplicates_remain_enforced_after_comments() {
        assert_eq!(
            parse_settings(br#"{"a":1,/*private*/"\u0061":2}"#),
            Err(InspectionIssue::DuplicateJsonKey)
        );
        let large = format!("/*{}*/{{}}", "x".repeat(LIMITS.max_manifest_bytes));
        assert_eq!(
            parse_settings(large.as_bytes()),
            Err(InspectionIssue::FileTooLarge)
        );
        let depth = format!(
            "/*x*/{}0{}",
            "[".repeat(LIMITS.max_json_depth + 1),
            "]".repeat(LIMITS.max_json_depth + 1)
        );
        assert_eq!(
            parse_settings(depth.as_bytes()),
            Err(InspectionIssue::JsonLimit)
        );
        let string = format!("/*x*/\"{}\"", "x".repeat(LIMITS.max_json_string_bytes + 1));
        assert_eq!(
            parse_settings(string.as_bytes()),
            Err(InspectionIssue::JsonLimit)
        );
        let entries = format!(
            "/*x*/[{}]",
            vec!["0"; LIMITS.max_json_entries + 1].join(",")
        );
        assert_eq!(
            parse_settings(entries.as_bytes()),
            Err(InspectionIssue::JsonLimit)
        );
    }
}
