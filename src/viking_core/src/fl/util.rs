use std::borrow::Cow;

use cu::pre::*;

use crate::config::Config;

pub fn parse_abs_address(config: &Config, hex_value: &str) -> cu::Result<u64> {
    Ok(parse_abs_address_as_offset(config, hex_value)? as u64 + config.repo.address_base)
}

/// Parse an absolute address (offset + base address) to just the offset.
pub fn parse_abs_address_as_offset(config: &Config, hex_value: &str) -> cu::Result<u32> {
    let hex_value = hex_value.strip_prefix("0x").unwrap_or(hex_value);
    let hex = cu::check!(
        u64::from_str_radix(hex_value, 16),
        "invalid address: address must be in hex"
    )?;
    let base = config.repo.address_base;
    let offset = cu::check!(
        hex.checked_sub(base),
        "invalid address: address must be greater than base (0x{base:x}), got 0x{hex:x}"
    )?;
    cu::ensure!(
        offset <= u32::MAX as u64,
        "invalid address: address is too big (0x{offset:x}), this is unlikely to be correct."
    )?;
    Ok(offset as u32)
}

/// Parse an address as the offset (without the base address)
pub fn parse_offset(hex_value: &str) -> cu::Result<u32> {
    let hex_value = hex_value.strip_prefix("0x").unwrap_or(hex_value);
    cu::check!(
        u32::from_str_radix(hex_value, 16),
        "invalid address: address must be in hex"
    )
}

pub fn quote_yaml_string_if_needed(s: &str) -> Cow<'_, str> {
    if is_safe_yaml_string(s) {
        return s.into();
    }
    format!("\'{}\'", s.replace('\'', "''")).into()
}

/// If the string is parsed back as the same string when written as a plain scalar,
/// including in flow context
fn is_safe_yaml_string(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$' | '/' | '-')) {
        return false;
    }
    // words that could be parsed as non-string
    !matches!(
        s.to_ascii_lowercase().as_str(),
        "null" | "true" | "false" | "yes" | "no" | "on" | "off" | "y" | "n"
    )
}
