use std::net::UdpSocket;

pub fn reserve_udp_port(bind_address: &str) -> (UdpSocket, u32) {
    let socket = UdpSocket::bind((bind_address, 0)).expect("reserve UDP port");
    let port = socket.local_addr().expect("reserved port").port() as u32;
    (socket, port)
}
