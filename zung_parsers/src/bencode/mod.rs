mod de;
mod error;
mod value;

pub use de::{from_bytes, from_str};
pub use error::{Error, Result};
pub use value::Value;

use std::{collections::HashMap, fmt::Display};
use value::ValueInput;

/// Parsed in string if string is passed in as the input. Parses in bytes if bytes are passed in as
/// the input.
pub fn to_value<'a, T>(input: T) -> Result<Value>
where
    T: Into<ValueInput<'a>>,
{
    match input.into() {
        ValueInput::Str(s) => {
            let mut bencode = Bencode {
                input: s.as_bytes(),
                in_bytes: false,
            };

            bencode.parse()
        }
        ValueInput::Bytes(b) => {
            let mut bencode = Bencode {
                input: b,
                in_bytes: true,
            };

            bencode.parse()
        }
    }
}

/// Encoding and decoding data in the [Bencode](https://en.wikipedia.org/wiki/Bencode) format.
///
/// Bencode is a simple binary encoding format used in various contexts, most notably in
/// BitTorrent. This type provides functionality to encode Rust data structures into Bencode
/// format and decode Bencode strings into Rust data structures or json or yaml. See the
/// implemented methods for more information,
///
/// # Usage
///
/// ## Examples
///
/// ```rust
/// use zung_parsers::bencode;
///
/// // Creating a Bencode instance from a bencode-encoded string
/// let bencode_string = "i42e";
/// let bencode = bencode::to_value(bencode_string).unwrap();
///
/// println!("{bencode}"); // Prints "42"
/// ```
struct Bencode<'a> {
    input: &'a [u8],
    in_bytes: bool,
}

impl<'a> Bencode<'a> {
    pub(crate) fn from_str(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            in_bytes: false,
        }
    }

    pub(crate) fn from_bytes(input: &'a [u8]) -> Self {
        Self {
            input,
            in_bytes: true,
        }
    }

    pub(crate) fn parse(&mut self) -> Result<Value> {
        if self.input.is_empty() {
            return Err(Error::EndOfStream);
        }

        match self.input[0] {
            b'0'..=b'9' => {
                let value = self.parse_bytes()?;
                if self.in_bytes {
                    Ok(Value::Bytes(value))
                } else {
                    let string =
                        String::from_utf8(value).map_err(|e| Error::Custom(e.to_string()))?;
                    Ok(Value::String(string))
                }
            }
            b'i' => {
                let value = self.parse_integer()?;
                Ok(Value::Integer(value))
            }
            b'l' => {
                let value = self.parse_list()?;
                Ok(Value::List(value))
            }
            b'd' => {
                let value = self.parse_dictionary()?;
                Ok(Value::Dictionary(value))
            }
            _ => Err(Error::InvalidType("Invalid bencode format".to_string())),
        }
    }

    pub(crate) fn parse_integer(&mut self) -> Result<i64> {
        // Find the position of the ending 'e'
        let end_pos = self.input.iter().position(|&b| b == b'e').ok_or_else(|| {
            Error::InvalidValue("Invalid integer bencode format: missing 'e'".to_string())
        })?;

        // Slice out the byte range between 'i' and 'e'
        let int_bytes = &self.input[1..end_pos];

        // Check if it's an empty integer
        if int_bytes.is_empty() {
            return Err(Error::InvalidType(
                "Invalid bencode integer format: empty integer".to_string(),
            ));
        }

        // Parse the integer manually, allowing for a possible negative sign
        let mut is_negative = false;
        let mut value: i64 = 0;
        let mut iter = int_bytes.iter();

        // Check for negative sign.
        if int_bytes[0] == b'-' {
            is_negative = true;

            // Move on from the negative sign
            iter.next();
        }

        // Manually parse the number from the remaining bytes
        for &byte in iter {
            if !byte.is_ascii_digit() {
                return Err(Error::InvalidType(
                    "Invalid character in bencode integer".to_string(),
                ));
            }

            value = value
                // multiply by 10 to “shift” the previous digits and add the new digit,
                // which builds the final number
                .checked_mul(10)
                // Subtracting the ASCII value of '0' (which is b'0' == 48) converts the byte to
                // its numeric value. For example, if byte is b'3', the result would be 3.
                .and_then(|v| v.checked_add((byte - b'0') as i64))
                .ok_or_else(|| Error::InvalidValue("Integer overflow".to_string()))?;
        }

        // Handle leading zeros (only '0' is allowed to start with zero, otherwise it's invalid)
        if int_bytes.starts_with(b"0") && int_bytes.len() > 1 {
            return Err(Error::InvalidType(
                "Invalid integer bencode integer format: leading zeros".to_string(),
            ));
        }

        // Apply the negative sign if necessary
        if is_negative {
            value = -value;
        }

        // Update the input to consume the parsed part (skip the 'e')
        self.input = &self.input[end_pos + 1..];

        Ok(value)
    }

    pub(crate) fn parse_bytes(&mut self) -> Result<Vec<u8>> {
        let colon_pos = self.input.iter().position(|p| *p == b':').ok_or_else(|| {
            Error::InvalidValue("Invalid string bencode format: missing ':'".to_string())
        })?;

        let len = self.input[..colon_pos]
            .iter()
            .try_fold(0usize, |acc, byte| {
                if byte.is_ascii_digit() {
                    // This expression converts the current byte (which represents an ASCII
                    // digit) to its numeric value:
                    //
                    // • byte - b'0': Subtracting the ASCII value of '0' (which is b'0' == 48)
                    //   converts the byte to its numeric value. For example, if byte is b'3', the
                    //   result would be 3.
                    //
                    // • acc * 10 + (byte - b'0'): This accumulates the numeric value of the byte.
                    //   We multiply acc by 10 to “shift” the previous digits and add the new digit,
                    //   which builds the final number.
                    //
                    // • Example: If the bytes are [b'1', b'2', b'3'], the iteration will result in:
                    //   •	acc = 0: after the first byte (b'1'), it becomes acc = 0 * 10 + 1 = 1.
                    //   •	acc = 1: after the second byte (b'2'), it becomes acc = 1 * 10 + 2 = 12.
                    //   •	acc = 12: after the third byte (b'3'), it becomes acc = 12 * 10 + 3 = 123.
                    Ok(acc * 10 + (byte - b'0') as usize)
                } else {
                    Err(Error::InvalidType(format!(
                        "Non Digit character found in the length of the string: '{}'",
                        String::from_utf8([*byte].to_vec()).unwrap()
                    )))
                }
            })?;

        let rest = &self.input[colon_pos + 1..];
        if len > rest.len() {
            return Err(Error::InvalidType(
                "Invalid string bencode format: length is higher than the remaining bytes"
                    .to_string(),
            ));
        }

        let (string, remainder) = rest.split_at(len);

        self.input = remainder;

        Ok(string.to_vec())
    }

    pub(crate) fn parse_list(&mut self) -> Result<Vec<Value>> {
        let mut list = Vec::new();

        // eat the 'l' tag
        self.input = &self.input[1..];

        while !self.input.is_empty() && self.input[0] != b'e' {
            list.push(self.parse()?);
        }

        // eat the 'e' tag
        if self.input.first() == Some(&b'e') {
            self.input = &self.input[1..];
        } else {
            return Err(Error::InvalidType(
                "Invalid list format: missing 'e'".to_string(),
            ));
        }

        Ok(list)
    }

    pub(crate) fn parse_dictionary(&mut self) -> Result<HashMap<String, Value>> {
        let mut dictionary = HashMap::new();

        // eat the 'd' tag
        self.input = &self.input[1..];

        while !self.input.is_empty() && self.input[0] != b'e' {
            let k = match self.parse()? {
                Value::String(key) => key, // If it's a valid string
                Value::Bytes(bytes) => {
                    String::from_utf8(bytes).map_err(|e| Error::Custom(e.to_string()))?
                } // Convert bytes to String
                _ => {
                    return Err(Error::InvalidType(
                        "Only string values are allowed as dictionary keys".to_string(),
                    ));
                }
            };

            let v = self.parse()?;
            dictionary.insert(k, v);
        }

        // eat the 'e' tag
        if self.input.first() == Some(&b'e') {
            self.input = &self.input[1..];
        } else {
            return Err(Error::InvalidType(
                "Invalid dictionary format: missing 'e'".to_string(),
            ));
        }

        Ok(dictionary)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "Null"),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Bytes(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => write!(f, "{s}"),
                Err(_) => write!(f, "/*BYTES*/"),
            },
            Value::String(s) => write!(f, "{s}"),
            Value::List(list) => {
                let mut benstr = String::new();
                for (i, bencode) in list.iter().enumerate() {
                    if i != list.len() - 1 {
                        benstr.push_str(&format!("{}, ", bencode));
                    } else {
                        benstr.push_str(&bencode.to_string())
                    }
                }
                write!(f, "[{benstr}]")
            }
            Value::Dictionary(dictionary) => {
                let mut benstr = String::new();
                for (k, v) in dictionary {
                    benstr.push_str(&format!("{k} : "));
                    benstr.push_str(&v.to_string());
                }
                write!(f, "[{benstr}]")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string() {
        let bencode = to_value("5:hello").unwrap();
        assert_eq!(Value::String(String::from("hello")), bencode);

        let bencode_err = to_value(b"10:hello");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid string bencode format: length is higher than the remaining bytes",
            bencode_err.unwrap_err().to_string()
        );

        let bencode_err = to_value(b"1d0:hello");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Non Digit character found in the length of the string: 'd'",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn test_parse_integer() {
        let bencode = to_value(b"i21e").unwrap();
        assert_eq!(Value::Integer(21), bencode);

        let bencode = to_value(b"i-21e").unwrap();
        assert_eq!(Value::Integer(-21), bencode);

        let bencode_err = to_value(b"i32je");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid character in bencode integer",
            bencode_err.unwrap_err().to_string()
        );

        let bencode_err = to_value(b"ie");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid bencode integer format: empty integer",
            bencode_err.unwrap_err().to_string()
        );

        let bencode_err = to_value(b"i004e");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid integer bencode integer format: leading zeros",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn parse_list() {
        let bencode = to_value("li32ei42ei52e5:helloe").unwrap();
        assert_eq!(
            Value::List(vec![
                Value::Integer(32),
                Value::Integer(42),
                Value::Integer(52),
                Value::String("hello".to_string())
            ]),
            bencode
        );

        let bencode_err = to_value(b"li32ei42ei52e5:hello");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid list format: missing 'e'",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn test_dictionary_bencode() {
        let bencode = to_value("d3:cow3:moo4:spam4:eggse").unwrap();
        let mut dictionary = HashMap::new();
        dictionary.insert("cow".to_string(), Value::String("moo".to_string()));
        dictionary.insert("spam".to_string(), Value::String("eggs".to_string()));
        assert_eq!(bencode, Value::Dictionary(dictionary));

        let bencode = to_value("d3:cow3:moo4:spam4:eggse").unwrap();
        let mut dictionary = HashMap::new();
        dictionary.insert("cow".to_string(), Value::String("moo".to_string()));
        dictionary.insert("spam".to_string(), Value::String("eggs".to_string()));
        assert_eq!(bencode, Value::Dictionary(dictionary));

        let bencode_err = to_value("di2e3:moo4:spam4:eggse");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Only string values are allowed as dictionary keys",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn invalid_becode() {
        let bencode_err = to_value("werd");

        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid bencode format",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn test_empty_input() {
        let bencode = to_value("");
        assert!(bencode.is_err());
        assert_eq!("End of stream", bencode.unwrap_err().to_string());
    }
}
