//! Values that can be processed by jaq.
//!
//! To process your own value type with jaq,
//! you need to implement the [`ValT`] trait.

use crate::box_iter::BoxIter;
use crate::path::Opt;
use core::convert::Infallible;
use core::fmt::{Debug, Display};
use core::ops::{Add, Div, Mul, Neg, Rem, Sub};

// Makes `f64::from_str` accessible as intra-doc link.
#[cfg(doc)]
use core::str::FromStr;
use std::string::ToString;

use alloc::string::String;

/// Value or eRror.
pub type ValR<V> = Result<V, crate::Error<V>>;
/// Value or eXception.
pub type ValX<'a, V> = Result<V, crate::Exn<'a, V>>;
/// Stream of values and eXceptions.
pub type ValXs<'a, V> = BoxIter<'a, ValX<'a, V>>;

/// Range of options, used for iteration operations.
pub type Range<V> = core::ops::Range<Option<V>>;

pub type ValStrOps<V: ValT> = V::ValStrOps;
pub type ValString<V: ValT> = <V::ValStrOps as ValStrOpsT>::ValString;
pub type ValStr<V: ValT> = <V::ValStrOps as ValStrOpsT>::ValStr;
pub type ValChar<V: ValT> = <V::ValStrOps as ValStrOpsT>::ValChar;

/// Values that can be processed by jaq.
///
/// Implement this trait if you want jaq to process your own type of values.
pub trait ValT:
    Clone
    + Display
    + From<bool>
    + From<isize>
    + From<alloc::string::String>
    + FromIterator<Self>
    + PartialEq
    + PartialOrd
    + Add<Output = ValR<Self>>
    + Sub<Output = ValR<Self>>
    + Mul<Output = ValR<Self>>
    + Div<Output = ValR<Self>>
    + Rem<Output = ValR<Self>>
    + Neg<Output = ValR<Self>>
{
    type ValStrOps: ValStrOpsT;

    /// Create a number from a string.
    ///
    /// The number should adhere to the format accepted by [`f64::from_str`].
    fn from_num(n: &str) -> ValR<Self>;

    fn from_string(n: ValString<Self>) -> ValR<Self>;

    /// Create an associative map (or object) from a sequence of key-value pairs.
    ///
    /// This is used when creating values with the syntax `{k: v}`.
    fn from_map<I: IntoIterator<Item = (Self, Self)>>(iter: I) -> ValR<Self>;

    /// Yield the children of a value.
    ///
    /// This is used by `.[]`.
    fn values(self) -> alloc::boxed::Box<dyn Iterator<Item = ValR<Self>>>;

    /// Yield the child of a value at the given index.
    ///
    /// This is used by `.[k]`.
    ///
    /// If `v.index(k)` is `Ok(_)`, then it is contained in `v.values()`.
    fn index(self, index: &Self) -> ValR<Self>;

    /// Yield a slice of the value with the given range.
    ///
    /// This is used by `.[s:e]`, `.[s:]`, and `.[:e]`.
    fn range(self, range: Range<&Self>) -> ValR<Self>;

    /// Map a function over the children of the value.
    ///
    /// This is used by
    /// - `.[]  |= f` (`opt` = [`Opt::Essential`]) and
    /// - `.[]? |= f` (`opt` = [`Opt::Optional`]).
    ///
    /// If the children of the value are undefined, then:
    ///
    /// - If `opt` is [`Opt::Essential`], return an error.
    /// - If `opt` is [`Opt::Optional`] , return the input value.
    fn map_values<'a, I: Iterator<Item = ValX<'a, Self>>>(
        self,
        opt: Opt,
        f: impl Fn(Self) -> I,
    ) -> ValX<'a, Self>;

    /// Map a function over the child of the value at the given index.
    ///
    /// This is used by `.[k] |= f`.
    ///
    /// See [`Self::map_values`] for the behaviour of `opt`.
    fn map_index<'a, I: Iterator<Item = ValX<'a, Self>>>(
        self,
        index: &Self,
        opt: Opt,
        f: impl Fn(Self) -> I,
    ) -> ValX<'a, Self>;

    /// Map a function over the slice of the value with the given range.
    ///
    /// This is used by `.[s:e] |= f`, `.[s:] |= f`, and `.[:e] |= f`.
    ///
    /// See [`Self::map_values`] for the behaviour of `opt`.
    fn map_range<'a, I: Iterator<Item = ValX<'a, Self>>>(
        self,
        range: Range<&Self>,
        opt: Opt,
        f: impl Fn(Self) -> I,
    ) -> ValX<'a, Self>;

    /// Return a boolean representation of the value.
    ///
    /// This is used by `if v then ...`.
    fn as_bool(&self) -> bool;

    /// If the value is string, return it.
    ///
    /// If `v.as_str()` yields `Some(s)`, then
    /// `"\(v)"` yields `s`, otherwise it yields `v.to_string()`
    /// (provided by [`Display`]).
    fn as_str(&self) -> Option<&ValStr<Self>>;
}

pub trait ValStrOpsT {
    type ValString: Debug + From<String> + for<'a> From<&'a str> + FromIterator<Self::ValChar> + Default;
    type ValStr: Debug + ?Sized;
    type ValChar: Debug + Copy;

    /// If the value is a valid UTF-8 string, return it.
    fn validate_str(val_str: &Self::ValStr) -> Result<&str, impl core::error::Error>;

    /// If the value is a valid UTF-8 string, return it.
    fn validate_into_string(val_str: Self::ValString) -> Result<String, impl core::error::Error>;

    // TODO: jaq-std uses isize instead of i32 for some reason .. look into why this is?
    fn char_to_i32(c: Self::ValChar) -> i32;

    fn char_from_i32(c: i32) -> Option<Self::ValChar>;

    fn char_from_utf16(u: u16) -> Result<Self::ValChar, impl core::error::Error>;

    // mz TODO: docs
    // TODO: rename str_utf8 ?
    fn str_bytes(val_str: &Self::ValStr) -> impl Iterator<Item = Result<&[u8], impl core::error::Error>>;

    fn str_utf8_bytes(val_str: &Self::ValStr) -> impl Iterator<Item = Result<u8, impl core::error::Error>>;

    // mz TODO: docs
    fn str_chars<'a>(val_str: &'a Self::ValStr) -> impl DoubleEndedIterator<Item = Self::ValChar> + 'a;

    // mz TODO: add default impl?
    fn from_bytes(data: &[u8]) -> Result<Self::ValString, impl core::error::Error>;

    // mz TODO: try to avoid Result<Result<..>>?
    fn from_read<E>(read: &mut impl FnMut(&mut [u8]) -> Result<usize, E>) -> Result<Result<Self::ValString, impl core::error::Error>, E>;

    // mz TODO: try to avoid Result<Result<..>>?
    fn lines_from_read<E>(read: impl FnMut(&mut [u8]) -> Result<usize, E>) -> impl Iterator<Item = Result<Result<Self::ValString, impl core::error::Error>, E>>;

    fn push_val_str(out: &mut Self::ValString, data: &Self::ValStr) -> Result<(), impl core::error::Error>;

    fn push_str(out: &mut Self::ValString, data: &str) -> Result<(), impl core::error::Error>;

    fn push_utf8(out: &mut Self::ValString, data: &[u8]) -> Result<(), impl core::error::Error>;

    fn push_utf16(out: &mut Self::ValString, data: &[u16]) -> Result<(), impl core::error::Error>;
}

pub struct StrStrOps;

impl ValStrOpsT for StrStrOps {
    type ValString = String;
    type ValStr = str;
    type ValChar = char;

    fn validate_str(val_str: &Self::ValStr) -> Result<&str, impl core::error::Error> {
        Ok::<_, Infallible>(val_str)
    }

    fn validate_into_string(val_str: Self::ValString) -> Result<String, impl core::error::Error> {
        Ok::<_, Infallible>(val_str)
    }

    fn char_to_i32(c: Self::ValChar) -> i32 {
        c as i32
    }

    fn char_from_i32(c: i32) -> Option<Self::ValChar> {
        char::from_u32(c as u32)
    }

    fn char_from_utf16(u: u16) -> Result<Self::ValChar, impl core::error::Error> {
        char::try_from(u as u32)
    }

    // TODO: rename str_utf8 ?
    fn str_bytes(val_str: &Self::ValStr) -> impl Iterator<Item = Result<&[u8], impl core::error::Error>> {
        [val_str.as_bytes()].into_iter()
            .map(|bytes| Ok::<_, Infallible>(bytes))
    }

    fn str_utf8_bytes(val_str: &Self::ValStr) -> impl Iterator<Item = Result<u8, impl core::error::Error>> {
        val_str.as_bytes().into_iter()
            .copied()
            .map(|byte| Ok::<_, Infallible>(byte))
    }

    fn str_chars(val_str: &Self::ValStr) -> impl DoubleEndedIterator<Item = Self::ValChar> {
        val_str.chars()
    }

    fn from_bytes(data: &[u8]) -> Result<Self::ValString, impl core::error::Error> {
        String::from_utf8(data.into())
    }

    fn from_read<E>(mut read: &mut impl FnMut(&mut [u8]) -> Result<usize, E>) -> Result<Result<Self::ValString, impl core::error::Error>, E> {
        let mut out = alloc::vec::Vec::new();
        let mut buf = [0; 1024];
        loop {
            let size = read(&mut buf)?;
            if size == 0 {
                return Ok(String::from_utf8(out));
            }
            out.extend_from_slice(&buf[..size]);
        }
    }

    fn lines_from_read<E>(read: impl FnMut(&mut [u8]) -> Result<usize, E>) -> impl Iterator<Item = Result<Result<Self::ValString, impl core::error::Error>, E>> {
        return I { read, buf: alloc::vec::Vec::new(), };
        struct I<R> {
            read: R,
            buf: alloc::vec::Vec<u8>,
        }
        impl<E, R: FnMut(&mut [u8]) -> Result<usize, E>> Iterator for I<R> {
            type Item = Result<Result<String, std::string::FromUtf8Error>, E>;

            fn next(&mut self) -> Option<Self::Item> {
                let mut buf = core::mem::take(&mut self.buf);
                let mut search_start = 0;
                loop {
                    if let Some(i) = buf[search_start..].iter().position(|c| *c == b'\n') {
                        self.buf.extend_from_slice(&buf[(search_start + i + 1)..]);
                        buf.truncate(search_start + i);
                        return Some(Ok(String::from_utf8(buf)));
                    };
                    let size = buf.len();
                    buf.resize(size + 1024, 0);
                    let tmp = &mut buf[size..];
                    let read_size = match (self.read)(tmp) {
                        Ok(s) => s,
                        Err(e) => return Some(Err(e)),
                    };
                    buf.truncate(size + read_size);
                    if read_size == 0 {
                        if buf.is_empty() {
                            return None;
                        } else {
                            return Some(Ok(String::from_utf8(buf)));
                        }
                    }
                    search_start = size;
                }
            }
        }
    }

    fn push_val_str(out: &mut Self::ValString, data: &str) -> Result<(), impl core::error::Error> {
        Self::push_str(out, data)
    }

    fn push_str(out: &mut Self::ValString, data: &str) -> Result<(), impl core::error::Error> {
        out.push_str(data);
        Ok::<_, Infallible>(())
    }

    fn push_utf8(out: &mut Self::ValString, data: &[u8]) -> Result<(), impl core::error::Error> {
        match core::str::from_utf8(data) {
            Ok(s) => { out.push_str(s); Ok(()) },
            Err(e) => Err(e),
        }
    }

    fn push_utf16(out: &mut Self::ValString, data: &[u16]) -> Result<(), impl core::error::Error> {
        let mut buf = [0; 4];
        for c in core::char::decode_utf16(data.iter().copied()) {
            let c = match c {
                Ok(c) => c,
                Err(e) => return Err(e),
            };
            out.push_str(c.encode_utf8(&mut buf));
        }
        Ok(())
    }
}
