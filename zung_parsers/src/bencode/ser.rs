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

use serde::{ser, Serialize};

use super::{
    error::{Error, Result},
    Value,
};

#[derive(Default)]
pub struct Serializer {
    buffer: Vec<u8>,
}

impl Serializer {
    pub fn new() -> Serializer {
        Self::default()
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.buffer
    }

    fn push<T>(&mut self, value: T)
    where
        T: AsRef<[u8]>,
    {
        self.buffer.extend_from_slice(value.as_ref())
    }
}

impl AsRef<[u8]> for Serializer {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

// By convention, the public API of a Serde serializer is one or more `to_abc`
// functions such as `to_string`, `to_bytes`, or `to_writer` depending on what
// Rust types the serializer is able to produce as output.

/// Convert a type `T` into a vector of bencode bytes.
///
/// This function takes a reference to a value of any type that implements the `Serialize` trait
/// and converts it into a vector of bytes using a custom `Serializer`.
///
/// # Arguments
///
/// * `value` - A reference to the value to be serialized.
///
/// # Example
///
/// ```rust
/// use serde::Serialize;
/// use zung_parsers::bencode;
///
/// #[derive(Serialize)]
/// struct MyStruct {
///     field: i32,
/// }
///
/// let my_struct = MyStruct { field: 42 };
/// let bytes = bencode::to_bytes(&my_struct).unwrap(); // outputs b"i42e"
/// ```
pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let mut serializer = Serializer { buffer: Vec::new() };
    value.serialize(&mut serializer)?;
    Ok(serializer.buffer)
}

/// Convert a type `T` into a bencode UTF-8 [`String`].
///
/// This function takes a reference to a value of any type that implements the `Serialize` trait
/// and converts it into a `String` using a custom `Serializer`.
///
/// # Arguments
///
/// * `value` - A reference to the value to be serialized.
///
/// # Example
///
/// ```rust
/// use serde::Serialize;
/// use zung_parsers::bencode;
///
/// #[derive(Serialize)]
/// struct MyStruct {
///     field: i32,
/// }
///
/// let my_struct = MyStruct { field: 42 };
/// let bytes = bencode::to_string(&my_struct).unwrap(); // outputs "i42e"
/// ```
pub fn to_string<T: ser::Serialize>(b: &T) -> Result<String> {
    let mut ser = Serializer::new();
    b.serialize(&mut ser)?;
    std::str::from_utf8(ser.as_ref())
        .map(std::string::ToString::to_string)
        .map_err(|_| Error::InvalidValue("Not an UTF-8".to_string()))
}

/// Convert a `T` into [`zung_parsers::bencode::Value`](crate::bencode::Value) which is an enum that
/// can represent any valid Bencode data.
///
/// ## Example
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
pub fn to_value<T>(value: T) -> Result<Value>
where
    T: Serialize,
{
    let ser = to_bytes(&value)?;
    super::parse(&ser)
}

impl<'a> ser::Serializer for &'a mut Serializer {
    // The output type produced by this `Serializer` during successful
    // serialization. Most serializers that produce text or binary output should
    // set `Ok = ()` and serialize into an `io::Write` or buffer contained
    // within the `Serializer` instance, as happens here. Serializers that build
    // in-memory data structures may be simplified by using `Ok` to propagate
    // the data structure around.
    type Ok = ();

    // The error type when some error occurs during serialization.
    type Error = Error;

    // Associated types for keeping track of additional state while serializing
    // compound data structures like sequences and maps. In this case no
    // additional state is required beyond what is already stored in the
    // Serializer struct.
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = SerializeMap<'a>;
    type SerializeStruct = SerializeMap<'a>;
    type SerializeStructVariant = SerializeMap<'a>;

    // Here we go with the simple methods. The following 12 methods receive one
    // of the primitive types of the data model and map it to JSON by appending
    // into the output string.
    fn serialize_bool(self, v: bool) -> Result<()> {
        self.push(if v { "i1e" } else { "i0e" });
        Ok(())
    }

    // JSON does not distinguish between different sizes of integers, so all
    // signed integers will be serialized the same and all unsigned integers
    // will be serialized the same. Other formats, especially compact binary
    // formats, may need independent logic for the different sizes.
    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    // Not particularly efficient but this is example code anyway. A more
    // performant approach would be to use the `itoa` crate.
    fn serialize_i64(self, v: i64) -> Result<()> {
        self.push(format!("i{v}e"));
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.push(format!("i{v}e"));
        Ok(())
    }

    fn serialize_f32(self, _v: f32) -> Result<()> {
        unimplemented!()
    }

    fn serialize_f64(self, _v: f64) -> Result<()> {
        unimplemented!()
    }

    // Serialize a char as a single-character string. Other formats may
    // represent this differently.
    fn serialize_char(self, v: char) -> Result<()> {
        self.push(format!("1:{v}"));
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.push(format!("{}:{v}", v.len()));
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.push(v.len().to_string());
        self.push(":");
        self.push(v);
        Ok(())
    }

    // An absent optional is represented as the JSON `null`.
    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    // A present optional is represented as just the contained value. Note that
    // this is a lossy representation. For example the values `Some(())` and
    // `None` both serialize as just `null`. Unfortunately this is typically
    // what people expect when working with JSON. Other formats are encouraged
    // to behave more intelligently if possible.
    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    // In Serde, unit means an anonymous value containing no data. Map this to
    // JSON as `null`.
    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    // Unit struct means a named value containing no data. Again, since there is
    // no data, map this to JSON as `null`. There is no need to serialize the
    // name in most formats.
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    // When serializing a unit variant (or any other kind of variant), formats
    // can choose whether to keep track of it by index or by name. Binary
    // formats typically use the index of the variant and human-readable formats
    // typically use the name.
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain.
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    // Note that newtype variant (and all of the other variant serialization
    // methods) refer exclusively to the "externally tagged" enum
    // representation.
    //
    // Serialize this to JSON in externally tagged form as `{ NAME: VALUE }`.
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    // Now we get to the serialization of compound types.
    //
    // The start of the sequence, each value, and the end are three separate
    // method calls. This one is responsible only for serializing the start,
    // which in JSON is `[`.
    //
    // The length of the sequence may or may not be known ahead of time. This
    // doesn't make a difference in JSON because the length is not represented
    // explicitly in the serialized form. Some serializers may only be able to
    // support sequences for which the length is known up front.
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.push("l");
        Ok(self)
    }

    // Tuples look just like sequences in JSON. Some formats may be able to
    // represent tuples more efficiently by omitting the length, since tuple
    // means that the corresponding `Deserialize implementation will know the
    // length without needing to look at the serialized data.
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    // Tuple structs look just like sequences in JSON.
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    // Tuple variants are represented in JSON as `{ NAME: [DATA...] }`. Again
    // this method is only responsible for the externally tagged representation.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.push("d");
        self.serialize_bytes(variant.as_bytes())?;
        self.push("l");
        Ok(self)
    }

    // Maps are represented in JSON as `{ K: V, K: V, ... }`.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(SerializeMap::new(self, len.unwrap_or(0)))
    }

    // Structs look just like maps in JSON. In particular, JSON requires that we
    // serialize the field names of the struct. Other formats may be able to
    // omit the field names when serializing structs because the corresponding
    // Deserialize implementation is required to know what the keys are without
    // looking at the serialized data.
    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }`.
    // This is the externally tagged representation.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.serialize_map(Some(len))
    }
}

// The following 7 impls deal with the serialization of compound types like
// sequences and maps. Serialization of such types is begun by a Serializer
// method and followed by zero or more calls to serialize individual elements of
// the compound type and one call to end the compound type.
//
// This impl is SerializeSeq so these methods are called after `serialize_seq`
// is called on the Serializer.
impl ser::SerializeSeq for &mut Serializer {
    // Must match the `Ok` type of the serializer.
    type Ok = ();
    // Must match the `Error` type of the serializer.
    type Error = Error;

    // Serialize a single element of the sequence.
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    // Close the sequence.
    fn end(self) -> Result<()> {
        self.push("e");
        Ok(())
    }
}

impl ser::SerializeTuple for &mut Serializer {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: ?Sized + ser::Serialize>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

// Same thing but for tuple structs.
impl ser::SerializeTupleStruct for &mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleVariant for &mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

// Some `Serialize` types are not able to hold a key and value in memory at the
// same time so `SerializeMap` implementations are required to support
// `serialize_key` and `serialize_value` individually.
//
// There is a third optional method on the `SerializeMap` trait. The
// `serialize_entry` method allows serializers to optimize for the case where
// key and value are both available simultaneously. In JSON it doesn't make a
// difference so the default behavior for `serialize_entry` is fine.
impl ser::SerializeMap for &mut Serializer {
    type Ok = ();
    type Error = Error;

    // The Serde data model allows map keys to be any serializable type. JSON
    // only allows string keys so the implementation below will produce invalid
    // JSON if the key serializes as something other than a string.
    //
    // A real JSON serializer would need to validate that map keys are strings.
    // This can be done by using a different Serializer to serialize the key
    // (instead of `&mut **self`) and having that other serializer only
    // implement `serialize_str` and return an error on any other data type.
    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)
    }

    // It doesn't make a difference whether the colon is printed at the end of
    // `serialize_key` or at the beginning of `serialize_value`. In this case
    // the code is a bit simpler having it here.
    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.push("e");
        Ok(())
    }
}

pub struct SerializeMap<'a> {
    ser: &'a mut Serializer,
    entries: Vec<(Vec<u8>, Vec<u8>)>,
    cur_key: Option<Vec<u8>>,
}

impl SerializeMap<'_> {
    pub fn new(ser: &mut Serializer, len: usize) -> SerializeMap {
        SerializeMap {
            ser,
            entries: Vec::with_capacity(len),
            cur_key: None,
        }
    }

    fn end_map(&mut self) -> Result<()> {
        if self.cur_key.is_some() {
            return Err(Error::InvalidValue(
                "`serialize_key` called without calling  `serialize_value`".to_string(),
            ));
        }
        let mut entries = std::mem::take(&mut self.entries);
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));
        self.ser.push("d");
        for (k, v) in entries {
            ser::Serializer::serialize_bytes(&mut *self.ser, k.as_ref())?;
            self.ser.push(v);
        }
        self.ser.push("e");
        Ok(())
    }
}

impl ser::SerializeMap for SerializeMap<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized + ser::Serialize>(&mut self, key: &T) -> Result<()> {
        if self.cur_key.is_some() {
            return Err(Error::InvalidValue(
                "`serialize_key` called multiple times without calling  `serialize_value`"
                    .to_string(),
            ));
        }
        self.cur_key = Some(key.serialize(&mut string::Serializer)?);
        Ok(())
    }

    fn serialize_value<T: ?Sized + ser::Serialize>(&mut self, value: &T) -> Result<()> {
        let key = self.cur_key.take().ok_or_else(|| {
            Error::InvalidValue(
                "`serialize_value` called without calling `serialize_key`".to_string(),
            )
        })?;
        let mut ser = Serializer::new();
        value.serialize(&mut ser)?;
        let value = ser.into_vec();
        if !value.is_empty() {
            self.entries.push((key, value));
        }
        Ok(())
    }

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<()>
    where
        K: ?Sized + ser::Serialize,
        V: ?Sized + ser::Serialize,
    {
        if self.cur_key.is_some() {
            return Err(Error::InvalidValue(
                "`serialize_key` called multiple times without calling  `serialize_value`"
                    .to_string(),
            ));
        }
        let key = key.serialize(&mut string::Serializer)?;
        let mut ser = Serializer::new();
        value.serialize(&mut ser)?;
        let value = ser.into_vec();
        if !value.is_empty() {
            self.entries.push((key, value));
        }
        Ok(())
    }
    fn end(mut self) -> Result<()> {
        self.end_map()
    }
}

impl ser::SerializeStruct for SerializeMap<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: ?Sized + ser::Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        ser::SerializeMap::serialize_entry(self, key, value)
    }
    fn end(mut self) -> Result<()> {
        self.end_map()
    }
}

impl ser::SerializeStructVariant for SerializeMap<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: ?Sized + ser::Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        ser::SerializeMap::serialize_entry(self, key, value)
    }
    fn end(mut self) -> Result<()> {
        self.end_map()?;
        self.ser.push("e");
        Ok(())
    }
}

mod string {
    //! Serializer for serializing *just* strings.

    use super::super::error::{Error, Result};
    use serde::de;
    use serde::ser;
    use std::fmt;
    use std::str;

    struct Expected;
    impl de::Expected for Expected {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a string or bytes")
        }
    }

    fn unexpected<T>(unexp: de::Unexpected<'_>) -> Result<T> {
        Err(de::Error::invalid_type(unexp, &Expected))
    }

    pub(crate) struct Serializer;

    impl ser::Serializer for &mut Serializer {
        type Ok = Vec<u8>;
        type Error = Error;
        type SerializeSeq = ser::Impossible<Vec<u8>, Error>;
        type SerializeTuple = ser::Impossible<Vec<u8>, Error>;
        type SerializeTupleStruct = ser::Impossible<Vec<u8>, Error>;
        type SerializeTupleVariant = ser::Impossible<Vec<u8>, Error>;
        type SerializeMap = ser::Impossible<Vec<u8>, Error>;
        type SerializeStruct = ser::Impossible<Vec<u8>, Error>;
        type SerializeStructVariant = ser::Impossible<Vec<u8>, Error>;

        fn serialize_bool(self, value: bool) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::Bool(value))
        }
        fn serialize_i8(self, value: i8) -> Result<Vec<u8>> {
            self.serialize_i64(i64::from(value))
        }
        fn serialize_i16(self, value: i16) -> Result<Vec<u8>> {
            self.serialize_i64(i64::from(value))
        }
        fn serialize_i32(self, value: i32) -> Result<Vec<u8>> {
            self.serialize_i64(i64::from(value))
        }
        fn serialize_i64(self, value: i64) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::Signed(value))
        }
        fn serialize_u8(self, value: u8) -> Result<Vec<u8>> {
            self.serialize_u64(u64::from(value))
        }
        fn serialize_u16(self, value: u16) -> Result<Vec<u8>> {
            self.serialize_u64(u64::from(value))
        }
        fn serialize_u32(self, value: u32) -> Result<Vec<u8>> {
            self.serialize_u64(u64::from(value))
        }
        fn serialize_u64(self, value: u64) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::Unsigned(value))
        }
        fn serialize_f32(self, value: f32) -> Result<Vec<u8>> {
            self.serialize_f64(f64::from(value))
        }
        fn serialize_f64(self, value: f64) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::Float(value))
        }
        fn serialize_char(self, value: char) -> Result<Vec<u8>> {
            self.serialize_bytes(&[value as u8])
        }
        fn serialize_str(self, value: &str) -> Result<Vec<u8>> {
            self.serialize_bytes(value.as_bytes())
        }
        fn serialize_bytes(self, value: &[u8]) -> Result<Vec<u8>> {
            Ok(value.into())
        }
        fn serialize_unit(self) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::Unit)
        }
        fn serialize_unit_struct(self, _name: &'static str) -> Result<Vec<u8>> {
            self.serialize_unit()
        }
        fn serialize_unit_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
        ) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::UnitVariant)
        }
        fn serialize_newtype_struct<T: ?Sized + ser::Serialize>(
            self,
            _name: &'static str,
            _value: &T,
        ) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::NewtypeStruct)
        }
        fn serialize_newtype_variant<T: ?Sized + ser::Serialize>(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _value: &T,
        ) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::NewtypeVariant)
        }
        fn serialize_none(self) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::Option)
        }
        fn serialize_some<T: ?Sized + ser::Serialize>(self, _value: &T) -> Result<Vec<u8>> {
            unexpected(de::Unexpected::Option)
        }
        fn serialize_seq(self, _len: Option<usize>) -> Result<ser::Impossible<Vec<u8>, Error>> {
            unexpected(de::Unexpected::Seq)
        }
        fn serialize_tuple(self, _size: usize) -> Result<ser::Impossible<Vec<u8>, Error>> {
            unexpected(de::Unexpected::Seq)
        }
        fn serialize_tuple_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<ser::Impossible<Vec<u8>, Error>> {
            unexpected(de::Unexpected::NewtypeStruct)
        }
        fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<ser::Impossible<Vec<u8>, Error>> {
            unexpected(de::Unexpected::TupleVariant)
        }
        fn serialize_map(self, _len: Option<usize>) -> Result<ser::Impossible<Vec<u8>, Error>> {
            unexpected(de::Unexpected::Map)
        }
        fn serialize_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<ser::Impossible<Vec<u8>, Error>> {
            unexpected(de::Unexpected::NewtypeStruct)
        }
        fn serialize_struct_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<ser::Impossible<Vec<u8>, Error>> {
            unexpected(de::Unexpected::StructVariant)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize, Debug, PartialEq)]
    struct TestStruct {
        integer: i32,
        string: String,
        vector: Vec<i32>,
    }

    #[test]
    fn test_to_bytes() {
        // Test with a simple integer
        let int_value = 42;
        let int_bytes = to_bytes(&int_value).unwrap();
        assert_eq!(int_bytes, b"i42e");

        // Test with a string
        let string_value = "hello";
        let string_bytes = to_bytes(&string_value).unwrap();
        assert_eq!(string_bytes, b"5:hello");

        // Test with a complex struct
        let test_struct = TestStruct {
            integer: 42,
            string: "hello".to_string(),
            vector: vec![1, 2, 3],
        };
        let struct_bytes = to_bytes(&test_struct).unwrap();
        assert_eq!(
            struct_bytes,
            b"d7:integeri42e6:string5:hello6:vectorli1ei2ei3eee"
        );
    }

    #[test]
    fn test_to_string() {
        // Test with a simple integer
        let int_value = 42;
        let int_string = to_string(&int_value).unwrap();
        assert_eq!(int_string, "i42e");

        // Test with a string
        let string_value = "hello";
        let string_string = to_string(&string_value).unwrap();
        assert_eq!(string_string, "5:hello");

        // Test with a complex struct
        let test_struct = TestStruct {
            integer: 42,
            string: "hello".to_string(),
            vector: vec![1, 2, 3],
        };
        let struct_string = to_string(&test_struct).unwrap();
        assert_eq!(
            struct_string,
            "d7:integeri42e6:string5:hello6:vectorli1ei2ei3eee"
        );
    }

    #[test]
    fn test_to_value() {
        // Test with a simple integer
        let int_value = 42;
        let int_bencode = to_value(int_value).unwrap();
        assert_eq!(int_bencode, Value::Integer(42));

        // Test with a string
        let string_value = "hello";
        let string_bencode = to_value(string_value).unwrap();
        assert_eq!(string_bencode, Value::String("hello".to_string()));

        // Test with a complex struct
        let test_struct = TestStruct {
            integer: 42,
            string: "hello".to_string(),
            vector: vec![1, 2, 3],
        };
        let struct_bencode = to_value(test_struct).unwrap();
        if let Value::Dictionary(dict) = struct_bencode {
            assert_eq!(dict.get("integer"), Some(&Value::Integer(42)));
            assert_eq!(
                dict.get("string"),
                Some(&Value::String("hello".to_string()))
            );
            assert_eq!(
                dict.get("vector"),
                Some(&Value::List(vec![
                    Value::Integer(1),
                    Value::Integer(2),
                    Value::Integer(3)
                ]))
            );
        } else {
            panic!("Expected dictionary");
        }
    }
}
