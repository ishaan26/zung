/*
This module is directly taken from the serde_bencode library with only a few minor changes.

It is licensed under MIT License as follows:

The MIT License (MIT)

Copyright (c) 2014 Ty Overby

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/

use serde::de::Error as DeError;
use serde::de::{Expected, Unexpected};
use serde::ser::Error as SerError;
use std::error::Error as StdError;
use std::fmt;
use std::fmt::Display;
use std::io::Error as IoError;
use std::result::Result as StdResult;

/// Alias for `Result<T, serde_bencode::Error>`.
pub type Result<T> = StdResult<T, Error>;

/// Represents all possible errors which can occur when serializing or deserializing bencode.
#[derive(Debug)]
pub enum Error {
    /// Raised when an IO error occurred.
    IoErr(IoError),

    /// Raised when the value being deserialized is of the incorrect type.
    InvalidType(String),

    /// Raised when the value being deserialized is of the right type, but is wrong for some other
    /// reason. For example, this error may occur when deserializing to a String but the input data
    /// is not valid UTF-8.
    InvalidValue(String),

    /// Raised when deserializing a sequence or map, but the input data is the wrong length.
    InvalidLength(String),

    /// Raised when deserializing an enum, but the variant has an unrecognized name.
    UnknownVariant(String),

    /// Raised when deserializing a struct, but there was a field which does not match any of the
    /// expected fields.
    UnknownField(String),

    /// Raised when deserializing a struct, but there was a field which was expected but not
    /// present.
    MissingField(String),

    /// Raised when deserializing a struct, but there is more than one field with the same name.
    DuplicateField(String),

    /// Catchall for any other kind of error.
    Custom(String),

    /// Unexpected end of input stream.
    EndOfStream,
}

impl SerError for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

impl DeError for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }

    fn invalid_type(unexpected: Unexpected<'_>, exp: &dyn Expected) -> Self {
        Error::InvalidType(format!("Invalid Type: {unexpected} (expected: `{exp}`)"))
    }

    fn invalid_value(unexpected: Unexpected<'_>, exp: &dyn Expected) -> Self {
        Error::InvalidValue(format!("Invalid Value: {unexpected} (expected: `{exp}`)"))
    }

    fn invalid_length(len: usize, exp: &dyn Expected) -> Self {
        Error::InvalidLength(format!("Invalid Length: {len} (expected: {exp})"))
    }

    fn unknown_variant(field: &str, expected: &'static [&'static str]) -> Self {
        Error::UnknownVariant(format!(
            "Unknown Variant: `{field}` (expected one of: {expected:?})"
        ))
    }

    fn unknown_field(field: &str, expected: &'static [&'static str]) -> Self {
        Error::UnknownField(format!(
            "Unknown Field: `{field}` (expected one of: {expected:?})"
        ))
    }

    fn missing_field(field: &'static str) -> Self {
        Error::MissingField(format!("Missing Field: `{field}`"))
    }

    fn duplicate_field(field: &'static str) -> Self {
        Error::DuplicateField(format!("Duplicate Field: `{field}`"))
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match *self {
            Error::IoErr(ref error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match *self {
            Error::IoErr(ref error) => return error.fmt(f),
            Error::InvalidType(ref s)
            | Error::InvalidValue(ref s)
            | Error::InvalidLength(ref s)
            | Error::UnknownVariant(ref s)
            | Error::UnknownField(ref s)
            | Error::MissingField(ref s)
            | Error::DuplicateField(ref s)
            | Error::Custom(ref s) => s,
            Error::EndOfStream => "End of stream",
        };
        f.write_str(message)
    }
}
