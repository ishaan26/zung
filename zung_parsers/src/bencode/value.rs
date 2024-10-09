use serde::{Deserialize, Serialize};

use std::collections::HashMap;

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
