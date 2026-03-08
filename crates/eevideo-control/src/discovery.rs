use std::collections::{BTreeMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

use if_addrs::{get_if_addrs, IfAddr};
use socket2::{Domain, Protocol, SockRef, Socket, Type};

use crate::coap::{CoapError, CoapMessage, CoapMessageType, CoapOption, CODE_CONTENT};

pub const DISCOVERY_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 1, 187);
pub const DISCOVERY_PORT: u16 = 5683;
pub const DISCOVERY_RESOURCE_TYPE: &str = "eev.cam";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryInterface {
    pub name: String,
    pub address: Ipv4Addr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryLink {
    pub target: String,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryAdvertisement {
    pub links: Vec<DiscoveryLink>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryResponse {
    pub interface_name: String,
    pub interface_address: Ipv4Addr,
    pub device_address: Ipv4Addr,
    pub advertisement: DiscoveryAdvertisement,
    pub message: CoapMessage,
}

pub fn build_discovery_request(token: &[u8], message_id: u16) -> Result<Vec<u8>, CoapError> {
    CoapMessage::new(
        CoapMessageType::NonConfirmable,
        1,
        message_id,
        token.to_vec(),
        vec![
            CoapOption::new(11, b".well-known".to_vec()),
            CoapOption::new(11, b"core".to_vec()),
            CoapOption::new(15, b"rt=eev.cam".to_vec()),
        ],
        Vec::<u8>::new(),
    )
    .encode()
}

pub fn parse_discovery_advertisement(
    payload: &[u8],
) -> Result<DiscoveryAdvertisement, DiscoveryError> {
    let payload = std::str::from_utf8(payload).map_err(DiscoveryError::InvalidUtf8)?;
    let mut links = Vec::new();

    for entry in payload.split(',').filter(|entry| !entry.trim().is_empty()) {
        let mut parts = entry.trim().split(';');
        let target = parts
            .next()
            .ok_or_else(|| DiscoveryError::InvalidAdvertisement(entry.trim().to_string()))?;
        let target = target
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
            .ok_or_else(|| DiscoveryError::InvalidAdvertisement(entry.trim().to_string()))?;

        let mut attributes = BTreeMap::new();
        for part in parts {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (key, value) = trimmed
                .split_once('=')
                .map(|(key, value)| (key, value.trim_matches('"')))
                .unwrap_or((trimmed, ""));
            attributes.insert(key.to_string(), value.to_string());
        }

        links.push(DiscoveryLink {
            target: target.to_string(),
            attributes,
        });
    }

    if links.is_empty() {
        return Err(DiscoveryError::InvalidAdvertisement(payload.to_string()));
    }

    Ok(DiscoveryAdvertisement { links })
}

pub fn discover_devices(
    interface_filter: Option<&str>,
    timeout: Duration,
) -> Result<Vec<DiscoveryResponse>, DiscoveryError> {
    let interfaces = list_interfaces(interface_filter)?;
    let request = build_discovery_request(&[0x01], 0x2000)?;
    let destination = SocketAddrV4::new(DISCOVERY_MULTICAST_ADDR, DISCOVERY_PORT);
    let mut responses = Vec::new();
    let mut seen = HashSet::new();

    for interface in interfaces {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .map_err(DiscoveryError::Io)?;
        socket
            .bind(&SocketAddrV4::new(interface.address, 0).into())
            .map_err(DiscoveryError::Io)?;

        let udp_socket: UdpSocket = socket.into();
        udp_socket
            .set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(DiscoveryError::Io)?;
        SockRef::from(&udp_socket)
            .set_multicast_if_v4(&interface.address)
            .map_err(DiscoveryError::Io)?;
        udp_socket
            .send_to(&request, destination)
            .map_err(DiscoveryError::Io)?;

        let deadline = Instant::now() + timeout;
        let mut buffer = [0u8; 2048];
        while Instant::now() < deadline {
            match udp_socket.recv_from(&mut buffer) {
                Ok((size, peer)) => {
                    let SocketAddr::V4(peer) = peer else {
                        continue;
                    };
                    let message = CoapMessage::decode(&buffer[..size])?;
                    if message.code != CODE_CONTENT {
                        continue;
                    }
                    let advertisement = parse_discovery_advertisement(&message.payload)?;
                    let key = (interface.name.clone(), *peer.ip());
                    if seen.insert(key) {
                        responses.push(DiscoveryResponse {
                            interface_name: interface.name.clone(),
                            interface_address: interface.address,
                            device_address: *peer.ip(),
                            advertisement,
                            message,
                        });
                    }
                }
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(err) => return Err(DiscoveryError::Io(err)),
            }
        }
    }

    Ok(responses)
}

fn list_interfaces(
    interface_filter: Option<&str>,
) -> Result<Vec<DiscoveryInterface>, DiscoveryError> {
    let mut interfaces = Vec::new();
    for interface in get_if_addrs().map_err(DiscoveryError::Io)? {
        if let Some(filter) = interface_filter {
            if interface.name != filter {
                continue;
            }
        }
        if interface.is_loopback() {
            continue;
        }
        if let IfAddr::V4(address) = interface.addr {
            interfaces.push(DiscoveryInterface {
                name: interface.name,
                address: address.ip,
            });
        }
    }
    Ok(interfaces)
}

#[derive(Debug)]
pub enum DiscoveryError {
    Io(std::io::Error),
    Coap(CoapError),
    InvalidUtf8(std::str::Utf8Error),
    InvalidAdvertisement(String),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => err.fmt(f),
            Self::Coap(err) => err.fmt(f),
            Self::InvalidUtf8(err) => err.fmt(f),
            Self::InvalidAdvertisement(payload) => {
                write!(f, "invalid discovery advertisement payload: {payload}")
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}

impl From<CoapError> for DiscoveryError {
    fn from(value: CoapError) -> Self {
        Self::Coap(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{build_discovery_request, parse_discovery_advertisement};

    #[test]
    fn discovery_request_matches_upstream_bytes() {
        let bytes = build_discovery_request(&[0x01], 0x2000).unwrap();
        let mut expected = vec![0x51, 0x01, 0x20, 0x00, 0x01, 0xBB];
        expected.extend_from_slice(b".well-known");
        expected.push(0x04);
        expected.extend_from_slice(b"core");
        expected.push(0x4A);
        expected.extend_from_slice(b"rt=eev.cam");

        assert_eq!(bytes, expected);
    }

    #[test]
    fn parses_link_format_payload() {
        let advertisement =
            parse_discovery_advertisement(br#"</stream>;rt="eev.cam";if="eth0""#).unwrap();
        assert_eq!(advertisement.links.len(), 1);
        assert_eq!(advertisement.links[0].target, "/stream");
        assert_eq!(advertisement.links[0].attributes["rt"], "eev.cam");
        assert_eq!(advertisement.links[0].attributes["if"], "eth0");
    }
}
