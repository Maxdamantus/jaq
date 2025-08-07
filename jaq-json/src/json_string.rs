use core::{error::Error, fmt::{Debug, Display}};

use alloc::{string::String, vec::Vec};
use ref_cast::{ref_cast_custom, RefCastCustom};

// similar to the std `char` type, this is not normally stored; negative numbers are used to represent errors, non-negatives correspond with valid `char` values
#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct JsonChar(i32);

#[derive(Copy, Clone, Debug)]
pub struct JsonCharError<T>(pub T);

#[derive(Copy, Clone)]
pub enum JsonCodeUnit {
    U8(u8),
    U16(u16),
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct JsonString {
    content: Vec<u8>,
}

#[derive(RefCastCustom)]
#[repr(transparent)]
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct JsonStr {
    content: [u8],
}

impl Debug for JsonStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("\"")?;
        for maybe_str in self.iter_str() {
            // NOTE: ugly allocations; in theory could be avoided by creating a temporary `Write` impl that drops the first byte and buffers the last?
            let tmp = match maybe_str {
                Ok(str) => alloc::format!("{:?}", str),
                Err(JsonCharError(cu)) => alloc::format!("{:?}", cu),
            };
            // drop the quotes
            f.write_str(&tmp[1..(tmp.len() - 1)])?;
        }
        f.write_str("\"")?;
        Ok(())
    }
}

impl Debug for JsonString {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(self.as_ref(), f)
    }
}

impl Debug for JsonCodeUnit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            JsonCodeUnit::U8(unit8) => write!(f, "'\\x{:02x}'", unit8),
            JsonCodeUnit::U16(unit16) => write!(f, "'\\u{:04x}'", unit16),
        }
    }
}

impl Debug for JsonChar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.to_char() {
            Ok(c) => Debug::fmt(&c, f),
            Err(cu) => Debug::fmt(&cu, f),
        }
    }
}

impl<T: Debug> Error for JsonCharError<T> {}

impl<T: Debug> Display for JsonCharError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Can't process character {:?}", self.0)
    }
}

impl From<&[u8]> for JsonString {
    fn from(value: &[u8]) -> Self {
        JsonString::from_utf8(value)
    }
}

impl From<&str> for JsonString {
    fn from(value: &str) -> Self {
        JsonString::from_string(value.into())
    }
}

impl From<&JsonStr> for JsonString {
    fn from(value: &JsonStr) -> Self {
        value.to_json_string()
    }
}

impl From<String> for JsonString {
    fn from(value: String) -> Self {
        JsonString::from_string(value.into())
    }
}

impl From<char> for JsonChar {
    fn from(value: char) -> Self {
        JsonChar::from_char(value)
    }
}

impl FromIterator<JsonChar> for JsonString {
    fn from_iter<T: IntoIterator<Item = JsonChar>>(iter: T) -> Self {
        let mut s = JsonString::new();
        for c in iter {
            s.push_json_char(c);
        }
        return s;
    }
}

/*
impl From<char> for JsonChar {
    fn from(value: char) -> Self {
        JsonChar::Char(value)
    }
}
*/

/*
impl From<std::string::String> for JsonString {
    fn from(value: String) -> Self {
        value.into()
    }
}
*/

/*
// TODO: deduplicate
impl From<char> for JsonString {
    fn from(value: char) -> Self {
        JsonString {
            content: value.into(),
        }
    }
}
*/

impl JsonChar {
    pub fn to_json_string(self) -> JsonString {
        let mut s = JsonString::new();
        s.push_json_char(self);
        s
    }

    pub fn from_char(char: char) -> JsonChar {
        JsonChar(char as i32)
    }

    pub fn from_utf8_code_unit(unit: u8) -> JsonChar {
        if unit < 0x80 {
            JsonChar::from_char(unit as char)
        } else {
            JsonChar(-(unit as i32))
        }
    }

    pub fn from_utf16_code_unit(unit: u16) -> JsonChar {
        if let Some(c) = char::from_u32(unit as u32) {
            JsonChar::from_char(c)
        } else {
            JsonChar(-(unit as i32))
        }
    }

    pub fn to_char(self) -> Result<char, JsonCharError<JsonCodeUnit>> {
        if let Some(c) = char::from_u32(self.0 as u32) {
            Ok(c)
        } else {
            let unit = -self.0;
            if unit < 0x100 {
                debug_assert!(unit >= 0x80);
                Err(JsonCharError(JsonCodeUnit::U8(unit as u8)))
            } else {
                debug_assert!(unit >= 0xD800 && unit < 0xE000);
                Err(JsonCharError(JsonCodeUnit::U16(unit as u16)))
            }
        }
    }
}

impl JsonString {
    pub fn new() -> JsonString {
        JsonString::from_string(String::new())
    }

    pub fn from_string(content: String) -> JsonString {
        JsonString { content: content.into_bytes() }
    }

    pub fn from_json_char(c: JsonChar) -> JsonString {
        c.to_json_string()
    }

    pub fn from_utf8(content: &[u8]) -> JsonString {
        let mut out = JsonString::new();
        out.push_utf8(content);
        out
    }

    pub fn maybe_use_utf8(self) -> Result<String, alloc::string::FromUtf8Error> {
        String::from_utf8(self.content)
    }

    pub fn contains(&self, needle: impl Pattern) -> bool {
        todo!()
    }

    pub fn push_json_str(&mut self, mut other: &JsonStr) {
        if other.is_empty() {
            return;
        }
        let (len, jc) = encoding::wtf8b_read_start(&other.content);
        match jc.to_char() {
            // no error, no fix possible
            Ok(_) => {},
            // if `other` starts with an error, push relevant code units to try and fix the errors
            Err(JsonCharError(JsonCodeUnit::U8(unit8))) => {
                // read up to 3 UTF-8 errors, since the largest possible correction is 1 error from `self` and 3 errors from `other` making a 4-byte code point
                other = &other[len.size()..];
                let mut buf = [0; 3];
                let mut x = 0;
                buf[x] = unit8;
                x += 1;
                while x < 3 && !other.is_empty() {
                    // TODO: optimisation: use `wtf8b_decode_utf8_error` instead?
                    let (len, jc) = encoding::wtf8b_read_start(&other.content);
                    match jc.to_char() {
                        Err(JsonCharError(JsonCodeUnit::U8(unit8))) => {
                            other = &other[len.size()..];
                            buf[x] = unit8;
                            x += 1;
                        },
                        _ => break,
                    }
                }
                self.push_utf8(&buf[..x]);
            },
            Err(JsonCharError(JsonCodeUnit::U16(unit16))) => {
                // no need to read more UTF-16 errors, since the largest possible correction is 1 error from `self` and 1 error from `other` making a 2-surrogate code point
                other = &other[len.size()..];
                self.push_utf16(&[unit16]);
            }
        }
        self.content.extend(&other.content);
    }

    pub fn push_str(&mut self, other: &str) {
        self.content.extend(other.as_bytes())
    }

    pub fn push_utf8(&mut self, mut other: &[u8]) {
        self.content.reserve(other.len());
        loop {
            match core::str::from_utf8(other) {
                Ok(other) => {
                    self.push_str(other);
                    return;
                },
                Err(err) => {
                    let i = err.valid_up_to();
                    self.content.extend(&other[0..i]); // valid UTF-8
                    other = &other[i..];
                    other = encoding::wtf8b_push_utf8_error(&mut self.content, other);
                },
            }
        }
    }

    pub fn push_utf16(&mut self, other: &[u16]) {
        // TODO: make this more optimal for longer inputs? probably only called with a 1-length slice, so it doesn't matter
        for utf16 in other {
            self.push_json_char(JsonChar::from_utf16_code_unit(*utf16));
        }
    }

    pub fn push_json_char(&mut self, i: JsonChar) {
        match i.to_char() {
            Ok(c) => self.push_str(c.encode_utf8(&mut [0; 4])),
            Err(JsonCharError(JsonCodeUnit::U8(data))) => {
                let data = &[data];
                let rest = encoding::wtf8b_push_utf8_error(&mut self.content, data);
                debug_assert!(rest.is_empty());
            },
            Err(JsonCharError(JsonCodeUnit::U16(data))) => encoding::wtf8b_push_utf16_error(&mut self.content, data),
        }
    }

    // safely try to read directly into the string; if the read is invalid, some data will be copied twice
    pub fn read_utf8_into<E>(&mut self, read_size: usize, read: &mut impl FnMut(&mut [u8]) -> Result<usize, E>) -> Result<usize, E> {
        let mut out = core::mem::replace(self, JsonString::new());
        let old_size = out.content.len();
        let tmp_size = old_size + read_size;
        if tmp_size < old_size {
            panic!("overflow");
        }
        out.content.resize(old_size + read_size, 0);
        let read_size = read(&mut out.content[old_size..])?;
        let read_size_valid = encoding::utf8_valid_up_to(&out.content[old_size..(old_size + read_size)]);
        let invalid_suffix = out.content[(old_size + read_size_valid)..(old_size + read_size)].to_vec();
        out.content.truncate(old_size + read_size_valid);
        out.push_utf8(&invalid_suffix);
        *self = out;
        return Ok(self.len() - old_size);
    }

    pub fn truncate(&mut self, size: usize) {
        self.content.truncate(self.check_index(size));
    }
}

impl core::ops::Deref for JsonString {
    type Target = JsonStr;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
        //&JsonStr { content: self.content };
    }
}

impl AsRef<JsonStr> for JsonString {
    fn as_ref(&self) -> &JsonStr {
        from_bytes_unchecked(self.content.as_slice())
    }
}

#[ref_cast_custom]
fn from_bytes_unchecked(s: &[u8]) -> &JsonStr;

fn from_str(s: &str) -> &JsonStr {
    // this is fine; valid UTF-8 has the same meaning internally in `JsonStr`
    from_bytes_unchecked(s.as_bytes())
}

impl JsonStr {
    pub fn char_indices(&self) -> impl Iterator<Item = (usize, JsonChar)> + '_ {
        struct I<'a> {
            index: usize,
            cur: &'a [u8],
        }

        impl Iterator for I<'_> {
            type Item = (usize, JsonChar);

            fn next(&mut self) -> Option<(usize, JsonChar)> {
                if self.cur.is_empty() {
                    return None;
                }
                let index = self.index;
                let (len, result) = encoding::wtf8b_read_start(self.cur);
                let size = len.size();
                self.cur = &self.cur[size..];
                self.index += size;
                return Some((index, result));
            }
        }

        I {
            index: 0,
            cur: &self.content,
        }
    }

    pub fn len(&self) -> usize {
        self.content.len()
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn maybe_utf8(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.content)
    }

    // TODO: consider using `Pattern` trait?
    pub fn split_inclusive(&self, pat: impl Pattern) -> impl Iterator<Item = &JsonStr> {
        self.split_inclusive_x(pat)
            .filter(|(item, pat_len, is_final)| !is_final || item.len() != 0) // mz TODO
            .map(|(s, _, _)| s)
    }

    // TODO: consider using `Pattern` trait?
    pub fn split<'a>(&'a self, pat: impl Pattern) -> impl Iterator<Item = &'a JsonStr> {
        self.split_inclusive_x(pat)
            .map(|(s, pat_len, _)| s.slice(0, s.len() - pat_len))
    }

    fn split_inclusive_x(&self, pat: impl Pattern) -> impl Iterator<Item = (&JsonStr, usize, bool)> {
        struct I<'a, P> {
            haystack: Option<&'a JsonStr>,
            pat: P,
        }
        impl <'a, P: Pattern> Iterator for I<'a, P> {
            type Item = (&'a JsonStr, usize, bool);

            fn next(&mut self) -> Option<Self::Item> {
                let haystack = self.haystack?;
                if let Some((start, end)) = P::next_match(haystack, &self.pat) {
                    let out = haystack.slice(0, end);
                    self.haystack = Some(haystack.slice(end, haystack.len()));
                    return Some((out, end - start, false));
                }
                self.haystack = None;
                return Some((haystack, 0, true));
            }
        }
        return I {
            haystack: Some(self),
            pat,
        };
    }

    fn slice(&self, start: usize, end: usize) -> &JsonStr {
        from_bytes_unchecked(&self.content[self.check_index(start)..self.check_index(end)])
    }

    fn check_index(&self, i: usize) -> usize {
        if i == self.len() || i == 0 {
            return i;
        }
        if i < 0 || i > self.len() {
            panic!();
        }
        if encoding::wtf8b_is_continuation(self.content[i]) {
            panic!();
        }
        return i;
    }

    fn check_range(&self, range@(start, end): (usize, usize)) -> (usize, usize) {
        if self.check_index(start) > self.check_index(end) {
            panic!();
        }
        return range;
    }

    // TODO: rename `json_chars`?
    pub fn chars(&self) -> JsonChars<'_> {
        JsonChars { content: self }
    }

    pub fn repeat(&self, n: usize) -> JsonString {
        todo!()
        //JsonString::from_string(self.content.repeat(n))
    }

    // TODO: use `From` instead
    pub fn to_json_string(&self) -> JsonString {
        JsonString { content: self.content.to_vec() }
    }

    // UTF-8 and UTF-16 errors are replaced with `Err` values
    pub fn iter_str(&self) -> impl Iterator<Item = Result<&str, JsonCharError<JsonCodeUnit>>> {
        self.iter_utf8_valid()
            .map(|maybe_utf8| maybe_utf8
                .map(|utf8| core::str::from_utf8(utf8).unwrap())
            )
    }

    // UTF-16 errors are replaced with `Err` values
    pub fn iter_utf8(&self) -> impl Iterator<Item = Result<&[u8], JsonCharError<u16>>> {
        self.iter_utf8_valid()
            .map(|item| match item {
                Ok(utf8_valid) => Ok(utf8_valid),
                Err(JsonCharError(JsonCodeUnit::U16(error16))) => Err(JsonCharError(error16)),
                Err(JsonCharError(JsonCodeUnit::U8(error8))) => Ok(encoding::error_byte(error8)),
            })
    }

    // UTF-8 and UTF-16 errors are replaced with `Err` values
    // NOTE: this arguably shouldn't exist since it's equivalent to `iter_str`, but having `iter_str` as a base case would involve potentially doing the `from_utf8` check twice, or using `unsafe`, or using `utf8_chunks`
    // TODO: consider using `utf8_chunks` instead once it's part of MSRV
    fn iter_utf8_valid(&self) -> impl Iterator<Item = Result<&[u8], JsonCharError<JsonCodeUnit>>> {
        struct I<'a>(&'a [u8]);
        impl <'a> Iterator for I<'a> {
            type Item = Result<&'a [u8], JsonCharError<JsonCodeUnit>>;

            fn next(&mut self) -> Option<Self::Item> {
                if self.0.is_empty() {
                    return None;
                }
                let valid_len = encoding::utf8_valid_up_to(self.0);
                if valid_len > 0 {
                    let ret = &self.0[..valid_len];
                    self.0 = &self.0[valid_len..];
                    return Some(Ok(ret));
                }
                let (len, c) = encoding::wtf8b_read_start(self.0);
                self.0 = &self.0[len.size()..];
                return Some(match c.to_char() {
                    Ok(_) => unreachable!(),
                    Err(err) => Err(err),
                });
            }
        }
        return I(&self.content);
    }

    // UTF-16 errors are replaced with replacement characters
    pub fn iter_utf8_lossy(&self) -> impl Iterator<Item = &[u8]> {
        self.iter_utf8()
            .map(|data| data.unwrap_or_else(|_err| "\u{FFFD}".as_bytes()))
    }

    pub fn iter_utf8_bytes<'a>(&'a self) -> impl Iterator<Item = Result<u8, JsonCharError<u16>>> + 'a {
        type JCE = JsonCharError<u16>;
        struct I<F, T, U> {
            f: F,
            titer: T,
            uiter: U,
        }
        impl<'a, T, U, F> Iterator for I<F, T, U>
        where
            T: Iterator<Item = Result<&'a [u8], JCE>>,
            U: Iterator<Item = u8>,
            F: Fn(&'a [u8]) -> U,
        {
            type Item = Result<u8, JCE>;

            fn next(&mut self) -> Option<Self::Item> {
                loop {
                    if let Some(u) = self.uiter.next() {
                        return Some(Ok(u));
                    }
                    if let Some(t) = self.titer.next() {
                        match t {
                            Ok(t) => {
                                self.uiter = (self.f)(t);
                                continue;
                            },
                            Err(e) => {
                                return Some(Err(e));
                            },
                        }
                    }
                    return None;
                }
            }
        }
        fn iter<'a, T, U, F: Fn(&'a [u8]) -> U + 'a>(titer: T, f: F) -> I<F, T, U> {
            I { titer, uiter: f(&[]), f }
        }
        return iter(self.iter_utf8(), |bytes: &[u8]| bytes.iter().copied());
    }
}

// TODO: should this be more like the (currently experimental `str::Pattern`?)
trait Pattern {
    fn next_match(haystack: &JsonStr, needle: &Self) -> Option<(usize, usize)>;
}

impl Pattern for &JsonStr {
    fn next_match(haystack: &JsonStr, needle: &&JsonStr) -> Option<(usize, usize)> {
        let mut haystack: &[u8] = &haystack.content;
        let mut n = 0;
        while haystack.len() > needle.len() {
            if haystack.starts_with(&needle.content) {
                return Some((n, n + needle.len()));
            }
            n += 1;
            haystack = &haystack[1..];
        }
        return None;
    }
}

impl Pattern for &JsonString {
    fn next_match(haystack: &JsonStr, needle: &Self) -> Option<(usize, usize)> {
        Pattern::next_match(haystack, &needle.as_ref())
    }
}

struct JsonCharPattern<T>(T);

impl <T: Fn(JsonChar) -> bool> Pattern for JsonCharPattern<T> {
    fn next_match(haystack: &JsonStr, needle: &JsonCharPattern<T>) -> Option<(usize, usize)> {
        let mut iter = haystack.char_indices();
        loop {
            let (n, c) = iter.next()?;
            if !needle.0(c) {
                continue;
            }
            let end = match iter.next() {
                Some((o, _)) => o,
                None => haystack.len(),
            };
            return Some((n, end));
        }
    }
}

impl <T: Fn(char) -> bool> Pattern for T {
    fn next_match(haystack: &JsonStr, needle: &T) -> Option<(usize, usize)> {
        Pattern::next_match(haystack, &JsonCharPattern(|jc: JsonChar| match jc.to_char() {
            Ok(c) => needle(c),
            _ => false,
        }))
    }
}

/*
impl core::ops::Index<usize> for JsonStr {
    type Output = JsonChar;

    fn index(&self, index: usize) -> &JsonChar {
        to_str(self).index(index)
    }
}
*/

impl core::ops::Index<core::ops::Range<usize>> for JsonStr {
    type Output = JsonStr;

    fn index(&self, index: core::ops::Range<usize>) -> &Self::Output {
        let start = self.check_index(index.start);
        let end = self.check_index(index.end);
        from_bytes_unchecked(&self.content[start..end])
    }
}

impl core::ops::Index<core::ops::RangeFrom<usize>> for JsonStr {
    type Output = JsonStr;

    fn index(&self, index: core::ops::RangeFrom<usize>) -> &Self::Output {
        let start = self.check_index(index.start);
        from_bytes_unchecked(&self.content[start..])
    }
}

impl core::ops::Index<core::ops::RangeTo<usize>> for JsonStr {
    type Output = JsonStr;

    fn index(&self, index: core::ops::RangeTo<usize>) -> &Self::Output {
        let end = self.check_index(index.end);
        from_bytes_unchecked(&self.content[..end])
    }
}

impl<'a, 'b> PartialEq<&JsonStr> for JsonString {
    fn eq(&self, other: &&JsonStr) -> bool {
        self.content == other.content
    }
}

impl<'a, 'b> PartialEq<JsonString> for &JsonStr {
    fn eq(&self, other: &JsonString) -> bool {
        self.content == other.content
    }
}

/*
impl<I> core::ops::Index<I> for JsonStr
where
    I: core::slice::SliceIndex<JsonStr>
{
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        to_str(self)[index]
    }
}
*/

pub struct JsonChars<'a> {
    content: &'a JsonStr,
}

impl Iterator for JsonChars<'_> {
    type Item = JsonChar;

    fn next(&mut self) -> Option<JsonChar> {
        if self.content.is_empty() {
            return None;
        }
        let (len, result) = encoding::wtf8b_read_start(&self.content.content);
        self.content = &self.content[len.size()..];
        return Some(result);
    }
}

impl DoubleEndedIterator for JsonChars<'_> {
    fn next_back(&mut self) -> Option<JsonChar> {
        if self.content.is_empty() {
            return None;
        }
        let old_len = self.content.len();
        let mut x = old_len - 1;
        while encoding::wtf8b_is_continuation(self.content.content[x]) {
            x -= 1;
        }
        let (len, result) = encoding::wtf8b_read_start(&self.content.content[x..]);
        assert!(x + len.size() == self.content.len());
        self.content = &self.content[..x];
        return Some(result);
    }
}

impl<'a> JsonChars<'a> {
    // TODO: rename `as_json_str`?
    pub fn as_json_str(&self) -> &'a JsonStr {
        self.content
    }
}

mod encoding {
    use alloc::vec::Vec;

    use super::JsonChar;

    #[derive(PartialEq, Clone, Copy, Debug)]
    pub enum DecodeLength {
        L1 = 1, L2 = 2, L3 = 3, L4 = 4
    }

    impl DecodeLength {
        pub(super) fn size(self) -> usize {
            return self as usize;
        }

        fn utf8_first_code_point(self) -> u32 {
            match self {
                Self::L1 => 0x0,
                Self::L2 => 0x80,
                Self::L3 => 0x800,
                Self::L4 => 0x1000,
            }
        }
    }

    const B0: u8 = 0b00000000;
    const B1: u8 = 0b10000000;
    const B2: u8 = 0b11000000;
    const B3: u8 = 0b11100000;
    const B4: u8 = 0b11110000;
    const B5: u8 = 0b11111000;

    #[inline]
    fn d32(a: u8, b: u8, c: u8, d: u8) -> u32 {
        (a as u32) << 0 |
        (b as u32) << 8 |
        (c as u32) << 16 |
        (d as u32) << 24
    }

    #[inline]
    fn bits(data: u32, byte: u32, mask_b: u8, shift: u8) -> u32 {
        ((data >> (byte*8)) & ((!mask_b) as u32)) << shift
    }

    // TODO: see if it's actually useful having this as a trait instead of a bitset?
    // seems more likely to result in inlining, but haven't checked
    trait DecodeFlags {
        fn l1() -> bool { false }
        fn l2() -> bool { false }
        fn l3() -> bool { false }
        fn l4() -> bool { false }
    }

    // decode up to 4 bytes of "generalised UTF-8"; no checking for overlong
    // codings or out-of-range code points, works by testing all fixed bits in each
    // of the 4 coding patterns, then shifting the value bits according to the
    // pattern
    #[inline]
    fn decode_x<F: DecodeFlags>(data: u32) -> Option<(DecodeLength, u32)> {
        if F::l1() && (data & d32(B1, B0, B0, B0)) == 0 {
            return Some((
                DecodeLength::L1,
                bits(data, 0, B0, 0)
            ));
        }
        if F::l2() && (data & d32(B3, B2, B0, B0)) == d32(B2, B1, B0, B0) {
            return Some((
                DecodeLength::L2,
                bits(data, 0, B3, 6) |
                bits(data, 1, B2, 0)
            ));
        }
        if F::l3() && (data & d32(B4, B2, B2, B0)) == d32(B3, B1, B1, B0) {
            return Some((
                DecodeLength::L3,
                bits(data, 0, B4, 12) |
                bits(data, 1, B2, 6) |
                bits(data, 2, B2, 0)
            ));
        }
        if F::l4() && (data & d32(B5, B2, B2, B2)) == d32(B4, B1, B1, B1) {
            return Some((
                DecodeLength::L4,
                bits(data, 0, B5, 18) |
                bits(data, 1, B2, 12) |
                bits(data, 2, B2, 6) |
                bits(data, 3, B2, 0)
            ));
        }
        return None;
    }

    // for decoding everything
    fn decode_1234(data: u32) -> Option<(DecodeLength, u32)> {
        struct F();
        impl DecodeFlags for F {
            fn l1() -> bool { true }
            fn l2() -> bool { true }
            fn l3() -> bool { true }
            fn l4() -> bool { true }
        }
        decode_x::<F>(data)
    }

    // for decoding everything other than ASCII (since we're detecting that specially as an optimisation)
    fn decode_234(data: u32) -> Option<(DecodeLength, u32)> {
        struct F();
        impl DecodeFlags for F {
            fn l2() -> bool { true }
            fn l3() -> bool { true }
            fn l4() -> bool { true }
        }
        let result = decode_x::<F>(data);
        debug_assert!(result == decode_1234(data));
        result
    }

    // for decoding UTF-8 errors
    fn decode_2(data: u32) -> Option<u32> {
        struct F();
        impl DecodeFlags for F {
            fn l2() -> bool { true }
        }
        let (len, v) = decode_x::<F>(data)?;
        debug_assert!(len == DecodeLength::L2);
        Some(v)
    }

    // for decoding UTF-16 errors
    fn decode_3(data: u32) -> Option<u32> {
        struct F();
        impl DecodeFlags for F {
            fn l3() -> bool { true }
        }
        let (len, v) = decode_x::<F>(data)?;
        debug_assert!(len == DecodeLength::L3);
        Some(v)
    }

    fn wtf8b_decode_utf8_error(data: &[u8; 2]) -> Option<u8> {
        let v = decode_2(d32(data[0], data[1], 0, 0))?;
        if v < 0x80 {
            Some((v as u8) + 0x80)
        } else {
            None
        }
    }

    fn wtf8b_decode_utf16_error(data: &[u8; 3]) -> Option<u16> {
        let v = decode_3(d32(data[0], data[1], data[2], 0))?;
        if v >= 0xD800 && v < 0xE000 {
            Some(v as u16)
        } else {
            None
        }
    }

    fn wtf8b_encode_utf8_error(v: u8) -> [u8; 2] {
        debug_assert!(v >= 0x80);
        let v = v - 0x80;
        // same as normal 2-byte UTF-8 pattern, though we're essentially encoding overlong ASCII
        [
            B2 | (v >> 6) & !B3,
            B1 | (v >> 0) & !B2,
        ]
    }

    fn wtf8b_encode_utf16_error(v: u16) -> [u8; 3] {
        debug_assert!(v >= 0xD800 && v < 0xE000);
        // same as normal 3-byte UTF-8 pattern, though we're encoding non-USV code points
        [
            B3 | (v >> 12) as u8 & !B4,
            B1 | (v >> 6) as u8 & !B2,
            B1 | (v >> 0) as u8 & !B2,
        ]
    }

    // NOTE: will panic on invalid WTF-8b or empty input
    pub(super) fn wtf8b_read_start(rest: &[u8]) -> (DecodeLength, JsonChar) {
        let b = rest[0];
        if (b & B1) == 0 {
            // fast path for ASCII
            return (DecodeLength::L1, JsonChar::from_char(b as char));
        }
        // TODO: is there a better way to write this?
        let data = match rest.len() {
            1 => d32(rest[0], 0, 0, 0),
            2 => d32(rest[0], rest[1], 0, 0),
            3 => d32(rest[0], rest[1], rest[2], 0),
            _ => d32(rest[0], rest[1], rest[2], rest[3]),
        };
        let (len, cp) = decode_234(data).unwrap();
        let code_point = if cp < len.utf8_first_code_point() {
            // overlong representation, used to encode UTF-8 error
            if len != DecodeLength::L2 {
                panic!()
            }
            JsonChar::from_utf8_code_unit((cp + 0x80) as u8)
        } else if cp >= 0xD800 && cp < 0xE000 {
            JsonChar::from_utf16_code_unit(cp as u16)
        } else {
            JsonChar::from_char(char::from_u32(cp).unwrap())
        };
        return (len, code_point);
    }

    pub(super) fn wtf8b_is_continuation(data: u8) -> bool {
        return (data & B2) == B1;
    }

    pub(super) fn utf8_length_from_lead_byte(data: u8) -> Option<DecodeLength> {
        if (data & B1) == B0 {
            Some(DecodeLength::L1) // ASCII
        } else if (data & B3) == B2 {
            Some(DecodeLength::L2)
        } else if (data & B4) == B3 {
            Some(DecodeLength::L3)
        } else if (data & B5) == B4 {
            Some(DecodeLength::L4)
        } else {
            None
        }
    }

    pub(super) fn error_byte(data: u8) -> &'static [u8] {
        static ERROR_BYTES: [u8; 128] = [
            // jaq -r -n '"0123456789ABCDEF" | split("") as $hex | $hex[8:][] as $h0 | ["0x" + $h0 + $hex[]] | join(", ") + ","'
            0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F,
            0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F,
            0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF,
            0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF,
            0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF,
            0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF,
            0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF,
            0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
        ];
        let i = (data - 0x80) as usize;
        let slice = &ERROR_BYTES[i..i + 1];
        debug_assert!(slice[0] == data);
        return slice;
    }

    // count number of valid UTF-8 bytes at start of slice
    pub(super) fn utf8_valid_up_to(utf8: &[u8]) -> usize {
        match core::str::from_utf8(utf8) {
            Ok(str) => str.as_bytes().len(),
            Err(e) => e.valid_up_to(),
        }
    }

    #[derive(Debug)]
    struct IncompleteUtf8 {
        len: usize,
        needed: usize,
        utf8_buf: [u8; 3],
    }

    impl IncompleteUtf8 {
        fn utf8_slice(&self) -> &[u8] {
            &self.utf8_buf[(self.utf8_buf.len() - self.len)..]
        }
    }

    // returns size of `wtf8b_content` with the suffix removed, and final decoded UTF-8 code units
    fn wtf8b_decode_utf8_errors_suffix<'a, 'b>(wtf8b_content: &'a [u8]) -> Option<(usize, IncompleteUtf8)> {
        let wtf_len = wtf8b_content.len();
        let mut utf8_buf = [0; 3];
        let mut buf_i = 3;
        let mut i = 2;
        while buf_i > 0 && i < wtf_len {
            let error_start = wtf_len - i;
            buf_i -= 1;
            let error = wtf8b_decode_utf8_error(<_>::try_from(&wtf8b_content[error_start..(error_start + 2)]).unwrap())?;
            utf8_buf[buf_i] = error;
            if !wtf8b_is_continuation(error) {
                let len = 3 - buf_i;
                let complete_len = utf8_length_from_lead_byte(error)?.size();
                return Some((wtf_len - i, IncompleteUtf8 {
                    len,
                    needed: if len < complete_len {
                        complete_len - len
                    } else {
                        // already has the expected number of continuation bytes; not a correctable error
                        None?
                    },
                    utf8_buf,
                }));
            }
            i += 2;
        }
        return None;
    }

    // returns size of `wtf8b_content` with the suffix removed, and final decoded UTF-16 code unit
    fn wtf8b_decode_utf16_error_suffix<'a, 'b>(wtf8b_content: &'a [u8]) -> Option<(usize, u16)> {
        let len = wtf8b_content.len();
        if len < 3 {
            return None;
        }
        return Some((len - 3, wtf8b_decode_utf16_error(<_>::try_from(&wtf8b_content[(len - 3)..]).unwrap())?));
    }

    // assumes `utf8` starts with invalid UTF-8, returns remaining slice of data that hasn't been pushed
    // up to 2 bytes of valid UTF-8 (following an error) might also be pushed from `utf8`
    pub(super) fn wtf8b_push_utf8_error<'a>(wtf8b_content: &mut Vec<u8>, utf8: &'a [u8]) -> &'a [u8] {
        assert!(utf8[0] >= 0x80);
        debug_assert!(utf8_valid_up_to(utf8) == 0);
        if let Some((wtf8b_content_len, incomplete)) = wtf8b_decode_utf8_errors_suffix(&wtf8b_content[..]) {
            if utf8.len() >= incomplete.needed {
                let incomplete_utf8 = incomplete.utf8_slice();
                let incomplete_len = incomplete_utf8.len();
                let mut buf = [0; 6];
                buf[..incomplete_len].copy_from_slice(incomplete_utf8);
                buf[incomplete_len..(incomplete_len + incomplete.needed)].copy_from_slice(&utf8[..incomplete.needed]);
                if let Ok(join) = core::str::from_utf8(&buf[..(incomplete_len + incomplete.needed)]) {
                    wtf8b_content.truncate(wtf8b_content_len);
                    wtf8b_content.extend_from_slice(join.as_bytes());
                    return &utf8[incomplete.needed..];
                }
            }
        }
        // correction not possible, just append one byte from `utf8` as an error
        wtf8b_content.extend_from_slice(&wtf8b_encode_utf8_error(utf8[0]));
        return &utf8[1..];
    }

    pub(super) fn wtf8b_push_utf16_error(wtf8b_content: &mut Vec<u8>, utf16: u16) {
        assert!(utf16 >= 0xD800 && utf16 < 0xE000);
        if let Some((wtf8b_content_len, prev_utf16)) = wtf8b_decode_utf16_error_suffix(&wtf8b_content[..]) {
            if let Ok(join) = alloc::string::String::from_utf16(&[prev_utf16, utf16]) {
                wtf8b_content.truncate(wtf8b_content_len);
                wtf8b_content.extend_from_slice(join.as_bytes());
                return;
            }
        }
        // correction not possible, just append the code unit from `utf16` as an error
        wtf8b_content.extend_from_slice(&wtf8b_encode_utf16_error(utf16));
    }
}
