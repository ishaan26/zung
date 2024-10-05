use serde::Deserialize;
use serde::{
    de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor},
    forward_to_deserialize_any,
};

use super::error::{Error, Result};

pub struct BencodeDeserializer<'de> {
    // This string starts with the input data and characters are truncated off
    // the beginning as data is parsed.
    input: &'de [u8],
}

impl<'de> BencodeDeserializer<'de> {
    // By convention, `Deserializer` constructors are named like `from_xyz`.
    // That way basic use cases are satisfied by something like
    // `serde_json::from_str(...)` while advanced use cases that require a
    // deserializer can make one with `serde_json::Deserializer::from_str(...)`.
    pub fn from_bytes(input: &'de [u8]) -> Self {
        BencodeDeserializer { input }
    }
}

// By convention, the public API of a Serde deserializer is one or more
// `from_xyz` methods such as `from_str`, `from_bytes`, or `from_reader`
// depending on what Rust types the deserializer is able to consume as input.
//
// This basic deserializer supports only `from_str`.
pub(crate) fn from_bytes<'a, T>(s: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = BencodeDeserializer::from_bytes(s);
    let t = T::deserialize(&mut deserializer)?;
    Ok(t)

    // NOTE: Trailing characters are giving an error in case of parsing bytes... WHY IS
    // THAT?????
    // if deserializer.input.is_empty() {
    //     Ok(t)
    // } else {
    //     Err(Error::Custom(format!("Trailing Characters: {:?} found", s)))
    // }
}

#[derive(Debug)]
enum ParseResult {
    Integer(i64),
    Bytes(Vec<u8>),
    List,
    Map,
    End,
}

// SERDE IS NOT A PARSING LIBRARY. This impl block defines a few basic parsing
// functions from scratch. More complicated formats may wish to use a dedicated
// parsing library to help implement their Serde deserializer.
impl<'de> BencodeDeserializer<'de> {
    // Look at the first character in the input without consuming it.
    fn peek_byte(&mut self) -> Result<u8> {
        if self.input.is_empty() {
            return Err(Error::Custom(
                "You are probably missing an end Character".to_string(),
            ));
        }
        Ok(self.input[0])
    }

    // Consume the first character in the input.
    fn next_byte(&mut self) -> Result<u8> {
        let b = self.peek_byte()?;
        self.input = &self.input[1..];
        Ok(b)
    }

    fn parse_integer(&mut self) -> Result<i64> {
        if self.next_byte()? == b'i' {
            if let Some(end_pos) = self.input.iter().position(|e| *e == b'e') {
                let integer = &self.input[..end_pos];
                self.input = &self.input[end_pos + 1..];
                let integer = String::from_utf8(integer.to_vec())
                    .map_err(|_| Error::InvalidValue(format!("{integer:?} is not valid utf8")))?;
                let integer = integer.parse::<isize>().map_err(|_| {
                    Error::InvalidValue(format!("Can't parse `{integer}` as integer"))
                })?;
                Ok(integer as i64)
            } else {
                Err(Error::EndOfStream)
            }
        } else {
            Err(Error::InvalidType("It ain't no integer bro".to_string()))
        }
    }

    // Parse the JSON identifier `true` or `false`.
    fn parse_bool(&mut self) -> Result<bool> {
        let int = self.parse_integer()?;
        if int == 1 {
            return Ok(true);
        } else if int == 0 {
            return Ok(false);
        }

        Err(Error::InvalidType(format!(
            "In bencode, only 0 and 1 can be boolean. So using integer {} is stupid",
            int
        )))
    }

    fn parse_bytes(&mut self) -> Result<&'de [u8]> {
        match self.peek_byte()? {
            b'0'..=b'9' => {
                let (len, rest) = match self.input.iter().position(|pos| *pos == b':') {
                    Some(pos) => (&self.input[..pos], &self.input[pos + 1..]),
                    None => {
                        return Err(Error::MissingField(
                            "Invalid string bencode format: missing ':'".to_string(),
                        ));
                    }
                };

                let len = std::str::from_utf8(len)
                    .map_err(|_| Error::InvalidValue(format!("{len:?} is not valid utf8")))?;

                let len = len
                    .parse::<usize>()
                    .map_err(|_| Error::Custom("Cannot parse the length".to_string()))?;

                let string = &rest[..len];
                self.input = &rest[len..];

                Ok(string)
            }

            t => Err(Error::InvalidValue(format!(
                "Received non utf8 bytes. {t} ain't no bytes bro",
            ))),
        }
    }

    // Parse a string until the next '"' character.
    //
    // Makes no attempt to handle escape sequences. What did you expect? This is
    // example code!
    fn parse_string(&mut self) -> Result<String> {
        let string = self.parse_bytes()?;
        match String::from_utf8(string.to_vec()) {
            Ok(string) => Ok(string),
            Err(_) => Err(Error::InvalidValue(format!("{string:?} is not valid utf8"))),
        }
    }

    fn parse_result(&mut self) -> Result<ParseResult> {
        match self.peek_byte()? {
            b'0'..=b'9' => Ok(ParseResult::Bytes(self.parse_bytes()?.to_vec())),
            b'i' => Ok(ParseResult::Integer(self.parse_integer()?)),
            b'l' => Ok(ParseResult::List),
            b'd' => Ok(ParseResult::Map),
            b'e' => Ok(ParseResult::End),
            o => {
                let o = String::from_utf8(Vec::from([o])).map_err(|_| {
                    Error::Custom(format!(
                        "{o} is not valid utf-8. So a bencode tag cannot be constructed from it"
                    ))
                })?;
                Err(Error::UnknownField(format!(
                    "{o} is not valid bencode syntax"
                )))
            }
        }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut BencodeDeserializer<'de> {
    type Error = Error;

    // Look at the input data to decide what Serde data model type to
    // deserialize as. Not all data formats are able to support this operation.
    // Formats that support `deserialize_any` are known as self-describing.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.parse_result()? {
            ParseResult::Integer(i) => visitor.visit_i64(i),
            ParseResult::Bytes(b) => visitor.visit_bytes(&b),
            ParseResult::List => self.deserialize_seq(visitor),
            ParseResult::Map => self.deserialize_map(visitor),
            ParseResult::End => Err(Error::EndOfStream),
        }
    }

    forward_to_deserialize_any! {
        f32 f64 unit unit_struct
        tuple_struct ignored_any struct
    }

    // Uses the `parse_bool` parsing function defined above to read the JSON
    // identifier `true` or `false` from the input.
    //
    // Parsing refers to looking at the input and deciding that it contains the
    // JSON value `true` or `false`.
    //
    // Deserialization refers to mapping that JSON value into Serde's data
    // model by invoking one of the `Visitor` methods. In the case of JSON and
    // bool that mapping is straightforward so the distinction may seem silly,
    // but in other cases Deserializers sometimes perform non-obvious mappings.
    // For example the TOML format has a Datetime type and Serde's data model
    // does not. In the `toml` crate, a Datetime in the input is deserialized by
    // mapping it to a Serde data model "struct" type with a special name and a
    // single field containing the Datetime represented as a string.
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    // The `parse_signed` function is generic over the integer type `T` so here
    // it is invoked with `T=i8`. The next 8 methods are similar.
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
        visitor.visit_i64(self.parse_integer()?)
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

    // The `Serializer` implementation on the previous page serialized chars as
    // single-character strings so handle that representation here.
    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Parse a string, check that it is one character, call `visit_char`.
        unimplemented!()
    }

    // Refer to the "Understanding deserializer lifetimes" page for information
    // about the three deserialization flavors of strings in Serde.
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.parse_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    // The `Serializer` implementation on the previous page serialized byte
    // arrays as JSON arrays of bytes. Handle that representation here.
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bytes(self.parse_bytes()?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_byte_buf(self.parse_bytes()?.to_vec())
    }

    // An absent optional is represented as the JSON `null` and a present
    // optional is represented as just the contained value.
    //
    // As commented in `Serializer` implementation, this is a lossy
    // representation. For example the values `Some(())` and `None` both
    // serialize as just `null`. Unfortunately this is typically what people
    // expect when working with JSON. Other formats are encouraged to behave
    // more intelligently if possible.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain. That means not
    // parsing anything other than the contained value.
    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    // Deserialization of compound types like sequences and maps happens by
    // passing the visitor an "Access" object that gives it the ability to
    // iterate through the data contained in the sequence.
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

    // Tuples look just like sequences in JSON. Some formats may be able to
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

    // An identifier in Serde is the type that identifies a field of a struct or
    // the variant of an enum. In JSON, struct fields and enum variants are
    // represented as strings. In other formats they may be represented as
    // numeric indices.
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }
}

// In order to handle commas correctly when deserializing a JSON array or map,
// we need to track whether we are on the first element or past the first
// element.
struct BencodeAccess<'a, 'de: 'a> {
    de: &'a mut BencodeDeserializer<'de>,
}

impl<'a, 'de> BencodeAccess<'a, 'de> {
    fn new(de: &'a mut BencodeDeserializer<'de>) -> Self {
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
        if self.de.peek_byte()? == b'e' {
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
        if self.de.peek_byte()? == b'e' {
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
        match self.de.parse_result()? {
            ParseResult::Bytes(_) | ParseResult::Map => {
                Ok((seed.deserialize(&mut *self.de)?, self))
            }
            t => Err(Error::InvalidValue(format!(
                "Expected a string or a map. Got {t:?}"
            ))),
        }
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

    // Newtype variants are represented in JSON as `{ NAME: VALUE }` so
    // deserialize the value here.
    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    // Tuple variants are represented in JSON as `{ NAME: [DATA...] }` so
    // deserialize the sequence of data here.
    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }` so
    // deserialize the inner map here.
    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_map(self.de, visitor)
    }
}
