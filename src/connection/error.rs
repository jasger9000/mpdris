use std::error::Error as stdError;
use std::fmt::{self, Display, Formatter};
use std::str::Utf8Error;
use std::{io, usize};

pub type MPDResult<T> = Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    stored: Box<dyn stdError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Seems to be unused as of now, likely if operation on something that has to be a list was
    /// not performed on list
    /// In MPD: ACK_ERROR_NOT_LIST
    NotAList,
    /// Another argument type or argument number was expected
    /// In MPD: ACK_ERROR_ARG
    WrongArgument,
    /// Incorrect password provided. Also occures if no password is set
    /// In MPD: ACK_ERROR_PASSWORD
    IncorrectPassword,
    /// No permission to execute that command
    /// In MPD: ACK_ERROR_PERMISSION
    PermissionDenied,
    /// An unknown command got executed
    /// In MPD: ACK_ERROR_UNKNOWN
    UnknownCommand,
    /// Item does not exist for example trying to load a playlist that does not exist
    /// When trying to play a songid out of the playlist length will return [Self::Argument]
    /// In MPD: ACK_ERROR_NO_EXIST
    DoesNotExist,
    /// Gets returned if a playlist is too large. Presumably when trying to add to a full playlist
    /// In MPD: ACK_ERROR_PLAYLIST_MAX
    PlaylistTooLarge,
    /// A system error like an io error or when no mixer available
    /// In MPD: ACK_ERROR_SYSTEM
    System,
    /// Cannot load a playlist. Seemingly unused for now.
    /// In MPD: ACK_ERROR_PLAYLIST_LOAD
    PlaylistLoad,
    /// Cannot update the database. Currently only when Update queue full
    /// In MPD: ACK_ERROR_UPDATE_ALREADY
    CannotUpdate,
    /// There's no current song
    /// In MPD: ACK_ERROR_PLAYER_SYNC
    PlayerSync,
    /// The command cannot be executed because the result already exists for example creating a new
    /// parition with a name that already exists or trying to subscribe to a channel that was
    /// already subscribed to
    /// In MPD: ACK_ERROR_ALREADY_EXIST
    AlreadyExists,

    /// An I/O Error [io::Error]
    IO,
    /// A [UTF-8 Parsing error ]
    UTF8,
    /// Data buffer limit reading the response was exceeded
    DataLimitExceeded,
    /// Gets returned when MPD does not respond with OK MPD {{VERSION}} while initialising the
    /// connection
    InvalidConnection,
    Other,
}

// impl Display for ErrorKind {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.write_str(self.as_str())
//     }
// }
//
// impl ErrorKind {
//     pub fn as_str(&self) -> &'static str {
//         use ErrorKind::*;
//
//         match *self {
//             NotAList => "tried to execute list operation on non list item",
//             WrongArgument => "wrong argument used",
//             IncorrectPassword => "password incorrect",
//             PermissionDenied => "permission not granted",
//             UnknownCommand => "unknown command",
//             DoesNotExist => "item does not exist",
//             PlaylistTooLarge => "playlist is too large",
//             System => "system error occured",
//             PlaylistLoad => "could not load playlist",
//             CannotUpdate => "could not update database",
//             PlayerSync => "could not sync player",
//             AlreadyExists => "The item already exists",
//
//             IO => "io error occured",
//             UTF8 => "response included invalid UTF-8",
//             DataLimitExceeded => "data limit exceeded",
//             InvalidConnection => "Could not validate MPD connection"
//             Other => ""
//         }
//     }
// }

impl ErrorKind {
    /// Tries to convert an error code returned from an MPD ACK response into its corresponding ErrorKind
    fn from_code(value: usize) -> Option<Self> {
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
            stored: Box::new(SourceError {
                source: Box::new(value),
            }),
        }
    }
}

impl From<Utf8Error> for Error {
    fn from(value: Utf8Error) -> Self {
        Self {
            kind: ErrorKind::UTF8,
            stored: Box::new(SourceError {
                source: Box::new(value),
            }),
        }
    }
}

impl From<ParseMPDError> for Error {
    fn from(value: ParseMPDError) -> Self {
        Self {
            kind: ErrorKind::Other,
            stored: Box::new(SourceError {
                source: Box::new(value),
            }),
        }
    }
}

impl Error {
    /// Creates a new error with a static error string
    /// If you want to create a IO or UTF-8 error type, please use the into() method
    /// See also [Self::new_string] to create a new Self with a heap allocated String
    pub fn new(kind: ErrorKind, message: &'static str) -> Self {
        Self {
            kind,
            stored: Box::new(StrMessageError { message }),
        }
    }

    /// Creates a new error with a heap allocated error string
    /// If you want to create a IO or UTF-8 error type, please use the into() method
    /// See also [Self::new] to create a new Self with a static error string
    pub fn new_string(kind: ErrorKind, message: String) -> Self {
        Self {
            kind,
            stored: Box::new(StringMessageError { message }),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Tries to parse an MPD error output into an error
    pub fn try_from_mpd(output: String) -> Result<Self, ParseMPDError> {
        use ParseMPDErrorKind::*;
        use ParseState::*;

        if output.is_empty() {
            return Err(ParseMPDError::new(EmptyString, 0));
        }

        let mut error_kind: Option<ErrorKind> = None;
        let mut list_number: Option<usize> = None;
        let mut failed_command: Option<String> = None;

        let mut state = FindACK;

        let mut temp = String::new();

        // ACK [error@command_listNum] {current_command} message_text

        for (i, chr) in output.chars().enumerate() {
            match state {
                FindACK => {
                    let ack = "ACK";
                    if let Some(ack_chr) = ack.chars().nth(i) {
                        if chr != ack_chr {
                            return Err(ParseMPDError::expected(ack_chr, i));
                        }
                    }
                    if i == ack.chars().count() - 1 {
                        state = FindLeftBracket;
                    }
                }
                FindLeftBracket => {
                    if chr == '[' {
                        state = GetErrorType;
                    } else if chr != ' ' {
                        return Err(ParseMPDError::expected('[', i));
                    }
                }
                GetErrorType => {
                    if chr == '@' {
                        error_kind = ErrorKind::from_code(match temp.parse::<usize>() {
                            Ok(i) => i,
                            Err(_) => {
                                return Err(ParseMPDError::number(i));
                            }
                        });

                        if error_kind.is_none() {
                            return Err(ParseMPDError::new(InvalidCode, i - 1));
                        }

                        temp.clear();
                        state = GetListNum;
                    } else if chr < '0' || chr > '9' {
                        return Err(ParseMPDError::number(i));
                    } else {
                        temp.push(chr);
                    }
                }
                GetListNum => {
                    if chr == ']' {
                        list_number = match temp.parse() {
                            Ok(i) => Some(i),
                            Err(_) => {
                                return Err(ParseMPDError::number(i));
                            }
                        };
                        temp.clear();
                        state = FindLeftBrace;
                    } else if chr < '0' || chr > '9' {
                        return Err(ParseMPDError::number(i));
                    } else {
                        temp.push(chr);
                    }
                }
                FindLeftBrace => {
                    if chr == '{' {
                        state = GetFailedCommand;
                    } else if chr != ' ' {
                        return Err(ParseMPDError::expected('{', i));
                    }
                }
                GetFailedCommand => {
                    if chr == '}' {
                        failed_command = Some(temp.clone());
                        temp.clear();
                        state = GetErrorMessage;
                    } else {
                        temp.push(chr);
                    }
                }
                GetErrorMessage => {
                    temp.push(chr);
                }
            };
        }

        temp = temp.trim().to_string();

        if error_kind.is_none() || list_number.is_none() || failed_command.is_none() || temp.is_empty() {
            return Err(ParseMPDError::new(
                UnexpectedSymbol,
                output.chars().count() - 1,
            ));
        }

        Ok(Self {
            kind: error_kind.unwrap(),
            stored: ParsedError {
                current_command: failed_command.unwrap(),
                list_num: list_number.unwrap(),
                message_text: temp,
            }
            .into(),
        })
    }
}

/// An error which can be returned when parsing a String into an [Error]
///
/// This `struct` is created when using the [`Error::try_from_mpd`] method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseMPDError {
    pub kind: ParseMPDErrorKind,
    /// The character position (0 indexed) at which the parsing failed
    pub pos: usize,
    /// The character the parser expected to find
    pub expected_char: Option<char>,
}

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
        write!(f, " at position {}\n", self.pos)
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

/// State of the MPD Error parser
enum ParseState {
    FindACK,
    FindLeftBracket,
    GetErrorType,
    GetListNum,
    FindLeftBrace,
    GetFailedCommand,
    GetErrorMessage,
}

#[derive(Debug)]
struct StrMessageError {
    message: &'static str,
}

#[derive(Debug)]
struct StringMessageError {
    message: String,
}

#[derive(Debug)]
struct SourceError {
    source: Box<dyn stdError>,
}

#[derive(Debug)]
struct ParsedError {
    current_command: String,
    list_num: usize,
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
