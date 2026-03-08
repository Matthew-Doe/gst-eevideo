pub(crate) use eevideo_control::{
    default_control_backend, ControlSession, ControlTarget, ControlTransportKind,
    RequestedStreamConfiguration as StreamConfiguration, SharedControlBackend,
    StreamFormatDescriptor,
};

pub(crate) fn default_control_target(stream_name: &str) -> ControlTarget {
    ControlTarget {
        device_uri: stream_name.to_string(),
        transport_kind: ControlTransportKind::Noop,
        auth_scope: None,
    }
}
