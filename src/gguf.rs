//! Minimal GGUF header parser.
//!
//! Reads only the file header: metadata key-value pairs and the tensor info
//! section (names, shapes, types, offsets). Tensor data is never read.
//!
//! Tensor GGML types are kept as raw `u32` values and never mapped to an
//! enum, so unknown or future quantization types (e.g. NVFP4 = 40, Q1_0 = 41,
//! Q2_0 = 42) parse cleanly instead of panicking.
//!
//! Format reference: https://github.com/ggml-org/ggml/blob/master/docs/gguf.md

use std::collections::BTreeMap;
use std::io::Read;

const MAGIC_GGUF_LE: u32 = 0x46554747; // "GGUF" bytes read as little-endian
const MAGIC_GGUF_BE: u32 = 0x47475546; // "GGUF" bytes read as big-endian

const MAX_KV_COUNT: u64 = 1_000_000;
const MAX_TENSOR_COUNT: u64 = 1_000_000;
const MAX_ARRAY_LEN: u64 = 1_000_000;
const MAX_STRING_LEN: u64 = 16 * 1024 * 1024;
const MAX_DIMS: u32 = 16;

/// A GGUF metadata value, mirroring the accessor semantics of
/// `serde_json::Value` (which the previous parser exposed).
#[derive(Debug, Clone, PartialEq)]
pub enum GgufValue {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
    String(String),
    Array(Vec<GgufValue>),
    U64(u64),
    I64(i64),
    F64(f64),
}

impl GgufValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            GgufValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            GgufValue::U8(v) => Some(*v as u64),
            GgufValue::U16(v) => Some(*v as u64),
            GgufValue::U32(v) => Some(*v as u64),
            GgufValue::U64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            GgufValue::I8(v) => Some(*v as i64),
            GgufValue::I16(v) => Some(*v as i64),
            GgufValue::I32(v) => Some(*v as i64),
            GgufValue::I64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            GgufValue::U8(v) => Some(*v as f64),
            GgufValue::I8(v) => Some(*v as f64),
            GgufValue::U16(v) => Some(*v as f64),
            GgufValue::I16(v) => Some(*v as f64),
            GgufValue::U32(v) => Some(*v as f64),
            GgufValue::I32(v) => Some(*v as f64),
            GgufValue::F32(v) => Some(*v as f64),
            GgufValue::U64(v) => Some(*v as f64),
            GgufValue::I64(v) => Some(*v as f64),
            GgufValue::F64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[GgufValue]> {
        match self {
            GgufValue::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Parsed GGUF header: metadata key-value pairs plus total parameter count
/// (sum of tensor element counts, computed from shapes alone).
#[derive(Debug, Clone, Default)]
pub struct GgufHeader {
    pub kv: BTreeMap<String, GgufValue>,
    pub parameters: u64,
}

/// Format a parameter count the way the previous parser did
/// (e.g. `8200000000` → `"8B"`, `1200000` → `"1M"`, `900` → `"900"`).
pub fn human_number(value: u64) -> String {
    match value {
        _ if value > 1_000_000_000 => format!("{:.0}B", value as f64 / 1_000_000_000.0),
        _ if value > 1_000_000 => format!("{:.0}M", value as f64 / 1_000_000.0),
        _ if value > 1_000 => format!("{:.0}K", value as f64 / 1_000.0),
        _ => format!("{}", value),
    }
}

struct Reader {
    inner: Box<dyn Read>,
    be: bool,
}

impl Reader {
    fn new(inner: Box<dyn Read>) -> Self {
        Self { inner, be: false }
    }

    fn read_u8(&mut self) -> anyhow::Result<u8> {
        let mut b = [0u8; 1];
        self.inner.read_exact(&mut b)?;
        Ok(b[0])
    }

    fn read_u16(&mut self) -> anyhow::Result<u16> {
        let mut b = [0u8; 2];
        self.inner.read_exact(&mut b)?;
        Ok(if self.be {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    }

    fn read_u32(&mut self) -> anyhow::Result<u32> {
        let mut b = [0u8; 4];
        self.inner.read_exact(&mut b)?;
        Ok(if self.be {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }

    fn read_u64(&mut self) -> anyhow::Result<u64> {
        let mut b = [0u8; 8];
        self.inner.read_exact(&mut b)?;
        Ok(if self.be {
            u64::from_be_bytes(b)
        } else {
            u64::from_le_bytes(b)
        })
    }

    fn read_i8(&mut self) -> anyhow::Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    fn read_i16(&mut self) -> anyhow::Result<i16> {
        Ok(self.read_u16()? as i16)
    }

    fn read_i32(&mut self) -> anyhow::Result<i32> {
        Ok(self.read_u32()? as i32)
    }

    fn read_i64(&mut self) -> anyhow::Result<i64> {
        Ok(self.read_u64()? as i64)
    }

    fn read_f32(&mut self) -> anyhow::Result<f32> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    fn read_f64(&mut self) -> anyhow::Result<f64> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    fn read_bool(&mut self) -> anyhow::Result<bool> {
        Ok(self.read_u8()? != 0)
    }

    fn read_string(&mut self, v1: bool) -> anyhow::Result<String> {
        let len = if v1 {
            self.read_u32()? as u64
        } else {
            self.read_u64()?
        };
        if len > MAX_STRING_LEN {
            return Err(anyhow::anyhow!("GGUF string too long: {} bytes", len));
        }
        let mut buf = vec![0u8; len as usize];
        self.inner.read_exact(&mut buf)?;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    fn read_value(&mut self, v1: bool) -> anyhow::Result<GgufValue> {
        let t = self.read_u32()?;
        Ok(match t {
            0 => GgufValue::U8(self.read_u8()?),
            1 => GgufValue::I8(self.read_i8()?),
            2 => GgufValue::U16(self.read_u16()?),
            3 => GgufValue::I16(self.read_i16()?),
            4 => GgufValue::U32(self.read_u32()?),
            5 => GgufValue::I32(self.read_i32()?),
            6 => GgufValue::F32(self.read_f32()?),
            7 => GgufValue::Bool(self.read_bool()?),
            8 => GgufValue::String(self.read_string(v1)?),
            9 => {
                let elem_type = self.read_u32()?;
                let len = if v1 {
                    self.read_u32()? as u64
                } else {
                    self.read_u64()?
                };
                if len > MAX_ARRAY_LEN {
                    return Err(anyhow::anyhow!("GGUF array too long: {} elements", len));
                }
                let mut items = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    items.push(self.read_value_elem(elem_type, v1)?);
                }
                GgufValue::Array(items)
            }
            10 => GgufValue::U64(self.read_u64()?),
            11 => GgufValue::I64(self.read_i64()?),
            12 => GgufValue::F64(self.read_f64()?),
            _ => return Err(anyhow::anyhow!("unsupported GGUF value type: {}", t)),
        })
    }

    /// Read one array element of a known type. Arrays of arrays are not
    /// valid GGUF, so the element type is always a scalar or a string.
    fn read_value_elem(&mut self, t: u32, v1: bool) -> anyhow::Result<GgufValue> {
        Ok(match t {
            0 => GgufValue::U8(self.read_u8()?),
            1 => GgufValue::I8(self.read_i8()?),
            2 => GgufValue::U16(self.read_u16()?),
            3 => GgufValue::I16(self.read_i16()?),
            4 => GgufValue::U32(self.read_u32()?),
            5 => GgufValue::I32(self.read_i32()?),
            6 => GgufValue::F32(self.read_f32()?),
            7 => GgufValue::Bool(self.read_bool()?),
            8 => GgufValue::String(self.read_string(v1)?),
            10 => GgufValue::U64(self.read_u64()?),
            11 => GgufValue::I64(self.read_i64()?),
            12 => GgufValue::F64(self.read_f64()?),
            _ => {
                return Err(anyhow::anyhow!(
                    "unsupported GGUF array element type: {}",
                    t
                ));
            }
        })
    }
}

/// Parse the GGUF header (metadata + tensor info) of the file at `path`.
///
/// Never reads tensor data and never fails on unknown tensor GGML types.
pub fn parse_header(path: &std::path::Path) -> anyhow::Result<GgufHeader> {
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("cannot open {}: {}", path.display(), e))?;
    let mut r = Reader::new(Box::new(file));

    // Detect byte order from the magic: bytes "GGUF" read as LE.
    let magic = r.read_u32()?;
    r.be = match magic {
        MAGIC_GGUF_LE => false,
        MAGIC_GGUF_BE => true,
        _ => return Err(anyhow::anyhow!("not a GGUF file (bad magic)")),
    };

    let version = r.read_u32()?;
    let v1 = version == 1;
    if !(1..=3).contains(&version) {
        return Err(anyhow::anyhow!("unsupported GGUF version: {}", version));
    }

    let tensor_count = if v1 {
        r.read_u32()? as u64
    } else {
        r.read_u64()?
    };
    let kv_count = if v1 {
        r.read_u32()? as u64
    } else {
        r.read_u64()?
    };
    if tensor_count > MAX_TENSOR_COUNT {
        return Err(anyhow::anyhow!(
            "GGUF tensor count too large: {}",
            tensor_count
        ));
    }
    if kv_count > MAX_KV_COUNT {
        return Err(anyhow::anyhow!(
            "GGUF metadata count too large: {}",
            kv_count
        ));
    }

    let mut kv = BTreeMap::new();
    for _ in 0..kv_count {
        let key = r.read_string(v1)?;
        let value = r.read_value(v1)?;
        kv.insert(key, value);
    }

    let mut parameters: u64 = 0;
    for _ in 0..tensor_count {
        let _name = r.read_string(v1)?;
        let n_dim = r.read_u32()?;
        if n_dim > MAX_DIMS {
            return Err(anyhow::anyhow!("invalid tensor dimension count: {}", n_dim));
        }
        let mut params: u128 = 1;
        for _ in 0..n_dim {
            params = params.saturating_mul(r.read_u64()? as u128);
        }
        // Raw GGML type: intentionally kept as a plain u32 and ignored, so
        // new quantization types never break header parsing.
        let _ggml_type = r.read_u32()?;
        let _offset = r.read_u64()?;
        parameters = parameters.saturating_add(params as u64);
    }

    Ok(GgufHeader { kv, parameters })
}
