#![doc = include_str!("../README.md")]

use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

/// [Msg] is a trait for defining custom formatters for [NoDebug] values.
pub trait Msg<T> {
    /// Prints a custom message to the given formatter without necessarily revealing the values
    /// information.
    ///
    /// Takes a reference to the value being debugged to allow some introspection.
    fn fmt(value: &T, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error>;
}

#[derive(Debug, Clone)]
pub struct WithTypeInfo;

impl<T> Msg<T> for WithTypeInfo {
    fn fmt(_value: &T, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "<no debug: {}>", std::any::type_name::<T>())
    }
}

#[derive(Debug, Clone)]
pub struct Ellipses;

impl<T> Msg<T> for Ellipses {
    fn fmt(_value: &T, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "...")
    }
}

/// Wraps a type `T` and provides a [Debug] impl that does not rely on `T` being [Debug].
#[derive(Eq, Ord, Clone)]
pub struct NoDebug<T, M: Msg<T> = WithTypeInfo>(T, std::marker::PhantomData<M>);

impl<T: std::hash::Hash, M: Msg<T>> std::hash::Hash for NoDebug<T, M> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash::<H>(state)
    }
}

impl<T: PartialOrd, M: Msg<T>> std::cmp::PartialOrd<T> for NoDebug<T, M> {
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl<T: PartialOrd, M: Msg<T>, N: Msg<T>> std::cmp::PartialOrd<NoDebug<T, N>> for NoDebug<T, M> {
    fn partial_cmp(&self, other: &NoDebug<T, N>) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&**other)
    }
}

impl<T: PartialEq, M: Msg<T>> std::cmp::PartialEq<T> for NoDebug<T, M> {
    fn eq(&self, other: &T) -> bool {
        &self.0 == other
    }
}

impl<T: PartialEq, M: Msg<T>, N: Msg<T>> std::cmp::PartialEq<NoDebug<T, N>> for NoDebug<T, M> {
    fn eq(&self, other: &NoDebug<T, N>) -> bool {
        **self == **other
    }
}

impl<T, M: Msg<T>> NoDebug<T, M> {
    pub fn take(self) -> T {
        self.0
    }
}

impl<T> NoDebug<T, WithTypeInfo> {
    pub fn new(value: T) -> Self {
        value.into()
    }
}

impl<T, M: Msg<T>> From<T> for NoDebug<T, M> {
    fn from(value: T) -> Self {
        Self(value, std::marker::PhantomData::default())
    }
}

impl<T, M: Msg<T>> Debug for NoDebug<T, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        M::fmt(&self.0, f)
    }
}

impl<T, M: Msg<T>> Deref for NoDebug<T, M> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, M: Msg<T>> DerefMut for NoDebug<T, M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cannot_debug_nodebug() {
        let value = NoDebug::new(3);
        assert_eq!(format!("{:?}", value), "<no debug: i32>")
    }

    #[test]
    fn cannot_debug_nodebug_via_into() {
        let value: NoDebug<i32> = 3.into();
        assert_eq!(format!("{:?}", value), "<no debug: i32>")
    }

    #[test]
    fn can_show_type_info() {
        let value: NoDebug<i32, WithTypeInfo> = 3.into();
        assert_eq!(format!("{:?}", value), "<no debug: i32>")
    }

    #[test]
    fn can_show_custom_message() {
        let value: NoDebug<i32, Ellipses> = 3.into();
        assert_eq!(format!("{:?}", value), "...")
    }

    #[test]
    fn dereferences_nodebug() {
        let value = NoDebug::new(3);
        assert_eq!(format!("{:?}", value), "<no debug: i32>");
        assert_eq!(format!("{:?}", *value), "3");
    }

    #[test]
    fn mut_dereferences_nodebug() {
        let mut value = NoDebug::new(3);
        *value = 4;
        assert_eq!(format!("{:?}", value), "<no debug: i32>");
        assert_eq!(format!("{:?}", *value), "4");
    }

    #[test]
    fn take_gets_value_from_nodebug() {
        let value = NoDebug::new(3);
        assert_eq!(value.take(), 3);
    }

    #[test]
    fn has_eq_with_inner() {
        let value = NoDebug::new(3);
        assert_eq!(*value, 3);
    }

    #[test]
    fn has_eq_with_raw_value() {
        let value = NoDebug::new(3);
        assert_eq!(value, 3);
    }

    #[test]
    fn has_eq_with_another_no_debug() {
        let value = NoDebug::new(3);
        let other = NoDebug::new(3);
        assert_eq!(value, other);
    }

    #[test]
    fn has_eq_with_another_no_debug_with_different_msg() {
        let value: NoDebug<i32, Ellipses> = 3.into();
        let other: NoDebug<i32, WithTypeInfo> = 3.into();
        assert_eq!(value, other);
    }

    #[test]
    fn has_ord_with_raw_value() {
        let value = NoDebug::new(2);
        assert!(value < 3);
    }

    #[test]
    fn has_ord_with_another_no_debug() {
        let value = NoDebug::new(2);
        let other = NoDebug::new(3);
        assert!(value < other);
    }

    #[test]
    fn has_ord_with_another_no_debug_with_different_msg() {
        let value: NoDebug<i32, Ellipses> = 2.into();
        let other: NoDebug<i32, WithTypeInfo> = 3.into();
        assert!(value < other);
    }

    fn get_hash<T>(obj: T) -> u64
    where
        T: std::hash::Hash,
    {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        let mut hasher = DefaultHasher::new();
        obj.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn has_hash_with_raw_value() {
        let value: NoDebug<i32> = 3.into();
        assert_eq!(get_hash(value), get_hash(3));
    }

    #[test]
    fn has_hash_with_another_no_debug() {
        let value: NoDebug<i32> = 3.into();
        let other: NoDebug<i32> = 3.into();
        assert_eq!(get_hash(value), get_hash(other));
    }

    #[test]
    fn has_hash_with_another_no_debug_with_different_msg() {
        let value: NoDebug<i32, Ellipses> = 3.into();
        let other: NoDebug<i32, WithTypeInfo> = 3.into();
        assert_eq!(get_hash(value), get_hash(other));
    }
}
