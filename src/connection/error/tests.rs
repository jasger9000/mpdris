use super::*;

type ParseResult = Result<Error, ParseMPDError>;

#[test]
fn test_valid_mpd_error() {
    let output = String::from("ACK [5@0] {play} No such song");
    let expected: ParseResult = Ok(Error {
        kind: ErrorKind::from_code(5).unwrap(),
        stored: Box::new(ParsedError {
            list_num: 0,
            current_command: String::from("play"),
            message_text: String::from("No such song"),
        }),
    });

    let result = Error::try_from_mpd(output);
    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}

#[test]
fn test_large_error_code() {
    let output = String::from("ACK [50@0] {load} No such playlist");
    let result = Error::try_from_mpd(output);
    let expected: ParseResult = Ok(Error {
        kind: ErrorKind::from_code(50).unwrap(),
        stored: Box::new(ParsedError {
            list_num: 0,
            current_command: String::from("load"),
            message_text: String::from("No such playlist"),
        }),
    });

    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}

#[test]
fn test_overflowing_error_code() {
    let output = String::from("ACK [340282366920938463463374607431768211456@0] {load} No such playlist");
    let result = Error::try_from_mpd(output.clone());
    let expected: ParseResult = Err(ParseMPDError::new(ParseMPDErrorKind::InvalidCode, 5));

    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}

#[test]
fn test_invalid_format_no_ack() {
    let output = String::from("5@0 {play} No such song");
    let result = Error::try_from_mpd(output);
    let expected: ParseResult = Err(ParseMPDError::expected('A', 0));

    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}

#[test]
fn test_invalid_format_no_brackets() {
    let output = String::from("ACK 5@0 play No such song");
    let result = Error::try_from_mpd(output);
    let expected: ParseResult = Err(ParseMPDError::expected('[', 4));

    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}

#[test]
fn test_missing_fields() {
    let output = String::from("ACK [5@] {play} No such song");
    let result = Error::try_from_mpd(output);
    let expected: ParseResult = Err(ParseMPDError::number(7));

    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}

#[test]
fn test_invalid_error_code() {
    let output = String::from("ACK [10@0] {play} No such song");
    let result = Error::try_from_mpd(output);
    let expected: ParseResult = Err(ParseMPDError::new(ParseMPDErrorKind::InvalidCode, 5));

    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}

#[test]
fn test_invalid_command_list_num() {
    let output = String::from("ACK [5@y] {play} No such song");
    let result = Error::try_from_mpd(output);
    let expected: ParseResult = Err(ParseMPDError::number(7));

    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}

#[test]
fn test_missing_message_text() {
    let output = String::from("ACK [5@0] {play}");
    let result = Error::try_from_mpd(output);
    let expected: ParseResult = Err(ParseMPDError::new(ParseMPDErrorKind::UnexpectedSymbol, 15));

    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}

#[test]
fn test_missing_command() {
    let output = String::from("ACK [5@0] {} No such song");
    let result = Error::try_from_mpd(output);
    let expected: ParseResult = Ok(Error {
        kind: ErrorKind::UnknownCommand,
        stored: ParsedError {
            current_command: String::from(""),
            list_num: 0,
            message_text: String::from("No such song"),
        }
        .into(),
    });

    assert_eq!(format!("{result:?}"), format!("{expected:?}"));
}
