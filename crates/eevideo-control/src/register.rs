use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use crate::coap::{
    response_code_description, CoapError, CoapMessage, CoapMessageType, CoapOption, CODE_CHANGED,
    CODE_CONTENT, CODE_GET, CODE_PUT, OPTION_EEV_BINARY_ADDRESS, OPTION_EEV_REG_ACCESS,
};
use crate::yaml::DeviceConfig;

static NEXT_MESSAGE_ID: AtomicU16 = AtomicU16::new(0x2000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterReadKind {
    Register = 0,
    Fifo = 1,
    RegisterIncrement = 4,
    String = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterWriteKind {
    Write = 0,
    Set = 1,
    Clear = 2,
    Toggle = 3,
    WriteIncrement = 5,
    MaskWriteIncrement = 6,
    ReadSetAdvanceIncrement = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegisterAccess {
    pub insert: bool,
    pub count: u8,
    pub access_type: u8,
}

impl RegisterAccess {
    pub fn read(kind: RegisterReadKind, count: u8) -> Self {
        Self {
            insert: kind != RegisterReadKind::Register,
            count,
            access_type: kind as u8,
        }
    }

    pub fn write(kind: RegisterWriteKind, count: u8) -> Self {
        Self {
            insert: false,
            count,
            access_type: kind as u8,
        }
    }

    pub fn option_value(self) -> Result<Option<u8>, RegisterError> {
        if self.count > 31 {
            return Err(RegisterError::InvalidAccess(format!(
                "count must be 0-31, got {}",
                self.count
            )));
        }
        if !self.insert {
            return Ok(None);
        }
        Ok(Some(self.count | (self.access_type << 5)))
    }
}

#[derive(Clone, Debug)]
pub struct RegisterClient {
    local_bind: SocketAddr,
    device_addr: SocketAddr,
    timeout: Duration,
    token_len: u8,
}

impl RegisterClient {
    pub fn new(local_bind: SocketAddr, device_addr: SocketAddr) -> Self {
        Self {
            local_bind,
            device_addr,
            timeout: Duration::from_millis(1000),
            token_len: 1,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_token_len(mut self, token_len: u8) -> Self {
        self.token_len = token_len.min(8);
        self
    }

    pub fn read_u32(&self, address: u32) -> Result<u32, RegisterError> {
        let payload = self.execute(address, None, RegisterAccess::read(RegisterReadKind::Register, 1))?;
        if payload.len() < 4 {
            return Err(RegisterError::Response(format!(
                "u32 register read requires 4 bytes, got {}",
                payload.len()
            )));
        }
        Ok(u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]))
    }

    pub fn write_u32(&self, address: u32, value: u32) -> Result<(), RegisterError> {
        self.execute(
            address,
            Some(&value.to_be_bytes()),
            RegisterAccess::write(RegisterWriteKind::Write, 1),
        )?;
        Ok(())
    }

    pub fn read_string(&self, address: u32) -> Result<String, RegisterError> {
        let payload = self.execute(address, None, RegisterAccess::read(RegisterReadKind::String, 1))?;
        let trimmed = payload.split(|byte| *byte == 0).next().unwrap_or(&[]);
        Ok(String::from_utf8(trimmed.to_vec()).map_err(RegisterError::InvalidString)?)
    }

    pub fn read_named_u32(&self, device: &DeviceConfig, register_name: &str) -> Result<u32, RegisterError> {
        let register = device
            .registers
            .get(register_name)
            .ok_or_else(|| RegisterError::UnknownRegister(register_name.to_string()))?;
        self.read_u32(register.addr)
    }

    pub fn write_named_u32(
        &self,
        device: &DeviceConfig,
        register_name: &str,
        value: u32,
    ) -> Result<(), RegisterError> {
        let register = device
            .registers
            .get(register_name)
            .ok_or_else(|| RegisterError::UnknownRegister(register_name.to_string()))?;
        self.write_u32(register.addr, value)
    }

    pub fn execute(
        &self,
        address: u32,
        payload: Option<&[u8]>,
        access: RegisterAccess,
    ) -> Result<Vec<u8>, RegisterError> {
        let socket = UdpSocket::bind(self.local_bind).map_err(RegisterError::Io)?;
        socket
            .set_read_timeout(Some(self.timeout))
            .map_err(RegisterError::Io)?;
        socket
            .set_write_timeout(Some(self.timeout))
            .map_err(RegisterError::Io)?;

        let message_id = NEXT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed);
        let token = build_token(self.token_len, message_id);
        let mut options = Vec::new();
        if let Some(option_value) = access.option_value()? {
            options.push(CoapOption::new(OPTION_EEV_REG_ACCESS, [option_value]));
        }
        options.push(CoapOption::new(OPTION_EEV_BINARY_ADDRESS, address.to_be_bytes()));

        let request = CoapMessage::new(
            CoapMessageType::Confirmable,
            if payload.is_some() { CODE_PUT } else { CODE_GET },
            message_id,
            token.clone(),
            options,
            payload.unwrap_or(&[]).to_vec(),
        );
        let bytes = request.encode().map_err(RegisterError::Coap)?;
        socket.send_to(&bytes, self.device_addr).map_err(RegisterError::Io)?;

        let mut buffer = [0u8; 2048];
        let (size, _) = socket.recv_from(&mut buffer).map_err(RegisterError::Io)?;
        let response = CoapMessage::decode(&buffer[..size]).map_err(RegisterError::Coap)?;

        if response.message_type != CoapMessageType::Acknowledgement {
            return Err(RegisterError::Response(format!(
                "unexpected response type {:?}",
                response.message_type
            )));
        }
        if response.message_id != message_id {
            return Err(RegisterError::Response(format!(
                "response message id mismatch: expected {message_id}, got {}",
                response.message_id
            )));
        }
        if response.token != token {
            return Err(RegisterError::Response("response token mismatch".to_string()));
        }
        if response.code != CODE_CHANGED && response.code != CODE_CONTENT {
            let description = response_code_description(response.code)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{}.{:02}", response.code >> 5, response.code & 0x1F));
            return Err(RegisterError::Response(format!(
                "unexpected response code {description}"
            )));
        }

        Ok(response.payload)
    }
}

fn build_token(token_len: u8, message_id: u16) -> Vec<u8> {
    if token_len == 0 {
        return Vec::new();
    }

    let mut token = Vec::with_capacity(token_len as usize);
    let mut seed = message_id.to_be_bytes().to_vec();
    while token.len() < token_len as usize {
        token.extend_from_slice(&seed);
        seed.rotate_left(1);
    }
    token.truncate(token_len as usize);
    token
}

#[derive(Debug)]
pub enum RegisterError {
    Io(std::io::Error),
    Coap(CoapError),
    InvalidAccess(String),
    UnknownRegister(String),
    InvalidString(std::string::FromUtf8Error),
    Response(String),
}

impl std::fmt::Display for RegisterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => err.fmt(f),
            Self::Coap(err) => err.fmt(f),
            Self::InvalidAccess(message) => f.write_str(message),
            Self::UnknownRegister(name) => write!(f, "unknown register {name}"),
            Self::InvalidString(err) => err.fmt(f),
            Self::Response(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for RegisterError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::net::{SocketAddr, UdpSocket};
    use std::thread;
    use std::time::Duration;

    use crate::coap::{CoapMessage, CoapMessageType, CoapOption, CODE_CONTENT, OPTION_EEV_BINARY_ADDRESS};
    use crate::yaml::{DeviceCapabilities, DeviceConfig, DeviceLocation, DeviceMemoryMap, DeviceRegisterValue};

    use super::{RegisterAccess, RegisterClient, RegisterReadKind};

    fn spawn_register_server(payload: Vec<u8>) -> SocketAddr {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();

        thread::spawn(move || {
            let mut buffer = [0u8; 2048];
            let (size, peer) = server.recv_from(&mut buffer).unwrap();
            let request = CoapMessage::decode(&buffer[..size]).unwrap();
            assert_eq!(request.message_type, CoapMessageType::Confirmable);
            assert_eq!(request.options[0].number, OPTION_EEV_BINARY_ADDRESS);

            let response = CoapMessage::new(
                CoapMessageType::Acknowledgement,
                CODE_CONTENT,
                request.message_id,
                request.token,
                Vec::<CoapOption>::new(),
                payload,
            );
            let bytes = response.encode().unwrap();
            server.send_to(&bytes, peer).unwrap();
        });

        addr
    }

    #[test]
    fn register_access_option_matches_upstream_packing() {
        let access = RegisterAccess::read(RegisterReadKind::String, 1);
        assert_eq!(access.option_value().unwrap(), Some(0b1010_0001));
    }

    #[test]
    fn read_u32_round_trips_with_udp_responder() {
        let addr = spawn_register_server(vec![0x12, 0x34, 0x56, 0x78]);
        let client = RegisterClient::new("127.0.0.1:0".parse().unwrap(), addr)
            .with_timeout(Duration::from_millis(250));

        assert_eq!(client.read_u32(0x1000).unwrap(), 0x1234_5678);
    }

    #[test]
    fn read_named_register_uses_device_yaml_mapping() {
        let addr = spawn_register_server(vec![0, 0, 0, 9]);
        let client = RegisterClient::new("127.0.0.1:0".parse().unwrap(), addr)
            .with_timeout(Duration::from_millis(250));
        let mut registers = BTreeMap::new();
        registers.insert(
            "stream0_DestPort".to_string(),
            DeviceRegisterValue {
                addr: 0x40000,
                access: "rw".to_string(),
                int_value: Some(0),
                str_value: None,
                fields: BTreeMap::new(),
            },
        );
        let device = DeviceConfig {
            location: DeviceLocation {
                interface_name: "eth0".to_string(),
                interface_address: "192.168.1.10".to_string(),
                device_address: "192.168.1.20".to_string(),
            },
            capabilities: DeviceCapabilities::default(),
            memory_map: DeviceMemoryMap::default(),
            registers,
        };

        assert_eq!(client.read_named_u32(&device, "stream0_DestPort").unwrap(), 9);
    }
}
