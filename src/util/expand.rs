use std::env;

use serde::{Deserialize, Deserializer};

use crate::HOME_DIR;

pub fn serde_expand_path<'de, D: Deserializer<'de>>(de: D) -> Result<Box<str>, D::Error> {
    Ok(expand_path(&String::deserialize(de)?).into_boxed_str())
}

pub fn expand_path(str: &str) -> String {
    if str == "~" {
        return HOME_DIR.clone();
    } else if (!str.contains('$') && !str.contains('~')) || str.chars().count() <= 1 {
        return str.to_string();
    }

    let mut ret = String::with_capacity(str.len());

    // str has at least 2 chars because we checked it above
    let (first, second) = {
        let mut i = str.chars();
        (i.next().unwrap(), i.next().unwrap())
    };

    if first == '~' && second == '/' {
        ret.reserve(HOME_DIR.len());

        // this isn't actually unsafe because the content of HOME_DIR is always valid UTF-8
        unsafe {
            ret.as_mut_vec().append(&mut HOME_DIR.clone().into_bytes());
        }
    }

    let mut remaining = if !ret.is_empty() { &str[1..] } else { str };
    while let Some(dollar_idx) = remaining.find('$') {
        ret.push_str(&remaining[..dollar_idx]);

        remaining = &remaining[dollar_idx + 1..];

        // if varname empty ignore it
        if remaining.len() <= 1 || !is_valid_varname_char(remaining.as_bytes()[0] as char) {
            ret.push('$');
            continue;
        }

        // if the dollar sign is escaped ignore it
        if is_char_escaped(str[..dollar_idx].as_bytes()) {
            ret.push('$');
            continue;
        }

        // go from dollar-idx until non-varname char
        let mut end_idx = remaining.len() - 1;
        for (i, chr) in remaining.chars().enumerate() {
            if !is_valid_varname_char(chr) {
                end_idx = i - 1;
                break;
            }
        }

        let varname = &remaining[..=end_idx];
        match env::var(varname) {
            Ok(var) => {
                ret.reserve(var.len());
                // this isn't actually unsafe because the content of var is always valid UTF-8
                unsafe {
                    ret.as_mut_vec().append(&mut var.into_bytes());
                }
            }
            Err(_e) => {
                eprintln!("encountered undefined environment variable: {varname}");
                ret.reserve(varname.len() + 1);
                ret.push('$');
                ret.push_str(varname);
            }
        }

        remaining = &remaining[end_idx + 1..];
    }

    ret.push_str(remaining);

    ret
}

fn is_valid_varname_char(chr: char) -> bool {
    chr.is_ascii_alphanumeric() || chr == '_'
}

/// Checks if a char is backslash escaped by looking at the chars before it.<br />
/// E.g. "\$" -> true; "\\$" -> false; "\\\$" -> true
///
/// the first byte of bytes should be the index after the char to check
fn is_char_escaped(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    let mut escaped = false;
    let mut n = bytes.len();
    while n > 0 {
        if bytes[n - 1] != b'\\' {
            break;
        }

        escaped = !escaped;
        n -= 1;
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escaped_char() {
        assert!(is_char_escaped(r"pr\\efi\x\".as_bytes()));
        assert!(is_char_escaped(r"\".as_bytes()));
        assert!(is_char_escaped(r"\postfix\".as_bytes()));
        assert!(is_char_escaped(r"\\postfix\".as_bytes()));
        assert!(!is_char_escaped(r"\postfix".as_bytes()));
        assert!(!is_char_escaped(r"\\".as_bytes()));
        assert!(!is_char_escaped(r"\\\\".as_bytes()));
        assert!(!is_char_escaped(r"\\middle\\".as_bytes()));
        assert!(!is_char_escaped(r"\\middle\f\\\\".as_bytes()));
        assert!(!is_char_escaped(&[]));
    }

    #[test]
    fn test_unset_expansion() {
        env::remove_var("UNSET_VAR");

        assert_eq!(expand_path("some/path"), "some/path");
        assert_eq!(expand_path("$UNSET_VAR/some/path"), "$UNSET_VAR/some/path");
        assert_eq!(expand_path("/some/$UNSET_VAR/path"), "/some/$UNSET_VAR/path");
        assert_eq!(expand_path("/some/path/$UNSET_VAR"), "/some/path/$UNSET_VAR");
        assert_eq!(expand_path("/some/path$"), "/some/path$");
        assert_eq!(expand_path("$/some/path"), "$/some/path");
        assert_eq!(expand_path("$"), "$");
    }

    #[test]
    fn test_expansion() {
        env::set_var("HOME", "/home/repeatable");
        env::set_var("SOME_VAR", "relative");
        env::remove_var("UNSET_VAR");

        assert_eq!(expand_path("~"), "/home/repeatable");
        assert_eq!(expand_path("~/"), "/home/repeatable/");
        assert_eq!(expand_path("/some/file/path/~/"), "/some/file/path/~/");
        assert_eq!(expand_path("~/some/dir/names"), "/home/repeatable/some/dir/names");
        assert_eq!(expand_path("~/ continue"), "/home/repeatable/ continue");
        assert_eq!(expand_path("~$UNSET_VAR"), "~$UNSET_VAR");
        assert_eq!(expand_path("~abcdef"), "~abcdef");
        assert_eq!(expand_path("~~"), "~~");
        assert_eq!(expand_path("~_"), "~_");

        assert_eq!(expand_path("$HOME"), "/home/repeatable");
        assert_eq!(expand_path("$HOME-"), "/home/repeatable-");
        assert_eq!(expand_path("$HOME_/path"), "$HOME_/path");
        assert_eq!(expand_path("$HOME/$SOME_VAR/dir"), "/home/repeatable/relative/dir");
        assert_eq!(expand_path("$SOME_VAR/"), "relative/");
        assert_eq!(expand_path("/some/path/$SOME_VAR-SOME_VAR"), "/some/path/relative-SOME_VAR");
        assert_eq!(expand_path(r"/some/path/\$HOME"), r"/some/path/\$HOME");
        assert_eq!(expand_path("/some/path/$HOME_HOME"), "/some/path/$HOME_HOME");
    }
}
