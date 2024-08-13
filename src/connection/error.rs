#[cfg(test)]
mod tests;

use std::error::Error as stdError;
use std::fmt::{self, Display, Formatter};
use std::str::Utf8Error;
use std::{io, usize};

pub type MPDResult<T> = Result<T, Error>;

/// Error representing an ACK response from MPD
///
/// You can use [Self::new] and [Self::new_string()] to get errors with custom messages or
/// directly convert [I/O](io::Error) and [Utf8Error]s using the into() method to construct a new
/// instance. You can also parse a string ACK error response into an Error using the
/// [Self::try_from_mpd()] method.
#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    stored: Box<dyn stdError + Send + Sync>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// It seems unused as of now, likely if operation on something that has to be a list was
    /// not performed on a list
    ///
    /// In MPD: [ACK_ERROR_NOT_LIST](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L11)
    NotAList,
    /// Another argument type or argument number was expected
    ///
    /// In MPD: [ACK_ERROR_ARG](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L12)
    WrongArgument,
    /// Incorrect password provided.
    /// Also, it occurs if no password is set
    ///
    /// In MPD:
    /// [ACK_ERROR_PASSWORD](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L13)
    IncorrectPassword,
    /// No permission to execute that command
    ///
    /// In MPD: [ACK_ERROR_PERMISSION](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L14)
    PermissionDenied,
    /// An unknown command got executed
    ///
    /// In MPD: [ACK_ERROR_UNKNOWN](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L15)
    UnknownCommand,
    /// Item does not exist, for example, trying to load a playlist that does not exist
    /// When trying to play a song id out of the playlist length will return [WrongArgument](ErrorKind::WrongArgument)
    ///
    /// In MPD: [ACK_ERROR_NO_EXIST](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L17)
    DoesNotExist,
    /// Gets returned if a playlist is too large. Presumably when trying to add to a full playlist
    ///
    /// In MPD: [ACK_ERROR_PLAYLIST_MAX](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L18)
    PlaylistTooLarge,
    /// A system error like an io error or when no mixer available
    ///
    /// In MPD: [ACK_ERROR_SYSTEM](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L19)
    System,
    /// Cannot load a playlist. Seemingly unused for now.
    ///
    /// In MPD: [ACK_ERROR_PLAYLIST_LOAD](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L20)
    PlaylistLoad,
    /// Cannot update the database.
    /// Currently only when Update queue is full
    ///
    /// In MPD:
    /// [ACK_ERROR_UPDATE_ALREADY](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L21)
    CannotUpdate,
    /// There's no current song
    ///
    /// In MPD: [ACK_ERROR_PLAYER_SYNC](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L22)
    PlayerSync,
    /// The command cannot be executed because the result already exists, for example, creating a new
    /// partition with a name that already exists or trying to subscribe to a channel that was
    /// already subscribed to
    ///
    /// In MPD: [ACK_ERROR_ALREADY_EXIST](https://github.com/MusicPlayerDaemon/MPD/blob/master/src/protocol/Ack.hxx#L23)
    AlreadyExists,

    /// An [I/O Error](io::Error)
    IO,
    /// A [UTF-8 Parsing error](Utf8Error)
    UTF8,
    /// Gets returned when MPD does not respond with OK MPD {{VERSION}} while initializing the
    /// connection
    InvalidConnection,
    /// Error that occurs when a line from MPD cannot be split into key value pairs
    KeyValueError,
    /// Some other custom error
    Other,
}

impl ErrorKind {
    /// Tries to convert an error code returned from an MPD ACK response into its corresponding ErrorKind
    fn from_code(value: u8) -> Option<Self> {
        use ErrorKind::*;

        match value {
            1 => Some(NotAList),
            2 => Some(WrongArgument),
            3 => Some(IncorrectPassword),
            4 => Some(PermissionDenied),
            5 => Some(UnknownCommand),
            50 => Some(DoesNotExist),
            51 => Some(PlaylistTooLarge),
            52 => Some(System),
            53 => Some(PlaylistLoad),
            54 => Some(CannotUpdate),
            55 => Some(PlayerSync),
            56 => Some(AlreadyExists),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.stored.fmt(f)
    }
}

impl stdError for Error {
    fn source(&self) -> Option<&(dyn stdError + 'static)> {
        self.stored.source()
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self {
            kind: ErrorKind::IO,
            stored: Box::new(SourceError { source: Box::new(value) }),
        }
    }
}

impl From<Utf8Error> for Error {
    fn from(value: Utf8Error) -> Self {
        Self {
            kind: ErrorKind::UTF8,
            stored: Box::new(SourceError { source: Box::new(value) }),
        }
    }
}

impl From<ParseMPDError> for Error {
    fn from(value: ParseMPDError) -> Self {
        Self {
            kind: ErrorKind::Other,
            stored: Box::new(SourceError { source: Box::new(value) }),
        }
    }
}

impl Error {
    /// Creates a new error with a static error string
    /// If you want to create an I/O or UTF-8 error type, please use the into() method
    /// See also [Self::new_string] to create a new Self with a heap allocated String
    pub fn new(kind: ErrorKind, message: &'static str) -> Self {
        Self {
            kind,
            stored: Box::new(StrMessageError { message }),
        }
    }

    /// Creates a new error with a heap allocated error string
    /// If you want to create an IO or UTF-8 error type, please use the into() method
    /// See also [Self::new] to create a new Self with a static error string
    pub fn new_string(kind: ErrorKind, message: String) -> Self {
        Self {
            kind,
            stored: Box::new(StringMessageError { message }),
        }
    }

    /// Tries to parse an MPD error output into an error.
    /// Returns the parsed error or when it was unable to parse [ParseMPDError]
    pub fn try_from_mpd(output: String) -> Result<Self, ParseMPDError> {
        use ParseMPDErrorKind::*;
        use ParseState::*;

        if output.is_empty() {
            return Err(ParseMPDError::new(EmptyString, 0));
        }

        let mut error_kind: Option<ErrorKind> = None;
        let mut list_number: Option<u8> = None;
        let mut failed_command: Option<String> = None;

        let mut state = FindACK;

        let mut begin = 0;
        // ACK [error@command_listNum] {current_command} message_text

        for (i, chr) in output.chars().enumerate() {
            match state {
                FindACK => {
                    let ack = "ACK";
                    if let Some(ack_chr) = ack.chars().nth(i) {
                        if chr != ack_chr {
                            return Err(ParseMPDError::expected(ack_chr, i));
                        }
                    } else {
                        state = FindLeftBracket;
                    }
                }
                FindLeftBracket => {
                    if chr == '[' {
                        begin = i + 1;
                        state = GetErrorType;
                    } else if chr != ' ' {
                        return Err(ParseMPDError::expected('[', i));
                    }
                }
                GetErrorType => {
                    if chr == '@' {
                        match output[begin..i].parse() {
                            Ok(err_code) => error_kind = ErrorKind::from_code(err_code),
                            Err(err) => {
                                use std::num::IntErrorKind::*;

                                match err.kind() {
                                    Empty | InvalidDigit => ParseMPDError::number(i),
                                    PosOverflow | NegOverflow => ParseMPDError::new(InvalidCode, begin),
                                    &_ => ParseMPDError::new(UnexpectedSymbol, i),
                                };
                            }
                        }

                        if error_kind.is_none() {
                            return Err(ParseMPDError::new(InvalidCode, begin));
                        }

                        begin = i + 1;
                        state = GetListNum;
                    } else if !chr.is_ascii_digit() {
                        return Err(ParseMPDError::number(i));
                    }
                }
                GetListNum => {
                    if chr == ']' {
                        list_number = match output[begin..i].parse() {
                            Ok(i) => Some(i),
                            Err(_) => return Err(ParseMPDError::number(i)),
                        };

                        state = FindLeftBrace;
                    } else if !chr.is_ascii_digit() {
                        return Err(ParseMPDError::number(i));
                    }
                }
                FindLeftBrace => {
                    if chr == '{' {
                        begin = i + 1;
                        state = GetFailedCommand;
                    } else if chr != ' ' {
                        return Err(ParseMPDError::expected('{', i));
                    }
                }
                GetFailedCommand => {
                    if chr == '}' {
                        failed_command = Some(output[begin..i].to_string());

                        begin = i + 1;
                        break;
                    }
                }
            };
        }

        let message_text = output[begin..].trim().to_string();

        if error_kind.is_none() || list_number.is_none() || failed_command.is_none() || message_text.is_empty() {
            return Err(ParseMPDError::new(UnexpectedSymbol, output.chars().count() - 1));
        }

        Ok(Self {
            kind: error_kind.unwrap(),
            stored: Box::new(ParsedError {
                current_command: failed_command.unwrap(),
                list_num: list_number.unwrap(),
                message_text,
            }),
        })
    }
}

/// An error which can be returned when parsing a String into an [Error]
///
/// This `struct` is created when using the [Error::try_from_mpd()] method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseMPDError {
    pub kind: ParseMPDErrorKind,
    /// The character position (zero indexed) at which the parsing failed
    pub pos: usize,
    /// The character the parser expected to find
    pub expected_char: Option<char>,
}

/// Kinds of errors the parser can encounter when parsing. Used in [ParseMPDError]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ParseMPDErrorKind {
    EmptyString,
    UnexpectedSymbol,
    ExpectedNumber,
    InvalidCode,
}

impl Display for ParseMPDErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use ParseMPDErrorKind::*;

        f.write_str(match self {
            EmptyString => "cannot parse error from empty string",
            UnexpectedSymbol => "encountered an unexpected symbol",
            ExpectedNumber => "expected a number",
            InvalidCode => "got invalid error code",
        })
    }
}

impl stdError for ParseMPDError {}

impl Display for ParseMPDError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)?;
        if let Some(chr) = self.expected_char {
            write!(f, ", expected char '{chr}'")?;
        }
        write!(f, " at position {}", self.pos)
        //        f.write_str(&self.output)?;
        //        write!(f, "\n{}/\\", " ".repeat(self.pos))
    }
}

impl ParseMPDError {
    fn new(kind: ParseMPDErrorKind, pos: usize) -> Self {
        Self {
            kind,
            pos,
            expected_char: None,
        }
    }

    fn expected(char: char, pos: usize) -> Self {
        Self {
            kind: ParseMPDErrorKind::UnexpectedSymbol,
            pos,
            expected_char: Some(char),
        }
    }

    fn number(pos: usize) -> Self {
        Self {
            kind: ParseMPDErrorKind::ExpectedNumber,
            pos,
            expected_char: None,
        }
    }
}

/// States of the MPD Error parser
enum ParseState {
    FindACK,
    FindLeftBracket,
    GetErrorType,
    GetListNum,
    FindLeftBrace,
    GetFailedCommand,
}

/// Internal type for holding a custom message when a new [Error] is constructed using
/// [Error::new()].
#[derive(Debug)]
struct StrMessageError {
    message: &'static str,
}

/// Internal type for holding a custom message when a new [Error] is constructed using
/// [Error::new_string()].
#[derive(Debug)]
struct StringMessageError {
    message: String,
}

/// Internal type for holding a source Error when a new [Error] is constructed using
/// [Error::source()]. Mostly used to wrap [I/O](io::Error) and [Utf8Error]s
#[derive(Debug)]
struct SourceError {
    source: Box<dyn stdError + Send + Sync>,
}

/// Internal type for holding a successfully parsed result from [Error::try_from_mpd()]
#[derive(Debug)]
struct ParsedError {
    current_command: String,
    list_num: u8,
    message_text: String,
}

impl Display for StrMessageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl Display for StringMessageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Display for SourceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.source.fmt(f)
    }
}

impl Display for ParsedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: at command {} (#{})",
            self.message_text, self.current_command, self.list_num
        )
    }
}

impl stdError for StrMessageError {}
impl stdError for StringMessageError {}
impl stdError for ParsedError {}
impl stdError for SourceError {
    fn source(&self) -> Option<&(dyn stdError + 'static)> {
        Some(self.source.as_ref())
    }
}
