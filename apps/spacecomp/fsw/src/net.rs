use leodos_libcfs::os::net::SocketAddr;
use leodos_protocols::datalink::link::cfs::UdpDataLink;
use leodos_protocols::network::isl::torus::Torus;

use crate::{NUM_ORBITS, NUM_SATS};

const LOCALHOST: &str = "127.0.0.1";
const PORT_BASE: u16 = 6000;
const PORTS_PER_SAT: u16 = 10;

const PORT_NORTH: u16 = 0;
const PORT_SOUTH: u16 = 2;
const PORT_EAST: u16 = 4;
const PORT_WEST: u16 = 6;
const PORT_GROUND: u16 = 8;

pub struct IslLinks {
    pub north: UdpDataLink,
    pub south: UdpDataLink,
    pub east: UdpDataLink,
    pub west: UdpDataLink,
    pub ground: UdpDataLink,
}

/// Returns the base UDP port for a satellite at (orbit, sat).
/// Each satellite owns a block of PORTS_PER_SAT consecutive ports
/// starting at PORT_BASE, laid out linearly across the constellation.
fn sat_base_port(orbit: u8, sat: u8) -> u16 {
    PORT_BASE + (orbit as u16 * NUM_SATS as u16 + sat as u16) * PORTS_PER_SAT
}

/// Writes a u8 as ASCII decimal digits into `buf`, returning the number
/// of bytes written. No-std replacement for itoa/format!.
fn format_u8(mut n: u8, buf: &mut [u8; 3]) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut i = 3;
    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10);
        n /= 10;
    }
    let len = 3 - i;
    if i > 0 {
        buf.copy_within(i..3, 0);
    }
    len
}

/// Formats the Docker static IP for a given orbit: "172.20.{orbit}.10".
/// Writes into `out` and returns the total byte length.
fn orbit_ip(orbit: u8, out: &mut [u8; 16]) -> usize {
    let prefix = b"172.20.";
    out[..7].copy_from_slice(prefix);
    let mut pos = 7;

    let mut digit_buf = [0u8; 3];
    let len = format_u8(orbit, &mut digit_buf);
    out[pos..pos + len].copy_from_slice(&digit_buf[..len]);
    pos += len;

    let suffix = b".10";
    out[pos..pos + 3].copy_from_slice(suffix);
    pos += 3;

    pos
}

fn local_link(my_port: u16, remote_port: u16) -> Result<UdpDataLink, leodos_libcfs::error::Error> {
    let local = SocketAddr::new_ipv4(LOCALHOST, my_port)?;
    let remote = SocketAddr::new_ipv4(LOCALHOST, remote_port)?;
    UdpDataLink::bind(local, remote)
}

fn cross_orbit_link(
    my_port: u16,
    remote_orbit: u8,
    remote_port: u16,
) -> Result<UdpDataLink, leodos_libcfs::error::Error> {
    let mut ip_buf = [0u8; 16];
    let len = orbit_ip(remote_orbit, &mut ip_buf);
    let ip = core::str::from_utf8(&ip_buf[..len]).unwrap_or(LOCALHOST);
    let local = SocketAddr::new_ipv4(LOCALHOST, my_port)?;
    let remote = SocketAddr::new_ipv4(ip, remote_port)?;
    UdpDataLink::bind(local, remote)
}

/// Binds all 5 ISL links (N, S, E, W, ground) for a satellite at (orbit, sat).
/// N/S are intra-orbit (localhost), E/W are inter-orbit (Docker container IPs).
pub fn bind_isl_links(orbit: u8, sat: u8) -> Result<IslLinks, leodos_libcfs::error::Error> {
    let my = sat_base_port(orbit, sat);

    let n_sat = Torus::next(sat, NUM_SATS);
    let s_sat = Torus::prev(sat, NUM_SATS);
    let e_orb = Torus::next(orbit, NUM_ORBITS);
    let w_orb = Torus::prev(orbit, NUM_ORBITS);

    Ok(IslLinks {
        north: local_link(
            my + PORT_NORTH,
            sat_base_port(orbit, n_sat) + PORT_SOUTH + 1,
        )?,
        south: local_link(
            my + PORT_SOUTH,
            sat_base_port(orbit, s_sat) + PORT_NORTH + 1,
        )?,
        east: cross_orbit_link(
            my + PORT_EAST,
            e_orb,
            sat_base_port(e_orb, sat) + PORT_WEST + 1,
        )?,
        west: cross_orbit_link(
            my + PORT_WEST,
            w_orb,
            sat_base_port(w_orb, sat) + PORT_EAST + 1,
        )?,
        ground: local_link(my + PORT_GROUND, my + PORT_GROUND + 1)?,
    })
}
