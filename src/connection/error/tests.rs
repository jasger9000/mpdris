use super::*;

type ParseResult = std::result::Result<Error, ParseMPDError>;

#[test]
fn test_valid_mpd_error() {
    let output = String::from("ACK [5@0] {play} No such song");
    let expected: ParseResult = Ok(Error {
        kind: ErrorKind::from_code(5).unwrap(),
        stored: ParsedError {
            list_num: 0,
            current_command: String::from("play"),
            message_text: String::from("No such song"),
        }
        .into(),
    });
    
    let result = Error::try_from_mpd(output);
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
    let expected: ParseResult = Err(ParseMPDError::new(ParseMPDErrorKind::InvalidCode, 6));

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
