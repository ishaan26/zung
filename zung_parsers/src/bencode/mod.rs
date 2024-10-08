use core::str;
use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

pub struct Bencode<'a> {
    input: &'a [u8],
    in_bytes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Integer(i64),
    Bytes(Vec<u8>),
    String(String),
    List(Vec<Value>),
    Dictionary(HashMap<String, Value>),
}

impl<'a> Bencode<'a> {
    pub fn from_string(input: &'a str) -> Result<Value> {
        let mut bencode = Self {
            input: input.as_bytes(),
            in_bytes: false,
        };

        bencode.parse()
    }

    pub fn from_bytes(input: &'a [u8]) -> Result<Value> {
        let mut bencode = Self {
            input,
            in_bytes: true,
        };
        bencode.parse()
    }

    pub(crate) fn parse(&mut self) -> Result<Value> {
        if self.input.is_empty() {
            bail!("Unexpected end of input while parsing list");
        }

        match self.input[0] {
            b'0'..=b'9' => {
                let value = self.parse_bytes()?;
                if self.in_bytes {
                    Ok(Value::Bytes(value))
                } else {
                    Ok(Value::String(String::from_utf8(value)?))
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
            _ => bail!("Invalid bencode format"),
        }
    }

    pub(crate) fn parse_integer(&mut self) -> Result<i64> {
        // Find the position of the ending 'e'
        let end_pos = self
            .input
            .iter()
            .position(|&b| b == b'e')
            .ok_or_else(|| anyhow!("Invalid integer bencode format: missing 'e'"))?;

        // Slice out the byte range between 'i' and 'e'
        let int_bytes = &self.input[1..end_pos];

        // Check if it's an empty integer
        if int_bytes.is_empty() {
            bail!("Invalid integer bencode format: empty integer");
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
                bail!("Invalid character in bencode integer");
            }

            value = value
                // multiply by 10 to “shift” the previous digits and add the new digit,
                // which builds the final number
                .checked_mul(10)
                // Subtracting the ASCII value of '0' (which is b'0' == 48) converts the byte to
                // its numeric value. For example, if byte is b'3', the result would be 3.
                .and_then(|v| v.checked_add((byte - b'0') as i64))
                .ok_or_else(|| anyhow!("Integer overflow"))?;
        }

        // Handle leading zeros (only '0' is allowed to start with zero, otherwise it's invalid)
        if int_bytes.starts_with(b"0") && int_bytes.len() > 1 {
            bail!("Invalid integer bencode integer format: leading zeros");
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
        let colon_pos = self
            .input
            .iter()
            .position(|p| *p == b':')
            .ok_or_else(|| anyhow!("Invalid string bencode format: missing ':'"))?;

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
                    bail!(
                        "Non Digit character found in the length of the string: '{}'",
                        String::from_utf8([*byte].to_vec())?
                    )
                }
            })?;

        let rest = &self.input[colon_pos + 1..];
        if len > rest.len() {
            bail!("Invalid string bencode format: length is higher than the remaining bytes");
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
            bail!("Invalid list format: missing 'e'");
        }

        Ok(list)
    }

    pub(crate) fn parse_dictionary(&mut self) -> Result<HashMap<String, Value>> {
        let mut dictionary = HashMap::new();

        // eat the 'd' tag
        self.input = &self.input[1..];

        while !self.input.is_empty() && self.input[0] != b'e' {
            let k = match self.parse()? {
                Value::String(key) => key,                        // If it's a valid string
                Value::Bytes(bytes) => String::from_utf8(bytes)?, // Convert bytes to String
                _ => bail!("Only string values are allowed as dictionary keys"),
            };

            let v = self.parse()?;
            dictionary.insert(k, v);
        }

        // eat the 'e' tag
        if self.input.first() == Some(&b'e') {
            self.input = &self.input[1..];
        } else {
            bail!("Invalid dictionary format: missing 'e'");
        }

        Ok(dictionary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string() {
        let bencode = Bencode::from_string("5:hello").unwrap();
        assert_eq!(Value::String(String::from("hello")), bencode);

        let bencode_err = Bencode::from_bytes(b"10:hello");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid string bencode format: length is higher than the remaining bytes",
            bencode_err.unwrap_err().to_string()
        );

        let bencode_err = Bencode::from_bytes(b"1d0:hello");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Non Digit character found in the length of the string: 'd'",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn test_parse_integer() {
        let bencode = Bencode::from_bytes(b"i21e").unwrap();
        assert_eq!(Value::Integer(21), bencode);

        let bencode = Bencode::from_bytes(b"i-21e").unwrap();
        assert_eq!(Value::Integer(-21), bencode);

        let bencode_err = Bencode::from_bytes(b"i32je");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid character in bencode integer",
            bencode_err.unwrap_err().to_string()
        );

        let bencode_err = Bencode::from_bytes(b"ie");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid integer bencode format: empty integer",
            bencode_err.unwrap_err().to_string()
        );

        let bencode_err = Bencode::from_bytes(b"i004e");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid integer bencode integer format: leading zeros",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn parse_list() {
        let bencode = Bencode::from_string("li32ei42ei52e5:helloe").unwrap();
        assert_eq!(
            Value::List(vec![
                Value::Integer(32),
                Value::Integer(42),
                Value::Integer(52),
                Value::String("hello".to_string())
            ]),
            bencode
        );

        let bencode_err = Bencode::from_bytes(b"li32ei42ei52e5:hello");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid list format: missing 'e'",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn test_dictionary_bencode() {
        let bencode = Bencode::from_string("d3:cow3:moo4:spam4:eggse").unwrap();
        let mut dictionary = HashMap::new();
        dictionary.insert("cow".to_string(), Value::String("moo".to_string()));
        dictionary.insert("spam".to_string(), Value::String("eggs".to_string()));
        assert_eq!(bencode, Value::Dictionary(dictionary));

        let bencode_err = Bencode::from_string("di2e3:moo4:spam4:eggse");
        assert!(bencode_err.is_err());
        assert_eq!(
            "Only string values are allowed as dictionary keys",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn invalid_becode() {
        let bencode_err = Bencode::from_string("werd");

        assert!(bencode_err.is_err());
        assert_eq!(
            "Invalid bencode format",
            bencode_err.unwrap_err().to_string()
        );
    }

    #[test]
    fn test_empty_input() {
        let bencode = Bencode::from_string("");
        assert!(bencode.is_err());
        assert_eq!(
            "Unexpected end of input while parsing list",
            bencode.unwrap_err().to_string()
        );
    }
}
