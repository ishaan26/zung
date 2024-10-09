use serde::{Deserialize, Serialize};

use std::{collections::HashMap, fmt::Display};

/// Reprasents Bencode values as a rust enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Integer(i64),
    Bytes(Vec<u8>),
    String(String),
    List(Vec<Value>),
    Dictionary(HashMap<String, Value>),
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
    fn test_value_integer() {
        let value = Value::Integer(42);
        assert_eq!(value.to_string(), "42");
    }

    #[test]
    fn test_value_null() {
        let value = Value::Null;
        assert_eq!(value.to_string(), "Null");
    }

    #[test]
    fn test_value_bytes_utf8() {
        let value = Value::Bytes(vec![72, 101, 108, 108, 111]); // 'Hello'
        assert_eq!(value.to_string(), "Hello");
    }

    #[test]
    fn test_value_bytes_non_utf8() {
        let value = Value::Bytes(vec![0, 159, 146, 150]); // Non-UTF8 bytes
        assert_eq!(value.to_string(), "/*BYTES*/");
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
        assert!(result.contains("key1 : 10"));
        assert!(result.contains("key2 : value"));
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
