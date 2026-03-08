use std::fmt;

pub const OPTION_EEV_REG_ACCESS: u16 = 65301;
pub const OPTION_EEV_BINARY_ADDRESS: u16 = 65305;
pub const CODE_GET: u8 = 1;
pub const CODE_PUT: u8 = 3;
pub const CODE_CHANGED: u8 = 68;
pub const CODE_CONTENT: u8 = 69;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoapMessageType {
    Confirmable = 0,
    NonConfirmable = 1,
    Acknowledgement = 2,
    Reset = 3,
}

impl CoapMessageType {
    fn from_u8(value: u8) -> Result<Self, CoapError> {
        match value {
            0 => Ok(Self::Confirmable),
            1 => Ok(Self::NonConfirmable),
            2 => Ok(Self::Acknowledgement),
            3 => Ok(Self::Reset),
            _ => Err(CoapError::InvalidType(value)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoapOption {
    pub number: u16,
    pub value: Vec<u8>,
}

impl CoapOption {
    pub fn new(number: u16, value: impl Into<Vec<u8>>) -> Self {
        Self {
            number,
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoapMessage {
    pub version: u8,
    pub message_type: CoapMessageType,
    pub code: u8,
    pub message_id: u16,
    pub token: Vec<u8>,
    pub options: Vec<CoapOption>,
    pub payload: Vec<u8>,
}

impl CoapMessage {
    pub fn new(
        message_type: CoapMessageType,
        code: u8,
        message_id: u16,
        token: impl Into<Vec<u8>>,
        options: Vec<CoapOption>,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            version: 1,
            message_type,
            code,
            message_id,
            token: token.into(),
            options,
            payload: payload.into(),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, CoapError> {
        if self.token.len() > 8 {
            return Err(CoapError::TokenTooLong(self.token.len()));
        }

        let mut buf = Vec::with_capacity(128);
        let first = (self.version << 6) | ((self.message_type as u8) << 4) | self.token.len() as u8;
        buf.push(first);
        buf.push(self.code);
        buf.extend_from_slice(&self.message_id.to_be_bytes());
        buf.extend_from_slice(&self.token);

        let mut previous = 0u16;
        for option in &self.options {
            if option.number < previous {
                return Err(CoapError::OptionsOutOfOrder {
                    previous,
                    current: option.number,
                });
            }
            let delta = option.number - previous;
            let (delta_nibble, mut delta_ext) = encode_extended(delta)?;
            let (length_nibble, mut length_ext) = encode_extended(option.value.len() as u16)?;
            buf.push((delta_nibble << 4) | length_nibble);
            buf.append(&mut delta_ext);
            buf.append(&mut length_ext);
            buf.extend_from_slice(&option.value);
            previous = option.number;
        }

        if !self.payload.is_empty() {
            buf.push(0xFF);
            buf.extend_from_slice(&self.payload);
        }

        Ok(buf)
    }

    pub fn decode(data: &[u8]) -> Result<Self, CoapError> {
        if data.len() < 4 {
            return Err(CoapError::MessageTooShort(data.len()));
        }

        let version = data[0] >> 6;
        if version != 1 {
            return Err(CoapError::InvalidVersion(version));
        }

        let token_len = (data[0] & 0x0F) as usize;
        let message_type = CoapMessageType::from_u8((data[0] >> 4) & 0x03)?;
        let code = data[1];
        let message_id = u16::from_be_bytes([data[2], data[3]]);

        let mut position = 4usize;
        if position + token_len > data.len() {
            return Err(CoapError::Truncated("token"));
        }
        let token = data[position..position + token_len].to_vec();
        position += token_len;

        let mut previous = 0u16;
        let mut options = Vec::new();
        while position < data.len() && data[position] != 0xFF {
            let header = data[position];
            position += 1;

            let (delta, consumed_delta) = decode_extended(header >> 4, &data[position..])?;
            position += consumed_delta;
            let (length, consumed_length) = decode_extended(header & 0x0F, &data[position..])?;
            position += consumed_length;

            let end = position + length as usize;
            if end > data.len() {
                return Err(CoapError::Truncated("option value"));
            }

            previous = previous
                .checked_add(delta)
                .ok_or(CoapError::OptionNumberOverflow)?;
            options.push(CoapOption {
                number: previous,
                value: data[position..end].to_vec(),
            });
            position = end;
        }

        let payload = if position < data.len() && data[position] == 0xFF {
            position += 1;
            data[position..].to_vec()
        } else {
            Vec::new()
        };

        Ok(Self {
            version,
            message_type,
            code,
            message_id,
            token,
            options,
            payload,
        })
    }
}

pub fn response_code_description(code: u8) -> Option<&'static str> {
    match code {
        64 => Some("2.00 Success"),
        68 => Some("2.04 Changed"),
        69 => Some("2.05 Content"),
        128 => Some("4.00 Bad Request"),
        129 => Some("4.01 Unauthorized"),
        130 => Some("4.02 Bad Option"),
        131 => Some("4.03 Forbidden"),
        132 => Some("4.04 Not Found"),
        160 => Some("5.00 Internal Server Error"),
        161 => Some("5.01 Not Implemented"),
        _ => None,
    }
}

fn encode_extended(value: u16) -> Result<(u8, Vec<u8>), CoapError> {
    match value {
        0..=12 => Ok((value as u8, Vec::new())),
        13..=268 => Ok((13, vec![(value - 13) as u8])),
        269..=u16::MAX => {
            let extended = value - 269;
            Ok((14, extended.to_be_bytes().to_vec()))
        }
    }
}

fn decode_extended(nibble: u8, remaining: &[u8]) -> Result<(u16, usize), CoapError> {
    match nibble {
        0..=12 => Ok((nibble as u16, 0)),
        13 => {
            if remaining.is_empty() {
                return Err(CoapError::Truncated("extended option nibble"));
            }
            Ok((13 + remaining[0] as u16, 1))
        }
        14 => {
            if remaining.len() < 2 {
                return Err(CoapError::Truncated("extended option nibble"));
            }
            Ok((269 + u16::from_be_bytes([remaining[0], remaining[1]]), 2))
        }
        15 => Err(CoapError::InvalidOptionNibble(nibble)),
        _ => Err(CoapError::InvalidOptionNibble(nibble)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoapError {
    TokenTooLong(usize),
    OptionsOutOfOrder { previous: u16, current: u16 },
    MessageTooShort(usize),
    InvalidVersion(u8),
    InvalidType(u8),
    InvalidOptionNibble(u8),
    OptionNumberOverflow,
    Truncated(&'static str),
}

impl fmt::Display for CoapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenTooLong(len) => write!(f, "token length must not exceed 8 bytes, got {len}"),
            Self::OptionsOutOfOrder { previous, current } => {
                write!(
                    f,
                    "options must be in ascending order: {current} after {previous}"
                )
            }
            Self::MessageTooShort(len) => write!(f, "message too short: {len} bytes"),
            Self::InvalidVersion(version) => write!(f, "invalid CoAP version {version}"),
            Self::InvalidType(kind) => write!(f, "invalid CoAP message type {kind}"),
            Self::InvalidOptionNibble(nibble) => write!(f, "invalid CoAP option nibble {nibble}"),
            Self::OptionNumberOverflow => f.write_str("option number overflow"),
            Self::Truncated(what) => write!(f, "truncated {what}"),
        }
    }
}

impl std::error::Error for CoapError {}

#[cfg(test)]
mod tests {
    use super::{
        CoapMessage, CoapMessageType, CoapOption, OPTION_EEV_BINARY_ADDRESS, OPTION_EEV_REG_ACCESS,
    };

    #[test]
    fn coap_round_trip_preserves_extended_options() {
        let message = CoapMessage::new(
            CoapMessageType::Confirmable,
            3,
            0x2000,
            [0x12, 0x34],
            vec![
                CoapOption::new(OPTION_EEV_REG_ACCESS, [0x22]),
                CoapOption::new(OPTION_EEV_BINARY_ADDRESS, [0, 0, 0x10, 0]),
            ],
            [0xDE, 0xAD, 0xBE, 0xEF],
        );

        let encoded = message.encode().unwrap();
        let decoded = CoapMessage::decode(&encoded).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn encode_rejects_descending_options() {
        let message = CoapMessage::new(
            CoapMessageType::Confirmable,
            1,
            1,
            Vec::<u8>::new(),
            vec![CoapOption::new(12, []), CoapOption::new(11, [])],
            Vec::<u8>::new(),
        );

        assert!(message.encode().is_err());
    }
}
