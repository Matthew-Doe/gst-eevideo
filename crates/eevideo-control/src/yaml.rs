use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

const EMBEDDED_FEATURES_YAML: &str = include_str!("../yaml/EEVideo_Features.yaml");

pub type FeatureCatalog = BTreeMap<u32, FeatureDefinition>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFieldDefinition {
    pub msb: u32,
    pub len: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureRegisterDefinition {
    pub offset: u32,
    pub name: String,
    #[serde(default, rename = "acc", skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, FeatureFieldDefinition>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeaturePointerDefinition {
    pub index: u32,
    pub name: String,
    pub registers: Vec<FeatureRegisterDefinition>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureDefinition {
    pub name: String,
    #[serde(rename = "sname")]
    pub short_name: String,
    pub pointers: Vec<FeaturePointerDefinition>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    #[serde(default, rename = "decAvail")]
    pub dec_avail: bool,
    #[serde(default, rename = "multAddr")]
    pub mult_addr: bool,
    #[serde(default, rename = "stringRd")]
    pub string_rd: bool,
    #[serde(default, rename = "fifoRd")]
    pub fifo_rd: bool,
    #[serde(default, rename = "readRst")]
    pub read_rst: bool,
    #[serde(default, rename = "maskWr")]
    pub mask_wr: bool,
    #[serde(default, rename = "bitTog")]
    pub bit_tog: bool,
    #[serde(default, rename = "bitSet")]
    pub bit_set: bool,
    #[serde(default, rename = "bitClear")]
    pub bit_clear: bool,
    #[serde(default, rename = "staticIP")]
    pub static_ip: bool,
    #[serde(default, rename = "linkLocIP")]
    pub link_local_ip: bool,
    #[serde(default, rename = "dhcpIP")]
    pub dhcp_ip: bool,
    #[serde(default, rename = "multiDisc")]
    pub multi_disc: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceLocation {
    #[serde(rename = "ifName")]
    pub interface_name: String,
    #[serde(rename = "ifIP")]
    pub interface_address: String,
    #[serde(rename = "devIP")]
    pub device_address: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceMemoryMap {
    #[serde(default, rename = "lastStatic")]
    pub last_static: u32,
    #[serde(default, rename = "firstMutable")]
    pub first_mutable: u32,
    #[serde(default, rename = "lastMutable")]
    pub last_mutable: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRegisterValue {
    pub addr: u32,
    #[serde(rename = "acc")]
    pub access: String,
    #[serde(default, rename = "intval", skip_serializing_if = "Option::is_none")]
    pub int_value: Option<u64>,
    #[serde(default, rename = "strval", skip_serializing_if = "Option::is_none")]
    pub str_value: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, FeatureFieldDefinition>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub location: DeviceLocation,
    #[serde(default)]
    pub capabilities: DeviceCapabilities,
    #[serde(default, rename = "map")]
    pub memory_map: DeviceMemoryMap,
    #[serde(rename = "features")]
    pub registers: BTreeMap<String, DeviceRegisterValue>,
}

pub fn load_embedded_feature_catalog() -> Result<FeatureCatalog, YamlError> {
    parse_feature_catalog(EMBEDDED_FEATURES_YAML)
}

pub fn parse_feature_catalog(source: &str) -> Result<FeatureCatalog, YamlError> {
    let raw: BTreeMap<String, FeatureDefinition> = serde_yaml::from_str(source).map_err(YamlError::Parse)?;
    let mut features = BTreeMap::new();
    for (key, definition) in raw {
        let trimmed = key.trim_start_matches("0x");
        let feature_id = u32::from_str_radix(trimmed, 16)
            .map_err(|_| YamlError::InvalidFeatureId(key.clone()))?;
        features.insert(feature_id, definition);
    }
    Ok(features)
}

pub fn read_device_config(path: impl AsRef<Path>) -> Result<DeviceConfig, YamlError> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).map_err(YamlError::Io)?;
    let yaml_path = if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .map_err(YamlError::Io)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("yaml")))
            .collect::<Vec<_>>();
        entries.sort();
        match entries.as_slice() {
            [single] => single.clone(),
            [] => return Err(YamlError::NoYamlInDirectory(path.display().to_string())),
            _ => return Err(YamlError::MultipleYamlFiles(path.display().to_string())),
        }
    } else {
        path.to_path_buf()
    };

    let source = fs::read_to_string(&yaml_path).map_err(YamlError::Io)?;
    serde_yaml::from_str(&source).map_err(YamlError::Parse)
}

pub fn write_device_config(
    path: impl AsRef<Path>,
    config: &DeviceConfig,
) -> Result<(), YamlError> {
    fs::write(path, device_config_to_string(config)?).map_err(YamlError::Io)
}

pub fn device_config_to_string(config: &DeviceConfig) -> Result<String, YamlError> {
    serde_yaml::to_string(config).map_err(YamlError::Serialize)
}

#[derive(Debug)]
pub enum YamlError {
    Io(std::io::Error),
    Parse(serde_yaml::Error),
    Serialize(serde_yaml::Error),
    InvalidFeatureId(String),
    NoYamlInDirectory(String),
    MultipleYamlFiles(String),
}

impl std::fmt::Display for YamlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => err.fmt(f),
            Self::Parse(err) => err.fmt(f),
            Self::Serialize(err) => err.fmt(f),
            Self::InvalidFeatureId(key) => write!(f, "invalid feature id key {key}"),
            Self::NoYamlInDirectory(path) => write!(f, "no YAML file found in directory {path}"),
            Self::MultipleYamlFiles(path) => {
                write!(f, "multiple YAML files found in directory {path}")
            }
        }
    }
}

impl std::error::Error for YamlError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        device_config_to_string, load_embedded_feature_catalog, read_device_config,
        DeviceCapabilities, DeviceConfig, DeviceLocation, DeviceMemoryMap, DeviceRegisterValue,
        FeatureFieldDefinition,
    };

    #[test]
    fn embedded_feature_catalog_contains_stream_definition() {
        let catalog = load_embedded_feature_catalog().unwrap();
        let stream = catalog.get(&0x103001).expect("video stream feature present");
        assert_eq!(stream.short_name, "stream");
    }

    #[test]
    fn device_config_round_trips_to_yaml() {
        let mut registers = BTreeMap::new();
        registers.insert(
            "stream0_DestPort".to_string(),
            DeviceRegisterValue {
                addr: 0x40000,
                access: "rw".to_string(),
                int_value: Some(5000),
                str_value: None,
                fields: BTreeMap::from([(
                    "dPort".to_string(),
                    FeatureFieldDefinition { msb: 15, len: 16 },
                )]),
            },
        );

        let config = DeviceConfig {
            location: DeviceLocation {
                interface_name: "eth0".to_string(),
                interface_address: "192.168.1.10".to_string(),
                device_address: "192.168.1.20".to_string(),
            },
            capabilities: DeviceCapabilities::default(),
            memory_map: DeviceMemoryMap::default(),
            registers,
        };

        let yaml = device_config_to_string(&config).unwrap();
        let parsed: DeviceConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn read_device_config_accepts_single_yaml_in_directory() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("device.yaml");
        std::fs::write(
            &path,
            r#"
location:
  ifName: eth0
  ifIP: 192.168.1.10
  devIP: 192.168.1.20
capabilities: {}
map: {}
features:
  stream0_DestPort:
    addr: 262144
    acc: rw
"#,
        )
        .unwrap();

        let config = read_device_config(temp.path()).unwrap();
        assert_eq!(config.location.device_address, "192.168.1.20");
    }
}
