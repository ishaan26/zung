mod deser;
mod error;
mod ser;

use std::{collections::HashMap, fmt::Display};

use anyhow::{bail, Error, Result};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::bencode;

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
/// use zung_parsers::bencode::Bencode;
///
/// // Creating a Bencode instance from a bencode-encoded string
/// let bencode_string = "i42e";
/// let bencode = Bencode::from(bencode_string);
///
/// assert_eq!(bencode.to_string(), "42");
///
/// println!("{bencode}");
/// // Prints "42"
/// ```
#[derive(Debug)]
pub struct Bencode {
    bencode: Value,
    str_remainder: String,
    byte_remainder: Vec<u8>,
}

/// An enum representing various data types used in Bencode encoding.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Value {
    /// Reprasents a string value.
    String(String),

    /// Represents a byte sequence value.
    Bytes(Vec<u8>),

    /// Represents an integer value.
    Integer(isize),

    /// Represents a list of `Value` variants.
    List(Vec<Value>),

    /// Represents a dictionary where keys are strings and values are `Value` variants.
    Dictionary(HashMap<String, Value>),

    /// Represents the null value.
    Null,
}

impl Bencode {
    /// Parses a Bencode-encoded string and returns a `Bencode` instance.
    ///
    /// # Arguments
    ///
    /// * `pattern` - A string or a value that can be converted to a string using the `ToString`
    ///    trait, representing the Bencode-encoded data.
    ///
    /// # Returns
    ///
    /// * `Result<Self, Error>` - Returns a `Bencode` instance on success or an `Error` on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use zung_parsers::bencode::Bencode;
    ///
    /// let bencode_string = "i42e";
    /// let bencode = Bencode::from_string(bencode_string).unwrap();
    /// assert_eq!(bencode.to_string(), "42");
    ///
    /// let bencode_string = "l5:hello4:from4:rust7:bencode";
    /// let bencode = Bencode::from_string(bencode_string).unwrap();
    ///
    /// assert_eq!(bencode.to_string(), "[hello, from, rust, bencode]");
    /// ```
    pub fn from_string(pattern: impl ToString) -> Result<Self, Error> {
        let pattern = pattern.to_string();
        let (tag, value) = pattern.split_at(1);
        let tag = tag.chars().next().unwrap();

        match tag {
            // Strings
            '0'..='9' => match pattern.split_once(':') {
                Some((len, string)) => {
                    let len = len.parse::<usize>().unwrap_or_else(|e| {
                        panic!("Cannot parse the string: {} due to {e}", string)
                    });

                    if let Some(s) = string.get(..len) {
                        Ok(Bencode {
                            bencode: Value::String(s.to_string()),
                            str_remainder: string[len..].to_string(),
                            byte_remainder: Vec::new(),
                        })
                    } else {
                        bail!("Invalid encoding of string value")
                    }
                }
                None => bail!("unknown value provided"),
            },

            // Integers
            'i' => {
                if let Some((integer, rest)) = value.split_once('e') {
                    let integer = integer.parse::<isize>().unwrap_or_else(|e| {
                        panic!("Cannot parse the integer: {} due to {e}", integer)
                    });

                    Ok(Bencode {
                        bencode: Value::Integer(integer),
                        str_remainder: rest.to_string(),
                        byte_remainder: Vec::new(),
                    })
                } else {
                    bail!("Invalid encoding of integer value")
                }
            }

            // Lists
            'l' => {
                // TODO: use Vector
                let mut list = Vec::new();

                let mut remainder = value.to_string();

                while !remainder.is_empty() && !remainder.starts_with('e') {
                    let bencode = Bencode::from_string(remainder)?;
                    list.push(bencode.bencode);
                    remainder = bencode.str_remainder;
                }

                if let Some(string) = remainder.strip_prefix('e') {
                    remainder = string.to_string();
                }

                Ok(Bencode {
                    bencode: Value::List(list),
                    str_remainder: remainder,
                    byte_remainder: Vec::new(),
                })
            }

            // Dictionaries
            'd' => {
                let mut dictionary = HashMap::new();

                let mut remainder = value.to_string();

                while !remainder.is_empty() && !remainder.starts_with('e') {
                    let k = Bencode::from_string(remainder)?;
                    let v = Bencode::from_string(k.str_remainder.clone())?;
                    dictionary.insert(k.bencode.to_string(), v.bencode);
                    remainder = v.str_remainder;
                }

                if let Some(string) = remainder.strip_prefix('e') {
                    remainder = string.to_string();
                }

                Ok(Bencode {
                    bencode: Value::Dictionary(dictionary),
                    str_remainder: remainder,
                    byte_remainder: Vec::new(),
                })
            }

            // Null value
            'n' => Ok(Bencode {
                bencode: Value::Null,
                str_remainder: String::new(),
                byte_remainder: Vec::new(),
            }),

            // Not Bencode
            _ => bail!("unknown value provided"),
        }
    }

    /// Parses a Bencode-encoded byte slice and returns a `Bencode` instance.
    ///
    /// # Arguments
    ///
    /// * `bytes` - A slice of bytes (`&[u8]`) representing the Bencode-encoded data.
    ///
    /// # Returns
    ///
    /// * `Result<Self, Error>` - Returns a `Bencode` instance on success or an `Error` on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use zung_parsers::bencode::Bencode;
    ///
    /// let bencode_bytes = [105, 52, 50, 101]; // Bytes representing "i42e"
    /// let bencode = Bencode::from_bytes(&bencode_bytes).unwrap();
    /// assert_eq!(bencode.to_string(), "42");
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let (tag, value) = (&bytes[0], &bytes[1..]);

        match *tag {
            // Strings
            b'0'..=b'9' => {
                let (len, rest) = match bytes.iter().position(|pos| *pos == b':') {
                    Some(pos) => (&bytes[..pos], &bytes[pos + 1..]),
                    None => bail!("Invalid string bencode format: missing ':'"),
                };

                let len = std::str::from_utf8(len)?;
                let len: usize = len.parse()?;

                let (string, remainder) = (&rest[..len], &rest[len..]);

                match String::from_utf8(string.to_vec()) {
                    Ok(string) => Ok(Bencode {
                        bencode: Value::String(string),
                        str_remainder: String::new(),
                        byte_remainder: remainder.to_vec(),
                    }),
                    Err(_) => Ok(Bencode {
                        bencode: Value::Bytes(string.to_vec()),
                        str_remainder: String::new(),
                        byte_remainder: remainder.to_vec(),
                    }),
                }
            }

            // Integers
            b'i' => {
                let (integer, remainder) = match value.iter().position(|p| *p == b'e') {
                    Some(pos) => (&value[..pos], &value[pos + 1..]),
                    None => bail!("Cannot find the end tag of the Integer"),
                };

                let integer = String::from_utf8(integer.to_vec())?;
                let integer = integer.parse::<isize>()?;

                Ok(Bencode {
                    bencode: Value::Integer(integer),
                    str_remainder: String::new(),
                    byte_remainder: remainder.to_vec(),
                })
            }

            // Lists
            b'l' => {
                // TODO: use Vector
                let mut list = Vec::new();

                let mut remainder = value.to_vec();

                while !remainder.is_empty() && remainder[0] != b'e' {
                    let bencode = Bencode::from_bytes(&remainder)?;
                    list.push(bencode.bencode);
                    remainder = bencode.byte_remainder;
                }

                if remainder.len() > 1 && remainder[0] == b'e' {
                    remainder = remainder[1..].to_vec();
                }

                Ok(Bencode {
                    bencode: Value::List(list),
                    str_remainder: String::new(),
                    byte_remainder: remainder,
                })
            }

            // Dictionaries
            b'd' => {
                let mut dictionary = HashMap::with_capacity(value.len());

                let mut remainder = value.to_vec();

                while !remainder.is_empty() && remainder[0] != b'e' {
                    let k = Bencode::from_bytes(&remainder)?;
                    let v = Bencode::from_bytes(&k.byte_remainder)?;
                    dictionary.insert(k.bencode.to_string(), v.bencode);
                    remainder = v.byte_remainder;
                }

                if remainder.len() > 1 && remainder[0] == b'e' {
                    remainder = remainder[1..].to_vec();
                }

                Ok(Bencode {
                    bencode: Value::Dictionary(dictionary),
                    str_remainder: String::new(),
                    byte_remainder: remainder,
                })
            }

            // Null value
            b'n' => Ok(Bencode {
                bencode: Value::Null,
                str_remainder: String::new(),
                byte_remainder: Vec::new(),
            }),

            _ => bail!("Not a bencode value"),
        }
    }

    /// Returns an immutable reference to the internal `Value` representation of the Bencode data.
    ///
    /// # Examples
    ///
    /// ```
    /// use zung_parsers::bencode::Bencode;
    /// use zung_parsers::bencode::Value;
    /// use std::collections::HashMap;
    ///
    /// let bencode = Bencode::from("i42e");
    /// let value = bencode.get_value();
    /// assert_eq!(value, &Value::Integer(42));
    ///
    /// let bencode = Bencode::from(b"d5:helloi5ee");
    /// let value = bencode.get_value();
    ///
    /// let mut map = HashMap::new();
    /// map.insert("hello".to_string(), Value::Integer(5));
    ///
    /// assert_eq!(value, &Value::Dictionary(map));
    /// ```
    pub fn get_value(&self) -> &Value {
        &self.bencode
    }

    /// Returns a mutable reference to the internal `Value` representation of the Bencode data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use zung_parsers::bencode::{Bencode, Value};
    /// let mut bencode = Bencode::from("l3:fooe");
    /// if let Value::List(list) = bencode.get_value_mut() {
    ///     list.push(Value::String("bar".to_string()));
    /// }
    ///
    /// assert_eq!(
    ///     bencode.get_value(),
    ///     &Value::List(vec![
    ///         Value::String("foo".to_string()),
    ///         Value::String("bar".to_string())
    ///     ])
    /// );    
    /// ```
    pub fn get_value_mut(&mut self) -> &mut Value {
        &mut self.bencode
    }

    /// Returns a [`String`] representation of the Bencode data in the Bencode encoding format.
    ///
    /// # Panics
    ///
    /// Panics if the bencode data contains non utf-8 bytes
    ///
    /// # Examples
    ///
    /// ```
    /// # use zung_parsers::bencode::Bencode;
    /// let bencode = Bencode::from_string("d3:key3:vale").unwrap();
    /// assert_eq!(bencode.to_bencode_string(), "d3:key3:vale");
    /// ```
    pub fn to_bencode_string(&self) -> String {
        self.bencode.to_bencode_string()
    }

    /// Converts the [`Bencode`] data to a [serde_json::Value].
    ///
    /// This function leverages the [`serde`] framework to convert the data within the
    /// `Bencode` struct (likely a `Value` variant) into a `serde_json::Value`. This allows for
    /// easier interaction with JSON data structures.
    ///
    /// ## Returns
    ///
    /// A `Result` containing either a `serde_json::Value` on success or an error
    /// if the conversion fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # use zung_parsers::bencode::Bencode;
    /// # use serde_json::*;
    /// let bencode = Bencode::from_string("d3:key3:vale").unwrap();
    /// let json_value = bencode.to_json().unwrap();
    /// assert_eq!(json_value, serde_json::json!({"key": "val"}));
    /// ```
    pub fn to_json(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(&self.bencode)?)
    }

    /// Converts the Bencode data to a pretty-printed JSON `String` representation.
    ///
    /// This function first converts the [`Bencode`] data to a [`serde_json::Value`] using
    /// `to_json` and then uses [`serde_json::to_string_pretty`] to generate a human-readable
    /// JSON string with indentation.
    /// # Examples
    ///
    /// ```
    /// # use zung_parsers::bencode::Bencode;
    /// let bencode = Bencode::from_string("d3:key3:vale").unwrap();
    /// let json_string = bencode.to_json_pretty().unwrap();
    /// assert_eq!(json_string, "{\n  \"key\": \"val\"\n}");
    /// ```
    pub fn to_json_pretty(&self) -> Result<String> {
        let value = self.to_json()?;
        Ok(serde_json::to_string_pretty(&value)?)
    }

    /// Converts the [`Bencode`] data to [`serde_yaml::Value`].
    ///
    /// This function leverages the [`serde`] framework to convert the data within the
    /// `Bencode` struct (likely a `Value` variant) into a `serde_yaml::Value`.
    /// This allows for easier interaction with YAML data structures.
    ///
    /// ## Returns
    ///
    /// A `Result` containing either a `serde_yaml::Value` on success or an error
    /// if the conversion fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # use zung_parsers::bencode::Bencode;
    /// # use serde_yaml::*;
    /// let bencode = Bencode::from_string("d3:key3:vale").unwrap();
    /// let yaml_value = bencode.to_yaml_value().unwrap();
    ///
    /// assert_eq!(
    ///     yaml_value,
    ///     serde_yaml::Value::Mapping(
    ///         vec![(
    ///             serde_yaml::Value::String("key".to_string()),
    ///             serde_yaml::Value::String("val".to_string())
    ///         )]
    ///         .into_iter()
    ///         .collect()
    ///     )
    /// );
    /// ```
    pub fn to_yaml_value(&self) -> Result<serde_yaml::Value> {
        Ok(serde_yaml::to_value(&self.bencode)?)
    }

    pub fn to_yaml_string(&self) -> Result<String> {
        Ok(serde_yaml::to_string(&self.bencode)?)
    }

    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(&self.bencode)?)
    }

    pub fn deserialize_into<'a, T>(bytes: &'a [u8]) -> Result<T, bencode::error::Error>
    where
        T: Deserialize<'a>,
    {
        bencode::deser::from_bytes(bytes)
    }

    pub fn serialize_to_string<T>(value: &T) -> Result<String, bencode::error::Error>
    where
        T: Serialize,
    {
        ser::to_string(value)
    }

    pub fn serialize_to_bytes<T>(value: &T) -> Result<Vec<u8>, bencode::error::Error>
    where
        T: Serialize,
    {
        ser::to_bencode_bytes(value)
    }

    pub fn serialize_to_self<T>(value: &T) -> Result<Self, bencode::error::Error>
    where
        T: Serialize,
    {
        Ok(Self::from(ser::to_bencode_bytes(value)?))
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(string) => write!(f, "{string}"),
            Value::Integer(integer) => write!(f, "{integer}"),
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
            Value::Null => write!(f, "Null"),
            Value::Bytes(_) => panic!(
                "Bencode contains non utf8 characters which cannot be converted into bencode"
            ),
        }
    }
}

impl Value {
    pub fn to_bencode_string(&self) -> String {
        match &self {
            Value::String(string) => format!("{}:{string}", string.len()),
            Value::Integer(integer) => format!("i{integer}e"),
            Value::List(list) => {
                let mut benstr = String::new();
                for bencode in list {
                    benstr.push_str(&bencode.to_bencode_string())
                }
                format!("l{benstr}e")
            }
            Value::Dictionary(dictionary) => {
                let mut benstr = String::new();
                for (k, v) in dictionary {
                    benstr.push_str(&format!("{}:{k}", k.len()));
                    benstr.push_str(&v.to_bencode_string());
                }
                format!("d{benstr}e")
            }
            Value::Null => String::from('n'),
            Value::Bytes(bytes) => format!("{}:{bytes:?}", bytes.len()),
        }
    }
}

impl Display for Bencode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.bencode)
    }
}

impl PartialEq for Bencode {
    fn eq(&self, other: &Self) -> bool {
        self.bencode == other.bencode
    }
}

impl Eq for Bencode {}

impl From<&str> for Bencode {
    fn from(value: &str) -> Self {
        match Self::from_string(value) {
            Ok(bencode) => bencode,
            Err(e) => panic!("cannot construct a bencode value due to: {e}"),
        }
    }
}

impl From<String> for Bencode {
    fn from(value: String) -> Self {
        match Self::from_string(value) {
            Ok(bencode) => bencode,
            Err(e) => panic!("cannot construct a bencode value due to: {e}"),
        }
    }
}

impl From<&[u8]> for Bencode {
    fn from(value: &[u8]) -> Self {
        match Self::from_bytes(value) {
            Ok(bencode) => bencode,
            Err(e) => panic!("cannot construct a bencode value due to: {e}"),
        }
    }
}

impl<const N: usize> From<&[u8; N]> for Bencode {
    fn from(value: &[u8; N]) -> Self {
        match Self::from_bytes(value) {
            Ok(bencode) => bencode,
            Err(e) => panic!("cannot construct a bencode value due to: {e}"),
        }
    }
}

impl<const N: usize> From<[u8; N]> for Bencode {
    fn from(value: [u8; N]) -> Self {
        match Self::from_bytes(&value) {
            Ok(bencode) => bencode,
            Err(e) => panic!("cannot construct a bencode value due to: {e}"),
        }
    }
}

impl From<Vec<u8>> for Bencode {
    fn from(value: Vec<u8>) -> Self {
        match Self::from_bytes(&value) {
            Ok(bencode) => bencode,
            Err(e) => panic!("cannot construct a bencode value due to: {e}"),
        }
    }
}

impl From<Bytes> for Bencode {
    fn from(value: Bytes) -> Self {
        match Self::from_bytes(&value) {
            Ok(bencode) => bencode,
            Err(e) => panic!("cannot construct a bencode value due to: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_bencode() {
        let bencode = Bencode::from_string("4:spam").unwrap();
        assert_eq!(bencode.bencode, Value::String("spam".to_string()));
        assert_eq!(bencode.str_remainder, "".to_string());
        assert_eq!(bencode.to_bencode_string(), "4:spam".to_string());
    }

    #[test]
    fn test_integer_bencode() {
        let bencode = Bencode::from_string("i3e").unwrap();
        assert_eq!(bencode.bencode, Value::Integer(3));
        assert_eq!(bencode.str_remainder, "".to_string());
        assert_eq!(bencode.to_bencode_string(), "i3e".to_string());
    }

    #[test]
    fn test_list_bencode() {
        let bencode = Bencode::from_string("l4:spam4:eggsi3e5:braine").unwrap();
        assert_eq!(
            bencode.bencode,
            Value::List(vec![
                Value::String("spam".to_string()),
                Value::String("eggs".to_string()),
                Value::Integer(3),
                Value::String("brain".to_string())
            ])
        );
        assert_eq!(bencode.str_remainder, "".to_string());
        assert_eq!(
            bencode.to_bencode_string(),
            "l4:spam4:eggsi3e5:braine".to_string()
        );
    }

    #[test]
    fn test_dictionary_bencode() {
        let bencode = Bencode::from_string("d3:cow3:moo4:spam4:eggse").unwrap();
        let mut dictionary = HashMap::new();
        dictionary.insert("cow".to_string(), Value::String("moo".to_string()));
        dictionary.insert("spam".to_string(), Value::String("eggs".to_string()));
        assert_eq!(bencode.bencode, Value::Dictionary(dictionary));
        assert_eq!(bencode.str_remainder, "".to_string());
    }

    #[test]
    fn test_null_bencode() {
        let bencode = Bencode::from_string("n").unwrap();
        assert_eq!(bencode.bencode, Value::Null);
        assert_eq!(bencode.str_remainder, "".to_string());
        assert_eq!(bencode.to_bencode_string(), "n".to_string());
    }

    #[test]
    fn test_invalid_bencode() {
        let bencode = Bencode::from_string("x");
        assert!(bencode.is_err());
    }

    #[test]
    fn test_to_json() {
        let bencode = Bencode::from_string("d3:cow3:moo4:spam4:eggse").unwrap();
        let json_value = bencode.to_json().unwrap();
        let expected_json: serde_json::Value = serde_json::json!({
            "cow": "moo",
            "spam": "eggs"
        });
        assert_eq!(json_value, expected_json);
    }

    #[test]
    fn test_empty_string_bencode() {
        let bencode = Bencode::from_string("0:").unwrap();
        assert_eq!(bencode.bencode, Value::String("".to_string()));
        assert_eq!(bencode.str_remainder, "".to_string());
        assert_eq!(bencode.to_bencode_string(), "0:".to_string());
    }

    #[test]
    fn test_large_integer_bencode() {
        let bencode = Bencode::from_string("i9223372036854775807e").unwrap();
        assert_eq!(bencode.bencode, Value::Integer(9223372036854775807));
        assert_eq!(bencode.str_remainder, "".to_string());
        assert_eq!(
            bencode.to_bencode_string(),
            "i9223372036854775807e".to_string()
        );
    }

    #[test]
    fn test_negative_integer_bencode() {
        let bencode = Bencode::from_string("i-42e").unwrap();
        assert_eq!(bencode.bencode, Value::Integer(-42));
        assert_eq!(bencode.str_remainder, "".to_string());
        assert_eq!(bencode.to_bencode_string(), "i-42e".to_string());
    }

    #[test]
    fn test_empty_list_bencode() {
        let bencode = Bencode::from_string("le").unwrap();
        assert_eq!(bencode.bencode, Value::List(vec![]));
        assert_eq!(bencode.str_remainder, "".to_string());
        assert_eq!(bencode.to_bencode_string(), "le".to_string());
    }

    #[test]
    fn test_empty_dictionary_bencode() {
        let bencode = Bencode::from_string("de").unwrap();
        assert_eq!(bencode.bencode, Value::Dictionary(HashMap::new()));
        assert_eq!(bencode.str_remainder, "".to_string());
        assert_eq!(bencode.to_bencode_string(), "de".to_string());
    }

    #[test]
    #[should_panic]
    fn test_invalid_integer_bencode() {
        let _ = Bencode::from_string("i4re");
    }

    #[test]
    fn test_invalid_string_bencode() {
        let bencode = Bencode::from_string("4:abc");
        assert!(bencode.is_err());
    }

    #[test]
    fn test_from_bytes_strings() {
        let test_cases = vec![("4:spam", "spam"), ("5:hello", "hello"), ("0:", "")];

        for (input, expected) in test_cases {
            let result = Bencode::from_bytes(input.as_bytes()).unwrap();
            match result.bencode {
                Value::String(s) => assert_eq!(s, expected),
                _ => panic!("Expected a string, but got something else"),
            }
        }
    }

    #[test]
    fn test_from_bytes_integers() {
        let test_cases = vec![("i42e", 42), ("i0e", 0), ("i-123e", -123)];

        for (input, expected) in test_cases {
            let result = Bencode::from_bytes(input.as_bytes()).unwrap();
            match result.bencode {
                Value::Integer(i) => assert_eq!(i, expected),
                _ => panic!("Expected an integer, but got something else"),
            }
        }
    }

    #[test]
    fn test_from_bytes_lists() {
        let test_cases = vec![
            (
                "l4:spam4:eggse",
                vec![
                    Value::String("spam".to_string()),
                    Value::String("eggs".to_string()),
                ],
            ),
            ("li42ei-1ee", vec![Value::Integer(42), Value::Integer(-1)]),
            ("le", vec![]),
        ];

        for (input, expected) in test_cases {
            let result = Bencode::from_bytes(input.as_bytes()).unwrap();
            match result.bencode {
                Value::List(list) => assert_eq!(list, expected),
                _ => panic!("Expected a list, but got something else"),
            }
        }
    }

    #[test]
    fn test_from_bytes_dictionaries() {
        let mut expected = HashMap::new();
        expected.insert("spam".to_string(), Value::String("eggs".to_string()));
        expected.insert("cow".to_string(), Value::Integer(42));

        let result = Bencode::from_bytes("d4:spam4:eggs3:cowi42ee".as_bytes()).unwrap();
        match result.bencode {
            Value::Dictionary(dict) => {
                assert_eq!(dict.len(), expected.len());
                for (key, value) in dict {
                    assert_eq!(&value, expected.get(&key).unwrap());
                }
            }
            _ => panic!("Expected a dictionary, but got something else"),
        }
    }

    #[test]
    fn test_get_value_mut() {
        let mut bencode = Bencode::from("l3:fooe");
        if let Value::List(list) = bencode.get_value_mut() {
            list.push(Value::String("bar".to_string()));
        }

        assert_eq!(
            bencode.get_value(),
            &Value::List(vec![
                Value::String("foo".to_string()),
                Value::String("bar".to_string())
            ])
        );
    }

    #[test]
    fn test_to_yaml() {
        let bencode = Bencode::from_string("d3:key3:vale").unwrap();
        let yaml_value = bencode.to_yaml_value().unwrap();
        assert_eq!(
            yaml_value,
            serde_yaml::Value::Mapping(
                vec![(
                    serde_yaml::Value::String("key".to_string()),
                    serde_yaml::Value::String("val".to_string())
                )]
                .into_iter()
                .collect()
            )
        );
    }
}
