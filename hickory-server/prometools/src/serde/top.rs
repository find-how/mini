use super::error::{Error, Unexpected};
use super::str::Writer;
use super::value;
use serde::ser::{Impossible, Serialize, SerializeStruct, Serializer};
use std::error;
use std::fmt;

#[inline]
pub(super) fn serializer(writer: Writer<'_>) -> impl '_ + Serializer<Ok = (), Error = Error> {
    TopSerializer { writer }
}

pub(super) struct TopSerializer<'w> {
    writer: Writer<'w>,
}

macro_rules! unsupported_scalars {
    ($($($method:ident: $kind:ident($ty:ty)),+ $(,)?)?) => {$($(
        #[inline]
        fn $method(self, v: $ty) -> Result<Self::Ok, Error> {
            Err(unsupported(Unexpected::$kind(v as _)))
        }
    )+)?}
}

impl<'w> Serializer for TopSerializer<'w> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = StructSerializer<'w>;
    type SerializeStructVariant = Impossible<(), Error>;

    unsupported_scalars! {
        serialize_bool: Bool(bool),
        serialize_i8: Signed(i8),
        serialize_i16: Signed(i16),
        serialize_i32: Signed(i32),
        serialize_i64: Signed(i64),
        serialize_u8: Unsigned(u8),
        serialize_u16: Unsigned(u16),
        serialize_u32: Unsigned(u32),
        serialize_u64: Unsigned(u64),
        serialize_f32: Float(f32),
        serialize_f64: Float(f64),
        serialize_char: Char(char),
    }

    #[inline]
    fn serialize_str(self, _value: &str) -> Result<(), Error> {
        Err(unsupported(Unexpected::Str))
    }

    #[inline]
    fn serialize_bytes(self, _value: &[u8]) -> Result<(), Error> {
        Err(unsupported(Unexpected::Bytes))
    }

    #[inline]
    fn serialize_unit(self) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        ty: &'static str,
        _index: u32,
        name: &'static str,
    ) -> Result<(), Error> {
        Err(unsupported(Unexpected::Variant(ty, name)))
    }

    #[inline]
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T>(
        self,
        ty: &'static str,
        _index: u32,
        name: &'static str,
        _value: &T,
    ) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        Err(unsupported(Unexpected::Variant(ty, name)))
    }

    #[inline]
    fn serialize_none(self) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Err(unsupported(Unexpected::Seq(len)))
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Error> {
        Err(unsupported(Unexpected::Tuple(len)))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        ty: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(unsupported(Unexpected::Struct(ty)))
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        ty: &'static str,
        _index: u32,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(unsupported(Unexpected::Variant(ty, name)))
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(unsupported(Unexpected::Map(len)))
    }

    #[inline]
    fn serialize_struct(
        self,
        _ty: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Ok(StructSerializer {
            has_written_anything: false,
            writer: self.writer,
        })
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        ty: &'static str,
        _index: u32,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(unsupported(Unexpected::Variant(ty, name)))
    }
}

pub(super) struct StructSerializer<'w> {
    has_written_anything: bool,
    writer: Writer<'w>,
}

impl SerializeStruct for StructSerializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        check_key(key)?;

        if self.has_written_anything {
            self.writer.write_str("\",").map_err(Error::new)?;
        } else {
            self.has_written_anything = true;
        }

        self.writer.write_str(key).map_err(Error::new)?;
        self.writer.write_str("=\"").map_err(Error::new)?;

        value.serialize(value::serializer(self.writer.reborrow()))
    }

    #[inline]
    fn end(mut self) -> Result<(), Error> {
        if self.has_written_anything {
            self.writer.write_str("\"").map_err(Error::new)?;
        }

        Ok(())
    }
}

fn check_key(key: &'static str) -> Result<(), Error> {
    let mut chars = key.chars();

    chars
        .next()
        .filter(|c| c.is_ascii_alphabetic() || *c == '_' || *c == ':')
        .ok_or_else(|| invalid_key(key))?;

    chars
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
        .then_some(())
        .ok_or_else(|| invalid_key(key))
}

fn invalid_key(key: &'static str) -> Error {
    #[derive(Debug)]
    struct InvalidKeyError(&'static str);

    impl error::Error for InvalidKeyError {
        #[allow(deprecated)]
        fn description(&self) -> &str {
            "invalid key"
        }
    }

    impl fmt::Display for InvalidKeyError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "invalid key ({:?})", self.0)
        }
    }

    Error::invalid_input(InvalidKeyError(key))
}

fn unsupported(kind: Unexpected) -> Error {
    #[derive(Debug)]
    struct UnsupportedInputError(Unexpected);

    impl error::Error for UnsupportedInputError {
        #[allow(deprecated)]
        fn description(&self) -> &str {
            "unsupported at top-level"
        }
    }

    impl fmt::Display for UnsupportedInputError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "unsupported {} at top-level", self.0)
        }
    }

    Error::invalid_input(UnsupportedInputError(kind))
}
