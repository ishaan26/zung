use serde::de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};
use serde::Deserialize;

use super::error::{Error, Result};
use super::Bencode;

pub struct Deserializer<'de> {
    bencode: Bencode<'de>,
}

// By convention, `Deserializer` constructors are named like `from_xyz`.
// That way basic use cases are satisfied by something like
// `serde_json::from_str(...)` while advanced use cases that require a
// deserializer can make one with `serde_json::Deserializer::from_str(...)`.
impl<'de> Deserializer<'de> {
    pub fn from_str(input: &'de str) -> Self {
        Deserializer {
            bencode: Bencode::from_str(input),
        }
    }

    pub fn from_bytes(input: &'de [u8]) -> Self {
        Deserializer {
            bencode: Bencode::from_bytes(input),
        }
    }
}

// By convention, the public API of a Serde deserializer is one or more
// `from_xyz` methods such as `from_str`, `from_bytes`, or `from_reader`
// depending on what Rust types the deserializer is able to consume as input.

/// Deserializes a bencode-encoded string into a value of type `T`.
///
/// This function takes a string slice containing bencode-encoded data and attempts to
/// deserialize it into a value of type `T`, which must implement the `Deserialize` trait.
///
/// # Type Parameters
///
/// * `T` - The type to deserialize into. This type must implement the `Deserialize` trait.
///
///
/// # Examples
///
/// ```rust
/// use zung_parsers::bencode;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct Person {
///     name: String,
///     age: i32,
/// }
///
/// let bencode_str = "d4:name5:Alice3:agei30ee";
/// let person: Person = bencode::from_str(bencode_str).unwrap();
/// assert_eq!(person, Person { name: "Alice".to_string(), age: 30 });
/// ```
///
/// # Errors
///
/// This function will return an error if:
/// * The input string is not valid bencode.
/// * The bencode structure doesn't match the structure of type `T`.
/// * Any other deserialization error occurs.
pub fn from_str<'a, T>(string: &'a str) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_str(string);
    let t = T::deserialize(&mut deserializer)?;
    Ok(t)
}

/// Deserializes bencode-encoded bytes into a value of type `T`.
///
/// This function takes a byte slice containing bencode-encoded data and attempts to deserialize it
/// into a value of type `T`, which must implement the `Deserialize` trait.
///
/// # Type Parameters
///
/// * `T` - The type to deserialize into. This type must implement the `Deserialize` trait.
///
/// # Examples
///
/// ```rust
/// use zung_parsers::bencode;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct Person {
///     name: String,
///     age: i32,
/// }
///
/// let bencode_bytes = b"d4:name5:Alice3:agei30ee";
/// let person: Person = bencode::from_bytes(bencode_bytes).unwrap();
/// assert_eq!(person, Person { name: "Alice".to_string(), age: 30 });
/// ```
///
/// # Errors
///
/// This function will return an error if:
/// * The input bytes are not valid bencode.
/// * The bencode structure doesn't match the structure of type `T`.
/// * Any other deserialization error occurs.
pub fn from_bytes<'a, T>(bytes: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_bytes(bytes);
    let t = T::deserialize(&mut deserializer)?;
    Ok(t)
}

impl<'de> Deserializer<'de> {
    // Look at the first character in the input without consuming it.
    fn peek_byte(&mut self) -> Result<u8> {
        if self.bencode.input.is_empty() {
            return Err(Error::Custom(
                "You are probably missing an end Character".to_string(),
            ));
        }
        Ok(self.bencode.input[0])
    }

    // Consume the first character in the input.
    fn next_byte(&mut self) -> Result<u8> {
        let b = self.peek_byte()?;
        self.bencode.input = &self.bencode.input[1..];
        Ok(b)
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    // Look at the input data to decide what Serde data model type to
    // deserialize as. Not all data formats are able to support this operation.
    // Formats that support `deserialize_any` are known as self-describing.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek_byte()? {
            b'0'..=b'9' => self.deserialize_bytes(visitor),
            b'i' => self.deserialize_i64(visitor),
            b'l' => self.deserialize_seq(visitor),
            b'd' => self.deserialize_map(visitor),
            _ => Err(Error::InvalidType("This is not valid bencode".to_string())),
        }
    }

    fn deserialize_bool<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.peek_byte()? != b'i' {
            return Err(Error::InvalidType("Expected String length".to_string()));
        }

        visitor.visit_i64(self.bencode.parse_integer()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    // Float parsing is stupidly hard.
    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    // Float parsing is stupidly hard.
    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek_byte()? {
            b'0'..=b'9' => {
                let parsed = self.bencode.parse_bytes()?;
                let parsed = String::from_utf8(parsed).map_err(|e| {
                    Error::Custom(format!("Error while deserializeing string data : {e}"))
                })?;
                visitor.visit_string(parsed)
            }
            e => Err(Error::InvalidType(format!(
                "Expected String length, found '{}'",
                std::str::from_utf8(&[e]).expect("Invalid utf8 character in string len")
            ))),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bytes(&self.bencode.parse_bytes()?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_byte_buf(self.bencode.parse_bytes()?)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    // Unit struct means a named value containing no data.
    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.next_byte()? == b'l' {
            let value = visitor.visit_seq(BencodeAccess::new(self))?;
            if self.next_byte()? == b'e' {
                Ok(value)
            } else {
                Err(Error::InvalidType("Expected Array".to_string()))
            }
        } else {
            Err(Error::InvalidType("Expected Array".to_string()))
        }
    }

    // Tuples look just like sequences in Bencode. Some formats may be able to
    // represent tuples more efficiently.
    //
    // As indicated by the length parameter, the `Deserialize` implementation
    // for a tuple in the Serde data model is required to know the length of the
    // tuple before even looking at the input data.
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Tuple structs look just like sequences in Bencode.
    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Much like `deserialize_seq` but calls the visitors `visit_map` method
    // with a `MapAccess` implementation, rather than the visitor's `visit_seq`
    // method with a `SeqAccess` implementation.
    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.next_byte()? == b'd' {
            let value = visitor.visit_map(BencodeAccess::new(self))?;
            if self.next_byte()? == b'e' {
                Ok(value)
            } else {
                Err(Error::InvalidType("Expected Map".to_string()))
            }
        } else {
            Err(Error::InvalidType("Expected Map".to_string()))
        }
    }

    // Structs look just like maps in Bencode.
    //
    // Notice the `fields` parameter - a "struct" in the Serde data model means
    // that the `Deserialize` implementation is required to know what the fields
    // are before even looking at the input data. Any key-value pairing in which
    // the fields cannot be known ahead of time is probably a map.
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(BencodeAccess::new(self))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    // Like `deserialize_any` but indicates to the `Deserializer` that it makes
    // no difference which `Visitor` method is called because the data is
    // ignored.
    //
    // Some deserializers are able to implement this more efficiently than
    // `deserialize_any`, for example by rapidly skipping over matched
    // delimiters without paying close attention to the data in between.
    //
    // Some formats are not able to implement this at all. Formats that can
    // implement `deserialize_any` and `deserialize_ignored_any` are known as
    // self-describing.
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

struct BencodeAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> BencodeAccess<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        BencodeAccess { de }
    }
}

// `SeqAccess` is provided to the `Visitor` to give it the ability to iterate
// through elements of the sequence.
impl<'de, 'a> SeqAccess<'de> for BencodeAccess<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.de.bencode.input[0] == b'e' {
            return Ok(None);
        }

        seed.deserialize(&mut *self.de).map(Some)
    }
}

// `MapAccess` is provided to the `Visitor` to give it the ability to iterate
// through entries of the map.
impl<'de, 'a> MapAccess<'de> for BencodeAccess<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        if self.de.bencode.input[0] == b'e' {
            return Ok(None);
        }

        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de)
    }
}

// `EnumAccess` is provided to the `Visitor` to give it the ability to determine
// which variant of the enum is supposed to be deserialized.
//
// Note that all enum deserialization methods in Serde refer exclusively to the
// "externally tagged" enum representation.
impl<'de, 'a> EnumAccess<'de> for BencodeAccess<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        Ok((seed.deserialize(&mut *self.de)?, self))
    }
}

// `VariantAccess` is provided to the `Visitor` to give it the ability to see
// the content of the single variant that it decided to deserialize.
impl<'de, 'a> VariantAccess<'de> for BencodeAccess<'a, 'de> {
    type Error = Error;

    // If the `Visitor` expected this variant to be a unit variant, the input
    // should have been the plain string case handled in `deserialize_enum`.
    fn unit_variant(self) -> Result<()> {
        Err(Error::EndOfStream)
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_map(self.de, visitor)
    }
}

/////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Deserialize)]
    struct TestStruct {
        cow: String,
        spam: String,
    }

    #[test]
    fn test_deserialize_integer() {
        let input = "i42e"; // Bencode for integer 42
        let result: i64 = from_str(input).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_deserialize_string() {
        let input = "4:spam"; // Bencode for string "spam"
        let result: String = from_str(input).unwrap();
        assert_eq!(result, "spam");
    }

    #[test]
    fn test_deserialize_list() {
        let input = "l4:spam4:eggse"; // Bencode for list ["spam", "eggs"]
        let result: Vec<String> = from_str(input).unwrap();
        assert_eq!(result, vec!["spam", "eggs"]);
    }

    #[test]
    fn test_deserialize_dict() {
        let input = "d3:cow3:moo4:spam4:eggse"; // Bencode for {"cow": "moo", "spam": "eggs"}
        let result: TestStruct = from_str(input).unwrap();
        assert_eq!(
            result,
            TestStruct {
                cow: "moo".to_string(),
                spam: "eggs".to_string(),
            }
        );
    }

    #[test]
    fn test_deserialize_invalid_input() {
        let input = "x42e"; // Invalid Bencode
        let result: Result<i64> = from_str(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_nested_list() {
        // Bencode for [["spam", "eggs"], ["ham", "bacon"]]
        let input = "ll4:spam4:eggsel3:ham5:baconee";
        let result: Vec<Vec<String>> = from_str(input).unwrap();
        assert_eq!(
            result,
            vec![
                vec!["spam".to_string(), "eggs".to_string()],
                vec!["ham".to_string(), "bacon".to_string()],
            ]
        );
    }

    #[test]
    fn test_deserialize_nested_dict() {
        let input = "d3:cowd3:moo4:oinkee"; // Bencode for {"cow": {"moo": "oink"}}
        let result: std::collections::HashMap<String, std::collections::HashMap<String, String>> =
            from_str(input).unwrap();
        let mut inner_map = std::collections::HashMap::new();
        inner_map.insert("moo".to_string(), "oink".to_string());
        let mut expected_map = std::collections::HashMap::new();
        expected_map.insert("cow".to_string(), inner_map);

        assert_eq!(result, expected_map);
    }
}
