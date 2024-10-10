use serde::{
    ser::{SerializeMap, SerializeSeq},
    Deserialize, Serialize, Serializer,
};

use std::{
    collections::HashMap,
    fmt::{self},
};

/// Representation of Bencode values in Rust.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// Represents an integer value in Bencode. Bencoded integers are prefixed with `i` and
    /// suffixed with `e` (e.g., `i42e` for the integer 42).
    Integer(i64),

    /// Represents a byte string in Bencode, which is typically binary data (e.g., `4:spam` for the
    /// byte string `b"spam"`).
    Bytes(Vec<u8>),

    /// Represents a UTF-8 encoded string in Bencode. While Bencode traditionally uses byte
    /// strings, this variant allows you to handle UTF-8 text directly.
    String(String),

    /// Represents a list in Bencode. Lists are prefixed and suffixed by `l` and `e`, respectively,
    /// and contain a sequence of other Bencode values (e.g., `l4:spam4:eggse`).
    List(Vec<Value>),

    ///  Represents a dictionary (or map) in Bencode. Dictionaries are key-value pairs where the
    ///  keys are strings, and values are other Bencode values. Dictionaries are prefixed and
    ///  suffixed with `d` and `e`, respectively (e.g., `d3:cow3:mooe` for a dictionary with one
    ///  key-value pair).
    Dictionary(HashMap<String, Value>),
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Integer(i) => serializer.serialize_i64(*i),
            Value::Bytes(b) => {
                // Convert bytes to hexadecimal string
                let hex_string = hex::encode(b);
                serializer.serialize_str(&hex_string)
            }
            Value::String(s) => serializer.serialize_str(s),
            Value::List(l) => {
                let mut seq = serializer.serialize_seq(Some(l.len()))?;
                for item in l {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Value::Dictionary(d) => {
                let mut map = serializer.serialize_map(Some(d.len()))?;
                for (k, v) in d {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

pub enum ValueInput<'a> {
    Str(&'a str),
    Bytes(&'a [u8]),
}

impl<'a> From<&'a str> for ValueInput<'a> {
    fn from(s: &'a str) -> Self {
        ValueInput::Str(s)
    }
}

impl<'a> From<&'a String> for ValueInput<'a> {
    fn from(s: &'a String) -> Self {
        ValueInput::Str(s)
    }
}

impl<'a> From<&'a [u8]> for ValueInput<'a> {
    fn from(b: &'a [u8]) -> Self {
        ValueInput::Bytes(b)
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for ValueInput<'a> {
    fn from(b: &'a [u8; N]) -> Self {
        ValueInput::Bytes(b)
    }
}

impl<'a> From<&'a Vec<u8>> for ValueInput<'a> {
    fn from(b: &'a Vec<u8>) -> Self {
        ValueInput::Bytes(b)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(i) => write!(f, "{}", i),
            Value::Bytes(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => write!(f, "{}", escape_string(s)),
                Err(_) => write!(f, "{}", hex::encode(bytes)),
            },
            Value::String(s) => write!(f, "{}", escape_string(s)),
            Value::List(list) => {
                f.write_str("[")?;
                for (i, value) in list.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    fmt::Display::fmt(value, f)?;
                }
                f.write_str("]")
            }
            Value::Dictionary(dictionary) => {
                f.write_str("{")?;
                for (i, (key, value)) in dictionary.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{}: ", escape_string(key))?;
                    fmt::Display::fmt(value, f)?;
                }
                f.write_str("}")
            }
        }
    }
}

fn escape_string(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => "\\\"".chars().collect(),
            '\\' => "\\\\".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            _ if c.is_control() => format!("\\u{:04x}", c as u32).chars().collect(),
            _ => vec![c],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_integer() {
        let value = Value::Integer(42);
        assert_eq!(value.to_string(), "42");
    }

    #[test]
    fn test_value_bytes_utf8() {
        let value = Value::Bytes(vec![72, 101, 108, 108, 111]); // 'Hello'
        assert_eq!(value.to_string(), "Hello");
    }

    #[test]
    fn test_value_bytes_non_utf8() {
        let value = Value::Bytes(vec![0, 159, 146, 150]); // Non-UTF8 bytes
        assert_eq!(value.to_string(), "009f9296");
    }

    #[test]
    fn test_value_string() {
        let value = Value::String("Test".to_string());
        assert_eq!(value.to_string(), "Test");
    }

    #[test]
    fn test_value_list() {
        let list = vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::String("three".to_string()),
        ];
        let value = Value::List(list);
        assert_eq!(value.to_string(), "[1, 2, three]");
    }

    #[test]
    fn test_value_dictionary() {
        let mut dict = HashMap::new();
        dict.insert("key1".to_string(), Value::Integer(10));
        dict.insert("key2".to_string(), Value::String("value".to_string()));
        let value = Value::Dictionary(dict);

        let result = value.to_string();
        dbg!(&result);
        assert!(result.contains("key1: 10"));
        assert!(result.contains("key2: value"));
    }

    #[test]
    fn test_valueinput_str() {
        let input: ValueInput = "test".into();
        if let ValueInput::Str(s) = input {
            assert_eq!(s, "test");
        } else {
            panic!("Expected ValueInput::Str");
        }
    }

    #[test]
    fn test_valueinput_array() {
        let input: ValueInput = (&[1, 2, 3]).into();
        if let ValueInput::Bytes(bytes) = input {
            assert_eq!(bytes, &[1, 2, 3]);
        } else {
            panic!("Expected ValueInput::Bytes");
        }
    }
}
