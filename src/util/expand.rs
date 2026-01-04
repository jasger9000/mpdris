use std::{env, ffi::OsString, path::PathBuf};

use log::warn;
use serde::{Deserialize, Deserializer};

use crate::HOME_DIR;

pub fn serde_expand_path<'de, D: Deserializer<'de>, T: From<PathBuf>>(de: D) -> Result<T, D::Error> {
    Ok(expand_path(&String::deserialize(de)?).into())
}

pub fn expand_path(str: &str) -> PathBuf {
    if str == "~" {
        return HOME_DIR.clone();
    } else if (!str.contains('$') && !str.contains('~')) || str.chars().count() < 2 {
        return PathBuf::from(str);
    }

    // str has at least 2 chars because we checked it above
    let (first, second) = {
        let mut i = str.chars();
        (i.next().unwrap(), i.next().unwrap())
    };

    let mut ret = if first == '~' && second == '/' {
        let home = HOME_DIR.as_os_str();
        let mut s = OsString::with_capacity(home.len() + str.len() - 2);
        s.push(home);
        s
    } else {
        OsString::with_capacity(str.len())
    };

    let mut rest = if !ret.is_empty() { &str[1..] } else { str };
    while let Some(dollar_idx) = rest.find('$') {
        ret.push(&rest[..dollar_idx]);

        rest = &rest[dollar_idx + 1..];

        // if varname empty ignore it
        if rest.len() <= 1 || !is_valid_varname_char(rest.as_bytes()[0] as char) {
            ret.push("$");
            continue;
        }

        // if the dollar sign is escaped ignore it
        if is_char_escaped(&str[..dollar_idx]) {
            ret.push("$");
            continue;
        }

        let end_idx = rest
            .chars()
            .position(|b| !is_valid_varname_char(b))
            .unwrap_or_else(|| rest.len());

        let varname = &rest[..end_idx];

        match env::var_os(varname) {
            Some(var) => ret.push(var),
            None => {
                warn!("encountered undefined environment variable: {varname}");

                ret.reserve(varname.len() + 1);
                ret.push("$");
                ret.push(varname);
            }
        }

        rest = &rest[end_idx..];
    }

    ret.push(rest);
    PathBuf::from(ret)
}

fn is_valid_varname_char(chr: char) -> bool {
    chr.is_ascii_alphanumeric() || chr == '_'
}

/// Checks if a char is backslash escaped by looking at the chars before it.<br />
/// E.g. "\$" -> true; "\\$" -> false; "\\\$" -> true
///
/// the last char of s should be the char before the possibly escaped char.
fn is_char_escaped(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut escaped = false;
    for chr in s.chars().rev() {
        if chr != '\\' {
            break;
        }

        escaped = !escaped;
    }

    escaped
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_escaped_char() {
        assert!(is_char_escaped(r"pr\\efi\x\"));
        assert!(is_char_escaped(r"\"));
        assert!(is_char_escaped(r"\postfix\"));
        assert!(is_char_escaped(r"\\postfix\"));
        assert!(!is_char_escaped(r"\postfix"));
        assert!(!is_char_escaped(r"\\"));
        assert!(!is_char_escaped(r"\\\\"));
        assert!(!is_char_escaped(r"\\middle\\"));
        assert!(!is_char_escaped(r"\\middle\f\\\\"));
        assert!(!is_char_escaped(""));
    }

    #[test]
    fn test_unset_expansion() {
        unsafe {
            env::remove_var("UNSET_VAR");
        }

        assert_eq!(expand_path("some/path"), Path::new("some/path"));
        assert_eq!(expand_path("$UNSET_VAR/some/path"), Path::new("$UNSET_VAR/some/path"));
        assert_eq!(expand_path("/some/$UNSET_VAR/path"), Path::new("/some/$UNSET_VAR/path"));
        assert_eq!(expand_path("/some/path/$UNSET_VAR"), Path::new("/some/path/$UNSET_VAR"));
        assert_eq!(expand_path("/some/path$"), Path::new("/some/path$"));
        assert_eq!(expand_path("$/some/path"), Path::new("$/some/path"));
        assert_eq!(expand_path("$"), Path::new("$"));
    }

    #[test]
    fn test_expansion() {
        unsafe {
            env::set_var("HOME", Path::new("/home/repeatable"));
            env::set_var("SOME_VAR", Path::new("relative"));
            env::remove_var("UNSET_VAR");
        }

        assert_eq!(expand_path("~"), Path::new("/home/repeatable"));
        assert_eq!(expand_path("~/"), Path::new("/home/repeatable/"));
        assert_eq!(expand_path("/some/file/path/~/"), Path::new("/some/file/path/~/"));
        assert_eq!(expand_path("~/some/dir/names"), Path::new("/home/repeatable/some/dir/names"));
        assert_eq!(expand_path("~/ continue"), Path::new("/home/repeatable/ continue"));
        assert_eq!(expand_path("~$UNSET_VAR"), Path::new("~$UNSET_VAR"));
        assert_eq!(expand_path("~abcdef"), Path::new("~abcdef"));
        assert_eq!(expand_path("~~"), Path::new("~~"));
        assert_eq!(expand_path("~_"), Path::new("~_"));

        assert_eq!(expand_path("$HOME"), Path::new("/home/repeatable"));
        assert_eq!(expand_path("$HOME-"), Path::new("/home/repeatable-"));
        assert_eq!(expand_path("$HOME_/path"), Path::new("$HOME_/path"));
        assert_eq!(expand_path("$HOME/$SOME_VAR/dir"), Path::new("/home/repeatable/relative/dir"));
        assert_eq!(expand_path("$SOME_VAR/"), Path::new("relative/"));
        assert_eq!(
            expand_path("/some/path/$SOME_VAR-SOME_VAR"),
            Path::new("/some/path/relative-SOME_VAR")
        );
        assert_eq!(expand_path(r"/some/path/\$HOME"), Path::new(r"/some/path/\$HOME"));
        assert_eq!(expand_path("/some/path/$HOME_HOME"), Path::new("/some/path/$HOME_HOME"));
    }
}
