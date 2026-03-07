use std::collections::BTreeSet;

use crate::register::{RegisterClient, RegisterError};
use crate::yaml::{DeviceConfig, DeviceRegisterValue, FeatureFieldDefinition};
use crate::{ControlError, ControlErrorKind};

const STREAM_REGISTER_NAMES: &[&str] = &[
    "MaxPacketSize",
    "Delay",
    "DestIPAddr",
    "DestPort",
    "PixelsPerLine",
    "LinesPerFrame",
    "PixelFormat",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterSelector {
    Name(String),
    Address(u32),
}

impl RegisterSelector {
    pub fn name(name: impl Into<String>) -> Self {
        Self::Name(name.into())
    }

    pub fn address(address: u32) -> Self {
        Self::Address(address)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterValue {
    U32(u32),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldUpdate {
    pub name: String,
    pub value: u32,
}

impl FieldUpdate {
    pub fn new(name: impl Into<String>, value: u32) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

pub fn register_name(prefix: &str, suffix: &str) -> String {
    format!("{prefix}_{suffix}")
}

pub fn stream_prefixes(device: &DeviceConfig) -> Vec<String> {
    let mut prefixes = BTreeSet::new();
    for register_name in device.registers.keys() {
        if let Some((prefix, suffix)) = register_name.split_once('_') {
            if STREAM_REGISTER_NAMES.contains(&suffix) {
                prefixes.insert(prefix.to_string());
            }
        }
    }
    prefixes.into_iter().collect()
}

pub fn resolve_stream_prefix(
    device: &DeviceConfig,
    requested_stream_name: &str,
) -> Result<String, ControlError> {
    let prefixes = stream_prefixes(device);
    if prefixes.is_empty() {
        return Err(ControlError::new(
            ControlErrorKind::InvalidConfiguration,
            "device does not expose a configurable stream register block",
        ));
    }

    if prefixes.iter().any(|prefix| prefix == requested_stream_name) {
        return Ok(requested_stream_name.to_string());
    }

    if prefixes.len() == 1 {
        return Ok(prefixes[0].clone());
    }

    Err(ControlError::new(
        ControlErrorKind::InvalidConfiguration,
        format!(
            "requested stream {requested_stream_name} does not match any available device stream prefix"
        ),
    ))
}

pub fn read_register_value(
    client: &RegisterClient,
    device: &DeviceConfig,
    selector: &RegisterSelector,
) -> Result<RegisterValue, ControlError> {
    match resolve_register(device, selector)? {
        Some((_name, register)) if register.str_value.is_some() => client
            .read_string(register.addr)
            .map(RegisterValue::String)
            .map_err(register_error),
        Some((_name, register)) => client
            .read_u32(register.addr)
            .map(RegisterValue::U32)
            .map_err(register_error),
        None => match selector {
            RegisterSelector::Address(address) => client
                .read_u32(*address)
                .map(RegisterValue::U32)
                .map_err(register_error),
            RegisterSelector::Name(name) => Err(unknown_register(name)),
        },
    }
}

pub fn read_register_field(
    client: &RegisterClient,
    device: &DeviceConfig,
    selector: &RegisterSelector,
    field_name: &str,
) -> Result<u32, ControlError> {
    let (register_name, register) = resolve_register(device, selector)?
        .ok_or_else(|| match selector {
            RegisterSelector::Name(name) => unknown_register(name),
            RegisterSelector::Address(address) => ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                format!("device does not expose register address 0x{address:08x}"),
            ),
        })?;
    let value = client.read_u32(register.addr).map_err(register_error)?;
    let definition = register.fields.get(field_name).ok_or_else(|| {
        ControlError::new(
            ControlErrorKind::InvalidConfiguration,
            format!("register {register_name} does not define field {field_name}"),
        )
    })?;
    extract_field(value, definition)
}

pub fn write_register_u32(
    client: &RegisterClient,
    device: &DeviceConfig,
    selector: &RegisterSelector,
    value: u32,
) -> Result<(), ControlError> {
    let (register_name, address) = match resolve_register(device, selector)? {
        Some((register_name, register)) => (register_name, register.addr),
        None => match selector {
            RegisterSelector::Address(address) => (format!("0x{address:08x}"), *address),
            RegisterSelector::Name(name) => return Err(unknown_register(name)),
        },
    };

    client.write_u32(address, value).map_err(register_error)?;

    let applied = client.read_u32(address).map_err(register_error)?;
    if applied != value {
        return Err(ControlError::new(
            ControlErrorKind::AppliedValueMismatch,
            format!(
                "device applied 0x{applied:08x} for register {register_name}, expected 0x{value:08x}"
            ),
        ));
    }

    Ok(())
}

pub fn write_register_fields(
    client: &RegisterClient,
    device: &DeviceConfig,
    selector: &RegisterSelector,
    fields: &[FieldUpdate],
) -> Result<(), ControlError> {
    let (register_name, register) = resolve_register(device, selector)?
        .ok_or_else(|| match selector {
            RegisterSelector::Name(name) => unknown_register(name),
            RegisterSelector::Address(address) => ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                format!("device does not expose register address 0x{address:08x}"),
            ),
        })?;
    if fields.is_empty() {
        return Err(ControlError::new(
            ControlErrorKind::InvalidConfiguration,
            format!("no register fields provided for {register_name}"),
        ));
    }

    let mut write_value = 0u32;
    let mut mask_value = 0u32;
    for field in fields {
        let definition = register.fields.get(&field.name).ok_or_else(|| {
            ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                format!("register {register_name} does not define field {}", field.name),
            )
        })?;
        let (shift, field_mask) = field_mask(definition)?;
        if (field.value & !field_mask) != 0 {
            return Err(ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                format!(
                    "field {} value 0x{:x} exceeds the bit width of register {register_name}",
                    field.name, field.value
                ),
            ));
        }
        write_value |= field.value << shift;
        mask_value |= field_mask << shift;
    }

    if register.fields.len() != fields.len() {
        let current = client.read_u32(register.addr).map_err(register_error)?;
        write_value = (current & !mask_value) | (write_value & mask_value);
    }

    client
        .write_u32(register.addr, write_value)
        .map_err(register_error)?;

    let applied = client.read_u32(register.addr).map_err(register_error)?;
    for field in fields {
        let definition = register
            .fields
            .get(&field.name)
            .expect("field looked up above");
        if extract_field(applied, definition)? != field.value {
            return Err(ControlError::new(
                ControlErrorKind::AppliedValueMismatch,
                format!(
                    "device did not apply field {} on register {register_name} as requested",
                    field.name
                ),
            ));
        }
    }

    Ok(())
}

pub fn extract_field(
    value: u32,
    definition: &FeatureFieldDefinition,
) -> Result<u32, ControlError> {
    let (shift, mask) = field_mask(definition)?;
    Ok((value >> shift) & mask)
}

pub fn field_mask(
    definition: &FeatureFieldDefinition,
) -> Result<(u32, u32), ControlError> {
    if definition.len == 0 || definition.len > 32 || definition.msb + 1 < definition.len {
        return Err(ControlError::new(
            ControlErrorKind::InvalidConfiguration,
            format!(
                "invalid field definition with msb {} and len {}",
                definition.msb, definition.len
            ),
        ));
    }

    let shift = definition.msb + 1 - definition.len;
    let mask = if definition.len == 32 {
        u32::MAX
    } else {
        (1u32 << definition.len) - 1
    };
    Ok((shift, mask))
}

pub(crate) fn register_error(error: RegisterError) -> ControlError {
    match error {
        RegisterError::Io(err) => {
            let kind = if err.kind() == std::io::ErrorKind::TimedOut {
                ControlErrorKind::Timeout
            } else {
                ControlErrorKind::Connection
            };
            ControlError::new(kind, err.to_string())
        }
        RegisterError::Coap(err) => {
            ControlError::new(ControlErrorKind::Connection, err.to_string())
        }
        RegisterError::InvalidAccess(message)
        | RegisterError::UnknownRegister(message)
        | RegisterError::Response(message) => {
            ControlError::new(ControlErrorKind::InvalidConfiguration, message)
        }
        RegisterError::InvalidString(err) => {
            ControlError::new(ControlErrorKind::Connection, err.to_string())
        }
    }
}

pub(crate) fn unknown_register(name: &str) -> ControlError {
    ControlError::new(
        ControlErrorKind::InvalidConfiguration,
        format!("device does not expose register {name}"),
    )
}

fn resolve_register<'a>(
    device: &'a DeviceConfig,
    selector: &RegisterSelector,
) -> Result<Option<(String, &'a DeviceRegisterValue)>, ControlError> {
    Ok(match selector {
        RegisterSelector::Name(name) => Some((
            name.clone(),
            device
                .registers
                .get(name)
                .ok_or_else(|| unknown_register(name))?,
        )),
        RegisterSelector::Address(address) => device
            .registers
            .iter()
            .find(|(_name, register)| register.addr == *address)
            .map(|(name, register)| (name.clone(), register)),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::yaml::{
        DeviceCapabilities, DeviceConfig, DeviceLocation, DeviceMemoryMap, DeviceRegisterValue,
    };

    use super::{extract_field, field_mask, resolve_stream_prefix, stream_prefixes, RegisterSelector};

    fn device() -> DeviceConfig {
        DeviceConfig {
            location: DeviceLocation::default(),
            capabilities: DeviceCapabilities::default(),
            memory_map: DeviceMemoryMap::default(),
            registers: BTreeMap::from([
                (
                    "stream0_DestPort".to_string(),
                    DeviceRegisterValue {
                        addr: 0x40014,
                        access: "rw".to_string(),
                        int_value: Some(0),
                        str_value: None,
                        fields: BTreeMap::new(),
                    },
                ),
                (
                    "stream0_MaxPacketSize".to_string(),
                    DeviceRegisterValue {
                        addr: 0x40000,
                        access: "rw".to_string(),
                        int_value: Some(1200),
                        str_value: None,
                        fields: BTreeMap::new(),
                    },
                ),
            ]),
        }
    }

    #[test]
    fn stream_prefixes_collect_unique_names() {
        assert_eq!(stream_prefixes(&device()), vec!["stream0".to_string()]);
    }

    #[test]
    fn resolve_stream_prefix_accepts_single_stream_devices() {
        assert_eq!(resolve_stream_prefix(&device(), "other").unwrap(), "stream0");
    }

    #[test]
    fn field_mask_rejects_invalid_lengths() {
        let error = field_mask(&crate::FeatureFieldDefinition { msb: 0, len: 2 }).unwrap_err();
        assert_eq!(error.kind(), crate::ControlErrorKind::InvalidConfiguration);
    }

    #[test]
    fn extract_field_uses_msb_and_len() {
        let value = extract_field(
            0b1011_0000,
            &crate::FeatureFieldDefinition { msb: 7, len: 4 },
        )
        .unwrap();
        assert_eq!(value, 0b1011);
    }

    #[test]
    fn selector_address_helper_is_stable() {
        assert_eq!(RegisterSelector::address(0x10), RegisterSelector::Address(0x10));
    }
}
