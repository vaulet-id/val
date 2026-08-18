//! Reading deterministic CBOR back.
//!
//! **Strict**: an encoding that is legal CBOR but not the deterministic one is
//! refused. That is the point of the exercise — if two byte strings decode to
//! the same value, then a hash over the bytes stops being a hash over the
//! value, and every root in this system means less than it says.

use std::collections::BTreeMap;

use crate::value::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Malformed {
    Truncated,
    /// A length or integer written longer than it needed to be.
    NotShortest { at: usize },
    /// Map keys out of order, or repeated.
    KeysNotSorted { at: usize },
    Unsupported { major: u8, at: usize },
    Trailing { at: usize },
    NotUtf8 { at: usize },
}

pub fn decode(bytes: &[u8]) -> Result<Value, Malformed> {
    let mut p = P { b: bytes, i: 0 };
    let v = p.value()?;
    if p.i != bytes.len() {
        return Err(Malformed::Trailing { at: p.i });
    }
    Ok(v)
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn byte(&mut self) -> Result<u8, Malformed> {
        let b = *self.b.get(self.i).ok_or(Malformed::Truncated)?;
        self.i += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], Malformed> {
        let end = self.i.checked_add(n).ok_or(Malformed::Truncated)?;
        let s = self.b.get(self.i..end).ok_or(Malformed::Truncated)?;
        self.i = end;
        Ok(s)
    }

    /// The head, with the shortest-form rule enforced on the way past.
    fn head(&mut self) -> Result<(u8, u64), Malformed> {
        let at = self.i;
        let b = self.byte()?;
        let major = b >> 5;
        let low = b & 0x1f;
        let arg = match low {
            0..=23 => low as u64,
            24 => {
                let v = self.byte()? as u64;
                if v < 24 {
                    return Err(Malformed::NotShortest { at });
                }
                v
            }
            25 => {
                let s = self.take(2)?;
                let v = u16::from_be_bytes([s[0], s[1]]) as u64;
                if v <= 0xff {
                    return Err(Malformed::NotShortest { at });
                }
                v
            }
            26 => {
                let s = self.take(4)?;
                let v = u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as u64;
                if v <= 0xffff {
                    return Err(Malformed::NotShortest { at });
                }
                v
            }
            27 => {
                let s = self.take(8)?;
                let v = u64::from_be_bytes(s.try_into().unwrap());
                if v <= 0xffff_ffff {
                    return Err(Malformed::NotShortest { at });
                }
                v
            }
            _ => return Err(Malformed::Unsupported { major, at }),
        };
        Ok((major, arg))
    }

    fn value(&mut self) -> Result<Value, Malformed> {
        let at = self.i;
        let (major, arg) = self.head()?;
        Ok(match major {
            0 => Value::Int(arg as i64),
            1 => Value::Int(-1 - arg as i64),
            2 => Value::Bytes(self.take(arg as usize)?.to_vec()),
            3 => {
                let s = self.take(arg as usize)?;
                Value::Str(std::str::from_utf8(s).map_err(|_| Malformed::NotUtf8 { at })?.to_string())
            }
            4 => {
                let mut items = Vec::with_capacity(arg.min(1024) as usize);
                for _ in 0..arg {
                    items.push(self.value()?);
                }
                Value::List(items)
            }
            6 if arg == crate::canonical::TAG_ENUM => match self.value()? {
                Value::List(items) => match items.as_slice() {
                    [Value::Str(a), Value::Str(b)] => Value::Enum(a.clone(), b.clone()),
                    _ => return Err(Malformed::Unsupported { major: 6, at }),
                },
                _ => return Err(Malformed::Unsupported { major: 6, at }),
            },
            6 if arg == crate::canonical::TAG_CREDENTIAL => match self.value()? {
                Value::Map(m) => Value::Credential {
                    ty: match m.get("type") {
                        Some(Value::Str(s)) => s.clone(),
                        _ => String::new(),
                    },
                    claims: match m.get("claims") {
                        Some(Value::Map(c)) => c.clone(),
                        _ => BTreeMap::new(),
                    },
                    verified: match m.get("verified") {
                        Some(Value::Str(s)) => Some(s.clone()),
                        _ => None,
                    },
                },
                _ => return Err(Malformed::Unsupported { major: 6, at }),
            },
            6 => return Err(Malformed::Unsupported { major: 6, at }),
            5 => {
                let mut m = BTreeMap::new();
                let mut last: Option<Vec<u8>> = None;
                for _ in 0..arg {
                    let key_at = self.i;
                    let key_start = self.i;
                    let key = match self.value()? {
                        Value::Str(s) => s,
                        _ => return Err(Malformed::Unsupported { major: 5, at: key_at }),
                    };
                    let key_bytes = self.b[key_start..self.i].to_vec();
                    if let Some(prev) = &last {
                        if *prev >= key_bytes {
                            return Err(Malformed::KeysNotSorted { at: key_at });
                        }
                    }
                    last = Some(key_bytes);
                    let v = self.value()?;
                    m.insert(key, v);
                }
                Value::Map(m)
            }
            7 => match arg {
                20 => Value::Bool(false),
                21 => Value::Bool(true),
                22 => Value::Null,
                _ => return Err(Malformed::Unsupported { major: 7, at }),
            },
            other => return Err(Malformed::Unsupported { major: other, at }),
        })
    }
}
