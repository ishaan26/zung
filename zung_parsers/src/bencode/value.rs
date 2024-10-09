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
