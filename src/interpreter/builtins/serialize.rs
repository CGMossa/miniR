//! GNU R binary serialization — reading RDS files written by GNU R.
//!
//! Implements the XDR binary format (version 2/3) used by `readRDS()`/`saveRDS()`.
//! See R-ints.texi "Serialization Formats" for the spec.

use crate::interpreter::value::*;
use indexmap::IndexMap;

// region: constants

/// R's NA_INTEGER is i32::MIN
const R_NA_INTEGER: i32 = i32::MIN;

/// R's NA_LOGICAL is also i32::MIN in serialization
const R_NA_LOGICAL: i32 = i32::MIN;

/// R's NA_REAL bit pattern: 0x7FF00000000007A2 (a specific NaN)
const R_NA_REAL_BITS: u64 = 0x7FF00000000007A2;

// SEXPTYPE codes
const NILSXP: u8 = 0;
const SYMSXP: u8 = 1;
const LISTSXP: u8 = 2;
const CLOSXP: u8 = 4;
const LANGSXP: u8 = 6;
const CHARSXP: u8 = 9;
const LGLSXP: u8 = 10;
const INTSXP: u8 = 13;
const REALSXP: u8 = 14;
const CPLXSXP: u8 = 15;
const STRSXP: u8 = 16;
const VECSXP: u8 = 19;
const EXPRSXP: u8 = 20;
const RAWSXP: u8 = 24;

// Pseudo-SEXPTYPE codes
const REFSXP: u8 = 255;
const NILVALUE_SXP: u8 = 254;
const GLOBALENV_SXP: u8 = 244;
const BASEENV_SXP: u8 = 243;
const EMPTYENV_SXP: u8 = 242;
#[allow(dead_code)]
const UNBOUNDVALUE_SXP: u8 = 245;
const MISSINGARG_SXP: u8 = 246;
const BASENAMESPACE_SXP: u8 = 247;
const NAMESPACESXP: u8 = 249;

// Flag bits
const HAS_ATTR_MASK: u32 = 1 << 9;
const HAS_TAG_MASK: u32 = 1 << 10;

// endregion

// region: XdrReader

/// Cursor-based reader for big-endian (XDR) binary data.
struct XdrReader<'a> {
    data: &'a [u8],
    pos: usize,
    /// Reference table for back-references (pseudo-SEXPTYPE 255).
    ref_table: Vec<RValue>,
}

impl<'a> XdrReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        XdrReader {
            data,
            pos: 0,
            ref_table: Vec::new(),
        }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], RError> {
        if self.pos + n > self.data.len() {
            return Err(RError::new(
                RErrorKind::Other,
                format!(
                    "unexpected end of RDS data: need {} bytes at offset {}, have {}",
                    n,
                    self.pos,
                    self.remaining()
                ),
            ));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    /// Read a big-endian i32.
    fn read_int(&mut self) -> Result<i32, RError> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a big-endian f64.
    fn read_double(&mut self) -> Result<f64, RError> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Read a length value, handling long vectors (length == -1 means 64-bit length).
    fn read_length(&mut self) -> Result<usize, RError> {
        let len = self.read_int()?;
        if len >= 0 {
            Ok(len as usize)
        } else if len == -1 {
            // Long vector: next two i32s form a 64-bit length (upper, lower).
            let upper = self.read_int()? as u32;
            let lower = self.read_int()? as u32;
            let long_len = (u64::from(upper) << 32) | u64::from(lower);
            usize::try_from(long_len).map_err(|_| {
                RError::new(
                    RErrorKind::Other,
                    format!("vector length {} too large for this platform", long_len),
                )
            })
        } else {
            Err(RError::new(
                RErrorKind::Other,
                format!("invalid vector length: {}", len),
            ))
        }
    }

    /// Read a CHARSXP: length + raw bytes. Length -1 means NA_STRING.
    fn read_charsxp(&mut self) -> Result<Option<String>, RError> {
        let len = self.read_int()?;
        if len == -1 {
            return Ok(None); // NA_STRING
        }
        let n = usize::try_from(len).map_err(|_| {
            RError::new(
                RErrorKind::Other,
                format!("invalid CHARSXP length: {}", len),
            )
        })?;
        let bytes = self.read_bytes(n)?;
        // R strings can be in various encodings; assume UTF-8 or Latin-1.
        // Try UTF-8 first, fall back to lossy conversion.
        match std::str::from_utf8(bytes) {
            Ok(s) => Ok(Some(s.to_string())),
            Err(_) => {
                // Latin-1 fallback: each byte maps to its Unicode codepoint
                let s: String = bytes.iter().map(|&b| b as char).collect();
                Ok(Some(s))
            }
        }
    }

    /// Register a value in the reference table and return it.
    fn ref_add(&mut self, value: RValue) -> RValue {
        self.ref_table.push(value.clone());
        value
    }

    /// Look up a reference by 1-based index.
    fn ref_get(&self, index: usize) -> Result<RValue, RError> {
        if index == 0 || index > self.ref_table.len() {
            return Err(RError::new(
                RErrorKind::Other,
                format!(
                    "invalid reference index {} (table has {} entries)",
                    index,
                    self.ref_table.len()
                ),
            ));
        }
        Ok(self.ref_table[index - 1].clone())
    }

    /// Read flags integer and extract SEXPTYPE, has-attr, has-tag bits.
    fn read_flags(&mut self) -> Result<(u8, bool, bool, u32), RError> {
        let flags = self.read_int()? as u32;
        let sxp_type = (flags & 0xFF) as u8;
        let has_attr = flags & HAS_ATTR_MASK != 0;
        let has_tag = flags & HAS_TAG_MASK != 0;
        Ok((sxp_type, has_attr, has_tag, flags))
    }

    /// Read attributes stored as a pairlist, returning an Attributes map.
    fn read_attributes(&mut self) -> Result<Attributes, RError> {
        let mut attrs = IndexMap::new();
        // Attributes are stored as a pairlist (LISTSXP chain).
        // Each node has: flags, tag (symbol), car (value), then cdr (next node or NILVALUE).
        loop {
            let (sxp_type, _has_attr, has_tag, _flags) = self.read_flags()?;
            match sxp_type {
                LISTSXP => {
                    let tag_name = if has_tag {
                        self.read_item_as_symbol()?
                    } else {
                        String::new()
                    };
                    let value = self.read_item()?;
                    if !tag_name.is_empty() {
                        attrs.insert(tag_name, value);
                    }
                    // CDR is the next node — continue the loop via recursion,
                    // but we handle it iteratively by reading the next flags.
                    // Actually, the CDR is explicitly written. We need to check
                    // if the next item is NILVALUE_SXP to stop.
                    // Peek at what we just consumed for CDR... Actually the pairlist
                    // structure means after reading TAG+CAR, we need to read CDR.
                    // But CDR is the next pairlist node, which is the next iteration.
                    // This doesn't work right - we need to read the full pairlist recursively.
                }
                NILVALUE_SXP => break,
                _ => {
                    return Err(RError::new(
                        RErrorKind::Other,
                        format!(
                            "unexpected SEXPTYPE {} in attribute pairlist (expected LISTSXP or NILVALUE)",
                            sxp_type
                        ),
                    ));
                }
            }
        }
        Ok(attrs)
    }

    /// Read one serialized item recursively.
    fn read_item(&mut self) -> Result<RValue, RError> {
        let (sxp_type, has_attr, has_tag, flags) = self.read_flags()?;
        self.read_item_inner(sxp_type, has_attr, has_tag, flags)
    }

    fn read_item_inner(
        &mut self,
        sxp_type: u8,
        has_attr: bool,
        has_tag: bool,
        flags: u32,
    ) -> Result<RValue, RError> {
        match sxp_type {
            NILVALUE_SXP => Ok(RValue::Null),
            NILSXP => Ok(RValue::Null),

            EMPTYENV_SXP | BASEENV_SXP | GLOBALENV_SXP | BASENAMESPACE_SXP => {
                // Environment singletons — we can't reconstruct full R environments,
                // but we register them so reference indices stay correct.
                let val = RValue::Null;
                Ok(self.ref_add(val))
            }

            MISSINGARG_SXP => Ok(RValue::Null),

            NAMESPACESXP => {
                // Namespace: read a STRSXP with the namespace info,
                // register as ref, return Null placeholder.
                let _info = self.read_item()?;
                let val = RValue::Null;
                Ok(self.ref_add(val))
            }

            REFSXP => {
                // Reference: the flags contain the packed index.
                // If the index in the flags field is 0, read an explicit integer.
                let ref_index = (flags >> 8) as usize;
                if ref_index == 0 {
                    let idx = self.read_int()? as usize;
                    self.ref_get(idx)
                } else {
                    self.ref_get(ref_index)
                }
            }

            SYMSXP => {
                // Symbol: read a CHARSXP for the name.
                let (inner_type, _ia, _it, inner_flags) = self.read_flags()?;
                let name = if inner_type == CHARSXP {
                    self.read_charsxp_with_flags(inner_flags)?
                        .unwrap_or_default()
                } else {
                    // Unexpected — try to read as item and convert
                    return Err(RError::new(
                        RErrorKind::Other,
                        format!("expected CHARSXP inside SYMSXP, got type {}", inner_type),
                    ));
                };
                let val = RValue::vec(Vector::Character(vec![Some(name)].into()));
                Ok(self.ref_add(val))
            }

            CHARSXP => {
                let s = self.read_charsxp_with_flags(flags)?;
                Ok(match s {
                    Some(s) => RValue::vec(Vector::Character(vec![Some(s)].into())),
                    None => RValue::Null,
                })
            }

            LGLSXP => {
                let len = self.read_length()?;
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    let raw = self.read_int()?;
                    if raw == R_NA_LOGICAL {
                        values.push(None);
                    } else {
                        values.push(Some(raw != 0));
                    }
                }
                let mut rv = RVector::from(Vector::Logical(values.into()));
                if has_attr {
                    let attrs = self.read_attributes()?;
                    rv.attrs = Some(Box::new(attrs));
                }
                Ok(RValue::Vector(rv))
            }

            INTSXP => {
                let len = self.read_length()?;
                let mut values: Vec<Option<i64>> = Vec::with_capacity(len);
                for _ in 0..len {
                    let raw = self.read_int()?;
                    if raw == R_NA_INTEGER {
                        values.push(None);
                    } else {
                        values.push(Some(i64::from(raw)));
                    }
                }
                let mut rv = RVector::from(Vector::Integer(values.into()));
                if has_attr {
                    let attrs = self.read_attributes()?;
                    rv.attrs = Some(Box::new(attrs));
                }
                Ok(RValue::Vector(rv))
            }

            REALSXP => {
                let len = self.read_length()?;
                let mut values: Vec<Option<f64>> = Vec::with_capacity(len);
                for _ in 0..len {
                    let val = self.read_double()?;
                    if val.to_bits() == R_NA_REAL_BITS {
                        values.push(None);
                    } else {
                        values.push(Some(val));
                    }
                }
                let mut rv = RVector::from(Vector::Double(values.into()));
                if has_attr {
                    let attrs = self.read_attributes()?;
                    rv.attrs = Some(Box::new(attrs));
                }
                Ok(RValue::Vector(rv))
            }

            CPLXSXP => {
                let len = self.read_length()?;
                let mut values: Vec<Option<num_complex::Complex64>> = Vec::with_capacity(len);
                for _ in 0..len {
                    let re = self.read_double()?;
                    let im = self.read_double()?;
                    if re.to_bits() == R_NA_REAL_BITS || im.to_bits() == R_NA_REAL_BITS {
                        values.push(None);
                    } else {
                        values.push(Some(num_complex::Complex64::new(re, im)));
                    }
                }
                let mut rv = RVector::from(Vector::Complex(values.into()));
                if has_attr {
                    let attrs = self.read_attributes()?;
                    rv.attrs = Some(Box::new(attrs));
                }
                Ok(RValue::Vector(rv))
            }

            STRSXP => {
                let len = self.read_length()?;
                let mut values: Vec<Option<String>> = Vec::with_capacity(len);
                for _ in 0..len {
                    // Each element is a CHARSXP.
                    let (inner_type, _ia, _it, inner_flags) = self.read_flags()?;
                    if inner_type == CHARSXP {
                        values.push(self.read_charsxp_with_flags(inner_flags)?);
                    } else if inner_type == NILVALUE_SXP {
                        values.push(None);
                    } else {
                        return Err(RError::new(
                            RErrorKind::Other,
                            format!(
                                "expected CHARSXP in STRSXP element, got type {}",
                                inner_type
                            ),
                        ));
                    }
                }
                let mut rv = RVector::from(Vector::Character(values.into()));
                if has_attr {
                    let attrs = self.read_attributes()?;
                    rv.attrs = Some(Box::new(attrs));
                }
                Ok(RValue::Vector(rv))
            }

            RAWSXP => {
                let len = self.read_length()?;
                let bytes = self.read_bytes(len)?.to_vec();
                let mut rv = RVector::from(Vector::Raw(bytes));
                if has_attr {
                    let attrs = self.read_attributes()?;
                    rv.attrs = Some(Box::new(attrs));
                }
                Ok(RValue::Vector(rv))
            }

            VECSXP | EXPRSXP => {
                let len = self.read_length()?;
                let mut elements = Vec::with_capacity(len);
                for _ in 0..len {
                    let val = self.read_item()?;
                    elements.push((None, val));
                }
                let mut list = RList::new(elements);
                if has_attr {
                    let attrs = self.read_attributes()?;
                    // Extract "names" attribute and apply to list elements.
                    if let Some(names_val) = attrs.get("names") {
                        if let Some(names_vec) = names_val.as_vector() {
                            let names = names_vec.to_characters();
                            for (i, name) in names.iter().enumerate() {
                                if i < list.values.len() {
                                    list.values[i].0 = name.clone();
                                }
                            }
                        }
                    }
                    // Store remaining attributes (excluding names, which we consumed).
                    let mut remaining: Attributes =
                        attrs.into_iter().filter(|(k, _)| k != "names").collect();
                    if !remaining.is_empty() {
                        // Re-add names to attrs too — R keeps them there
                        if let Some(first_name) = list.values.first() {
                            if first_name.0.is_some() {
                                let names: Vec<Option<String>> =
                                    list.values.iter().map(|(n, _)| n.clone()).collect();
                                remaining.insert(
                                    "names".to_string(),
                                    RValue::vec(Vector::Character(names.into())),
                                );
                            }
                        }
                        list.attrs = Some(Box::new(remaining));
                    }
                }
                Ok(RValue::List(list))
            }

            LISTSXP => {
                // Pairlist: TAG (optional) + CAR + CDR chain.
                // Convert to a named list.
                self.read_pairlist_as_list(has_attr, has_tag, flags)
            }

            CLOSXP => {
                // Closure: environment + formals (pairlist) + body (LANGSXP).
                // We can't fully reconstruct closures, but we need to read past them
                // so the stream stays in sync.
                let _env = self.read_item()?; // environment
                let _formals = self.read_item()?; // formals pairlist
                let _body = self.read_item()?; // body
                let val = RValue::Null; // placeholder
                if has_attr {
                    let _attrs = self.read_attributes()?;
                }
                Ok(val)
            }

            LANGSXP => {
                // Language object: same structure as pairlist (TAG + CAR + CDR).
                // Read as a list for now.
                self.read_pairlist_as_list(has_attr, has_tag, flags)
            }

            // S4 object (type 25)
            25 => {
                // OBJSXP / S4: read attributes only.
                let attrs = if has_attr {
                    self.read_attributes()?
                } else {
                    IndexMap::new()
                };
                let mut list = RList::new(Vec::new());
                if !attrs.is_empty() {
                    list.attrs = Some(Box::new(attrs));
                }
                Ok(RValue::List(list))
            }

            _ => Err(RError::new(
                RErrorKind::Other,
                format!(
                    "unsupported SEXPTYPE {} at offset {} in RDS data",
                    sxp_type,
                    self.pos - 4
                ),
            )),
        }
    }

    /// Read a CHARSXP given that the flags have already been read.
    fn read_charsxp_with_flags(&mut self, _flags: u32) -> Result<Option<String>, RError> {
        self.read_charsxp()
    }

    /// Read an item and extract it as a symbol name (string).
    fn read_item_as_symbol(&mut self) -> Result<String, RError> {
        let val = self.read_item()?;
        match &val {
            RValue::Vector(rv) => match &rv.inner {
                Vector::Character(c) => Ok(c.first().and_then(|s| s.clone()).unwrap_or_default()),
                _ => Ok(String::new()),
            },
            _ => Ok(String::new()),
        }
    }

    /// Read a pairlist (LISTSXP chain) and convert to RList.
    fn read_pairlist_as_list(
        &mut self,
        has_attr: bool,
        has_tag: bool,
        _flags: u32,
    ) -> Result<RValue, RError> {
        let mut elements = Vec::new();

        // Read the first node's tag + car.
        let tag = if has_tag {
            Some(self.read_item_as_symbol()?)
        } else {
            None
        };
        let car = self.read_item()?;
        elements.push((tag, car));

        // Read CDR chain.
        loop {
            let (sxp_type, _has_attr_cdr, has_tag_cdr, _cdr_flags) = self.read_flags()?;
            match sxp_type {
                LISTSXP => {
                    let tag = if has_tag_cdr {
                        Some(self.read_item_as_symbol()?)
                    } else {
                        None
                    };
                    let car = self.read_item()?;
                    elements.push((tag, car));
                }
                NILVALUE_SXP => break,
                _ => {
                    // CDR is a non-pairlist value (unusual but valid).
                    // Read it and store as unnamed.
                    let val =
                        self.read_item_inner(sxp_type, _has_attr_cdr, has_tag_cdr, _cdr_flags)?;
                    elements.push((None, val));
                    break;
                }
            }
        }

        let mut list = RList::new(elements);
        if has_attr {
            let attrs = self.read_attributes()?;
            list.attrs = Some(Box::new(attrs));
        }
        Ok(RValue::List(list))
    }
}

// endregion

// region: public API

/// Deserialize an R object from XDR binary format bytes.
///
/// The `data` should start at the format byte ('X', 'A', or 'B').
pub fn unserialize_xdr(data: &[u8]) -> Result<RValue, RError> {
    if data.len() < 2 {
        return Err(RError::new(
            RErrorKind::Other,
            "RDS data too short".to_string(),
        ));
    }

    // Parse format header: format byte + newline
    let format_byte = data[0];
    if data[1] != b'\n' {
        return Err(RError::new(
            RErrorKind::Other,
            format!("expected newline after format byte, got 0x{:02x}", data[1]),
        ));
    }

    match format_byte {
        b'X' => {} // XDR binary — supported
        b'A' => {
            return Err(RError::new(
                RErrorKind::Other,
                "ASCII serialization format is not yet supported; only XDR binary (format 'X') is implemented".to_string(),
            ));
        }
        b'B' => {
            return Err(RError::new(
                RErrorKind::Other,
                "native binary serialization format is not yet supported; only XDR binary (format 'X') is implemented".to_string(),
            ));
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Other,
                format!("unknown serialization format byte: 0x{:02x}", format_byte),
            ));
        }
    }

    let mut reader = XdrReader::new(&data[2..]);

    // Read three version integers.
    let _version = reader.read_int()?;
    let _r_version_wrote = reader.read_int()?;
    let _r_version_min = reader.read_int()?;

    // Version 3 has an additional native encoding string.
    if _version == 3 {
        let _native_encoding = reader.read_item()?;
    }

    // Read the actual object.
    reader.read_item()
}

/// Check if bytes look like a GNU R binary RDS file.
///
/// Returns true if the data starts with 'X\n', 'A\n', 'B\n', or a gzip header.
pub fn is_binary_rds(data: &[u8]) -> bool {
    if data.len() < 2 {
        return false;
    }
    // Direct format headers
    if data[1] == b'\n' && matches!(data[0], b'X' | b'A' | b'B') {
        return true;
    }
    // Gzip magic number
    is_gzip_data(data)
}

/// Check for gzip magic number (0x1f 0x8b).
pub fn is_gzip_data(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b
}

/// Decompress gzip data and then deserialize.
#[cfg(feature = "compression")]
pub fn unserialize_rds(data: &[u8]) -> Result<RValue, RError> {
    if is_gzip_data(data) {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).map_err(|e| {
            RError::new(
                RErrorKind::Other,
                format!("failed to decompress gzip RDS data: {}", e),
            )
        })?;
        unserialize_xdr(&decompressed)
    } else {
        unserialize_xdr(data)
    }
}

/// Decompress gzip data and then deserialize (no-compression fallback).
#[cfg(not(feature = "compression"))]
pub fn unserialize_rds(data: &[u8]) -> Result<RValue, RError> {
    if is_gzip_data(data) {
        Err(RError::new(
            RErrorKind::Other,
            "RDS file is gzip-compressed but miniR was built without the 'compression' feature; \
             rebuild with `--features compression` to read compressed RDS files"
                .to_string(),
        ))
    } else {
        unserialize_xdr(data)
    }
}

// endregion

// region: XdrWriter

/// Cursor-based writer for big-endian (XDR) binary data.
struct XdrWriter {
    buf: Vec<u8>,
}

impl XdrWriter {
    fn new() -> Self {
        XdrWriter { buf: Vec::new() }
    }

    /// Write a big-endian i32.
    fn write_int(&mut self, val: i32) {
        self.buf.extend_from_slice(&val.to_be_bytes());
    }

    /// Write a big-endian f64.
    fn write_double(&mut self, val: f64) {
        self.buf.extend_from_slice(&val.to_be_bytes());
    }

    /// Write flags for an object: SEXPTYPE in bits 0:7, has-attr in bit 9, has-tag in bit 10.
    fn write_flags(&mut self, sxp_type: u8, has_attr: bool, has_tag: bool) {
        let mut flags: u32 = u32::from(sxp_type);
        if has_attr {
            flags |= HAS_ATTR_MASK;
        }
        if has_tag {
            flags |= HAS_TAG_MASK;
        }
        self.write_int(flags as i32);
    }

    /// Write a CHARSXP: flags + length + raw bytes. Pass `None` for NA_STRING.
    fn write_charsxp(&mut self, s: Option<&str>) {
        match s {
            Some(text) => {
                // CHARSXP flags: type 9, UTF-8 encoding (bit 12)
                let flags: u32 = u32::from(CHARSXP) | (1 << 12);
                self.write_int(flags as i32);
                let bytes = text.as_bytes();
                self.write_int(i32::try_from(bytes.len()).unwrap_or(i32::MAX));
                self.buf.extend_from_slice(bytes);
            }
            None => {
                // NA_STRING: CHARSXP with length -1
                let flags: u32 = u32::from(CHARSXP);
                self.write_int(flags as i32);
                self.write_int(-1);
            }
        }
    }

    /// Write NILVALUE_SXP sentinel (end of pairlists, etc.)
    fn write_nilvalue(&mut self) {
        self.write_flags(NILVALUE_SXP, false, false);
    }

    /// Write a length value. Uses the standard i32 encoding for lengths < 2^31.
    fn write_length(&mut self, len: usize) {
        if let Ok(n) = i32::try_from(len) {
            self.write_int(n);
        } else {
            // Long vector: write -1 sentinel, then upper/lower 32-bit halves.
            self.write_int(-1);
            let long_len = len as u64;
            self.write_int((long_len >> 32) as i32);
            self.write_int(long_len as i32);
        }
    }

    /// Write attributes as a pairlist. Each entry becomes a LISTSXP node with a
    /// SYMSXP tag and the value as CAR. The chain terminates with NILVALUE_SXP.
    fn write_attributes(&mut self, attrs: &Attributes) {
        for (name, value) in attrs {
            self.write_flags(LISTSXP, false, true); // has_tag = true
                                                    // Tag: SYMSXP containing a CHARSXP
            self.write_flags(SYMSXP, false, false);
            self.write_charsxp(Some(name));
            // Value: the attribute value
            self.write_item(value);
        }
        self.write_nilvalue();
    }

    /// Write a single R value recursively.
    fn write_item(&mut self, value: &RValue) {
        match value {
            RValue::Null => {
                self.write_flags(NILVALUE_SXP, false, false);
            }
            RValue::Vector(rv) => {
                let has_attr = rv.attrs.as_ref().is_some_and(|a| !a.is_empty());
                match &rv.inner {
                    Vector::Logical(vals) => {
                        self.write_flags(LGLSXP, has_attr, false);
                        self.write_length(vals.len());
                        for v in vals.iter() {
                            match v {
                                Some(true) => self.write_int(1),
                                Some(false) => self.write_int(0),
                                None => self.write_int(R_NA_LOGICAL),
                            }
                        }
                    }
                    Vector::Integer(vals) => {
                        self.write_flags(INTSXP, has_attr, false);
                        self.write_length(vals.len());
                        for v in vals.iter() {
                            match v {
                                Some(i) => {
                                    // R integers are i32; clamp to i32 range.
                                    let clamped = i32::try_from(*i).unwrap_or_else(|_| {
                                        if *i > i64::from(i32::MAX) {
                                            i32::MAX
                                        } else {
                                            // i32::MIN is NA, so use MIN + 1
                                            i32::MIN + 1
                                        }
                                    });
                                    // Guard against accidentally writing NA_INTEGER
                                    // for a non-NA value.
                                    if clamped == R_NA_INTEGER {
                                        self.write_int(R_NA_INTEGER + 1);
                                    } else {
                                        self.write_int(clamped);
                                    }
                                }
                                None => self.write_int(R_NA_INTEGER),
                            }
                        }
                    }
                    Vector::Double(vals) => {
                        self.write_flags(REALSXP, has_attr, false);
                        self.write_length(vals.len());
                        for v in vals.iter() {
                            match v {
                                Some(d) => self.write_double(*d),
                                None => self.buf.extend_from_slice(&R_NA_REAL_BITS.to_be_bytes()),
                            }
                        }
                    }
                    Vector::Complex(vals) => {
                        self.write_flags(CPLXSXP, has_attr, false);
                        self.write_length(vals.len());
                        for v in vals.iter() {
                            match v {
                                Some(c) => {
                                    self.write_double(c.re);
                                    self.write_double(c.im);
                                }
                                None => {
                                    self.buf.extend_from_slice(&R_NA_REAL_BITS.to_be_bytes());
                                    self.buf.extend_from_slice(&R_NA_REAL_BITS.to_be_bytes());
                                }
                            }
                        }
                    }
                    Vector::Character(vals) => {
                        self.write_flags(STRSXP, has_attr, false);
                        self.write_length(vals.len());
                        for v in vals.iter() {
                            self.write_charsxp(v.as_deref());
                        }
                    }
                    Vector::Raw(bytes) => {
                        self.write_flags(RAWSXP, has_attr, false);
                        self.write_length(bytes.len());
                        self.buf.extend_from_slice(bytes);
                    }
                }
                if has_attr {
                    self.write_attributes(rv.attrs.as_ref().unwrap());
                }
            }
            RValue::List(list) => {
                // Build the effective attributes: merge list names into attrs.
                let has_names = list.values.iter().any(|(name, _)| name.is_some());
                let mut effective_attrs: Attributes = list
                    .attrs
                    .as_ref()
                    .map(|a| a.as_ref().clone())
                    .unwrap_or_default();
                if has_names && !effective_attrs.contains_key("names") {
                    let names: Vec<Option<String>> =
                        list.values.iter().map(|(n, _)| n.clone()).collect();
                    effective_attrs.insert(
                        "names".to_string(),
                        RValue::vec(Vector::Character(names.into())),
                    );
                }
                let has_attr = !effective_attrs.is_empty();

                self.write_flags(VECSXP, has_attr, false);
                self.write_length(list.values.len());
                for (_, val) in &list.values {
                    self.write_item(val);
                }
                if has_attr {
                    self.write_attributes(&effective_attrs);
                }
            }
            // Functions, environments, and language objects cannot be serialized
            // in a meaningful way; write NULL as a placeholder.
            RValue::Function(_) | RValue::Environment(_) | RValue::Language(_) => {
                self.write_flags(NILVALUE_SXP, false, false);
            }
        }
    }

    /// Write a SYMSXP (symbol): flags + CHARSXP for the name.
    fn write_symbol(&mut self, name: &str) {
        self.write_flags(SYMSXP, false, false);
        self.write_charsxp(Some(name));
    }

    /// Write a pairlist: a chain of LISTSXP nodes, each with a TAG (symbol) and
    /// CAR (value), terminated by NILVALUE_SXP. This is the format used by GNU R's
    /// `save()` for writing workspace files (RDX2 format).
    fn write_pairlist(&mut self, bindings: &[(String, RValue)]) {
        for (name, value) in bindings {
            // Each node: LISTSXP with has_tag = true
            self.write_flags(LISTSXP, false, true);
            // TAG: symbol with the binding name
            self.write_symbol(name);
            // CAR: the value
            self.write_item(value);
        }
        // Terminate with NILVALUE_SXP
        self.write_nilvalue();
    }

    fn finish(self) -> Vec<u8> {
        self.buf
    }
}

// endregion

// region: serialize public API

/// Serialize an R value to XDR binary format (version 2).
///
/// Returns the complete byte stream including the "X\n" header, version triple,
/// and the recursively serialized object. The output is compatible with GNU R's
/// `readRDS()` / `unserialize()`.
pub fn serialize_xdr(value: &RValue) -> Vec<u8> {
    let mut w = XdrWriter::new();

    // Format header: "X\n"
    w.buf.extend_from_slice(b"X\n");

    // Version 2
    w.write_int(2);
    // R version that wrote: encode as 4.3.0 (0x00040300)
    w.write_int(0x00040300);
    // Minimum R version to read: 2.3.0 (0x00020300)
    w.write_int(0x00020300);

    // The actual object
    w.write_item(value);

    w.finish()
}

/// Serialize an R value to an RDS byte stream, optionally gzip-compressed.
///
/// When `compress` is true and the `compression` feature is enabled, the output
/// is gzip-compressed (matching GNU R's default `saveRDS(compress = TRUE)`).
#[cfg(feature = "compression")]
pub fn serialize_rds(value: &RValue, compress: bool) -> Vec<u8> {
    let raw = serialize_xdr(value);
    if compress {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        // write_all is infallible for a Vec<u8> backed encoder
        encoder.write_all(&raw).expect("gzip encoding failed");
        encoder.finish().expect("gzip finish failed")
    } else {
        raw
    }
}

/// Serialize an R value to an RDS byte stream (no-compression fallback).
#[cfg(not(feature = "compression"))]
pub fn serialize_rds(value: &RValue, _compress: bool) -> Vec<u8> {
    serialize_xdr(value)
}

/// Serialize named bindings to GNU R binary .RData format (RDX2).
///
/// Writes the "RDX2\n" header followed by the XDR serialization stream containing
/// a pairlist where each node has TAG=symbol(name) and CAR=value. When `compress`
/// is true, the entire output is gzip-compressed.
///
/// This is compatible with GNU R's `load()`.
#[cfg(feature = "compression")]
pub fn serialize_rdata(bindings: &[(String, RValue)], compress: bool) -> Vec<u8> {
    let mut w = XdrWriter::new();

    // RDX2 header
    w.buf.extend_from_slice(b"RDX2\n");

    // XDR format header: "X\n"
    w.buf.extend_from_slice(b"X\n");

    // Version 2
    w.write_int(2);
    // R version that wrote: 4.3.0 (0x00040300)
    w.write_int(0x00040300);
    // Minimum R version to read: 2.3.0 (0x00020300)
    w.write_int(0x00020300);

    // Write the pairlist of bindings
    w.write_pairlist(bindings);

    let raw = w.finish();

    if compress {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).expect("gzip encoding failed");
        encoder.finish().expect("gzip finish failed")
    } else {
        raw
    }
}

/// Serialize named bindings to GNU R binary .RData format (no-compression fallback).
#[cfg(not(feature = "compression"))]
pub fn serialize_rdata(bindings: &[(String, RValue)], _compress: bool) -> Vec<u8> {
    let mut w = XdrWriter::new();

    // RDX2 header
    w.buf.extend_from_slice(b"RDX2\n");

    // XDR format header: "X\n"
    w.buf.extend_from_slice(b"X\n");

    // Version 2
    w.write_int(2);
    // R version that wrote: 4.3.0 (0x00040300)
    w.write_int(0x00040300);
    // Minimum R version to read: 2.3.0 (0x00020300)
    w.write_int(0x00020300);

    // Write the pairlist of bindings
    w.write_pairlist(bindings);

    w.finish()
}

// endregion

#[cfg(test)]
mod tests {
    use super::*;

    fn build_rds_header() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"X\n");
        buf.extend_from_slice(&2i32.to_be_bytes());
        buf.extend_from_slice(&0x00040300i32.to_be_bytes());
        buf.extend_from_slice(&0x00020300i32.to_be_bytes());
        buf
    }

    fn write_flags(buf: &mut Vec<u8>, sxp_type: u8, has_attr: bool, has_tag: bool) {
        let mut flags: u32 = u32::from(sxp_type);
        if has_attr {
            flags |= 1 << 9;
        }
        if has_tag {
            flags |= 1 << 10;
        }
        buf.extend_from_slice(&(flags as i32).to_be_bytes());
    }

    fn write_charsxp(buf: &mut Vec<u8>, s: &str) {
        let flags: u32 = 9 | (1 << 12);
        buf.extend_from_slice(&(flags as i32).to_be_bytes());
        buf.extend_from_slice(&(s.len() as i32).to_be_bytes());
        buf.extend_from_slice(s.as_bytes());
    }

    fn write_nilvalue(buf: &mut Vec<u8>) {
        write_flags(buf, 254, false, false);
    }

    #[test]
    fn unit_test_named_int_vec() {
        let mut buf = build_rds_header();

        // INTSXP (13) with has_attr
        write_flags(&mut buf, 13, true, false);
        buf.extend_from_slice(&3i32.to_be_bytes());
        buf.extend_from_slice(&10i32.to_be_bytes());
        buf.extend_from_slice(&20i32.to_be_bytes());
        buf.extend_from_slice(&30i32.to_be_bytes());

        // Attributes pairlist: LISTSXP with has_tag
        write_flags(&mut buf, 2, false, true);
        // Tag: SYMSXP
        write_flags(&mut buf, 1, false, false);
        write_charsxp(&mut buf, "names");
        // Value: STRSXP c("a", "b", "c")
        write_flags(&mut buf, 16, false, false);
        buf.extend_from_slice(&3i32.to_be_bytes());
        write_charsxp(&mut buf, "a");
        write_charsxp(&mut buf, "b");
        write_charsxp(&mut buf, "c");
        // NILVALUE
        write_nilvalue(&mut buf);

        let result = unserialize_xdr(&buf).unwrap();
        match &result {
            RValue::Vector(rv) => {
                assert!(
                    matches!(&rv.inner, Vector::Integer(_)),
                    "expected integer vector, got {:?}",
                    rv.inner
                );
                let names = rv.get_attr("names");
                assert!(
                    names.is_some(),
                    "expected names attribute, attrs: {:?}",
                    rv.attrs
                );
            }
            other => panic!("expected Vector, got {:?}", other),
        }
    }

    #[test]
    fn unit_test_simple_int_vec() {
        let mut buf = build_rds_header();

        write_flags(&mut buf, 13, false, false);
        buf.extend_from_slice(&3i32.to_be_bytes());
        buf.extend_from_slice(&1i32.to_be_bytes());
        buf.extend_from_slice(&2i32.to_be_bytes());
        buf.extend_from_slice(&3i32.to_be_bytes());

        let result = unserialize_xdr(&buf).unwrap();
        match &result {
            RValue::Vector(rv) => {
                assert!(matches!(&rv.inner, Vector::Integer(_)));
                if let Vector::Integer(ints) = &rv.inner {
                    assert_eq!(ints.len(), 3);
                    assert_eq!(ints[0], Some(1));
                    assert_eq!(ints[1], Some(2));
                    assert_eq!(ints[2], Some(3));
                }
            }
            other => panic!("expected Vector, got {:?}", other),
        }
    }
}
