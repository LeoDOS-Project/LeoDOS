use leodos_libcfs::error::Error;
use leodos_libcfs::os::net::SocketAddr;

use leodos_protocols::datalink::link::cfs::UdpDataLink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::routing::algorithm::distance_minimizing::DistanceMinimizing;
use leodos_protocols::network::isl::routing::local::{LocalAppHandle, LocalChannel, LocalRouterHandle};
use leodos_protocols::network::isl::routing::Router;
use leodos_protocols::network::isl::torus::Torus;

pub type UdpRouter<'a, const Q: usize, const M: usize> = Router<
    UdpDataLink,
    UdpDataLink,
    UdpDataLink,
    UdpDataLink,
    UdpDataLink,
    LocalRouterHandle<'a, Q, M>,
    DistanceMinimizing,
>;

pub struct ConstellationConfig {
    pub orbit: u8,
    pub sat: u8,
    pub num_orbits: u8,
    pub num_sats: u8,
    pub inclination_rad: f32,
    pub port_base: u16,
}

pub struct IslStack<'a, const Q: usize, const M: usize> {
    pub router: UdpRouter<'a, Q, M>,
    pub app_link: LocalAppHandle<'a, Q, M>,
    pub address: Address,
}

const LOCALHOST: &str = "127.0.0.1";
const PORTS_PER_SAT: u16 = 10;

const PORT_NORTH: u16 = 0;
const PORT_SOUTH: u16 = 2;
const PORT_EAST: u16 = 4;
const PORT_WEST: u16 = 6;
const PORT_GROUND: u16 = 8;

fn sat_base_port(orbit: u8, sat: u8, port_base: u16, num_sats: u8) -> u16 {
    port_base + (orbit as u16 * num_sats as u16 + sat as u16) * PORTS_PER_SAT
}

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

fn local_link(my_port: u16, remote_port: u16) -> Result<UdpDataLink, Error> {
    let local = SocketAddr::new_ipv4(LOCALHOST, my_port)?;
    let remote = SocketAddr::new_ipv4(LOCALHOST, remote_port)?;
    UdpDataLink::bind(local, remote)
}

fn cross_orbit_link(
    my_port: u16,
    remote_orbit: u8,
    remote_port: u16,
) -> Result<UdpDataLink, Error> {
    let mut ip_buf = [0u8; 16];
    let len = orbit_ip(remote_orbit, &mut ip_buf);
    let ip = core::str::from_utf8(&ip_buf[..len]).unwrap_or(LOCALHOST);
    let local = SocketAddr::new_ipv4(LOCALHOST, my_port)?;
    let remote = SocketAddr::new_ipv4(ip, remote_port)?;
    UdpDataLink::bind(local, remote)
}

struct IslLinks {
    north: UdpDataLink,
    south: UdpDataLink,
    east: UdpDataLink,
    west: UdpDataLink,
    ground: UdpDataLink,
}

fn bind_isl_links(config: &ConstellationConfig) -> Result<IslLinks, Error> {
    let orbit = config.orbit;
    let sat = config.sat;
    let my = sat_base_port(orbit, sat, config.port_base, config.num_sats);

    let n_sat = Torus::next(sat, config.num_sats);
    let s_sat = Torus::prev(sat, config.num_sats);
    let e_orb = Torus::next(orbit, config.num_orbits);
    let w_orb = Torus::prev(orbit, config.num_orbits);

    Ok(IslLinks {
        north: local_link(
            my + PORT_NORTH,
            sat_base_port(orbit, n_sat, config.port_base, config.num_sats) + PORT_SOUTH + 1,
        )?,
        south: local_link(
            my + PORT_SOUTH,
            sat_base_port(orbit, s_sat, config.port_base, config.num_sats) + PORT_NORTH + 1,
        )?,
        east: cross_orbit_link(
            my + PORT_EAST,
            e_orb,
            sat_base_port(e_orb, sat, config.port_base, config.num_sats) + PORT_WEST + 1,
        )?,
        west: cross_orbit_link(
            my + PORT_WEST,
            w_orb,
            sat_base_port(w_orb, sat, config.port_base, config.num_sats) + PORT_EAST + 1,
        )?,
        ground: local_link(my + PORT_GROUND, my + PORT_GROUND + 1)?,
    })
}

pub fn build_isl_stack<'a, const Q: usize, const M: usize>(
    config: &ConstellationConfig,
    channel: &'a LocalChannel<Q, M>,
) -> Result<IslStack<'a, Q, M>, Error> {
    let address = Address::satellite(config.orbit, config.sat);
    let links = bind_isl_links(config)?;
    let (app_link, router_link) = channel.split();
    let algorithm = DistanceMinimizing::new(config.inclination_rad);

    let router = Router::new(
        links.north,
        links.south,
        links.east,
        links.west,
        links.ground,
        router_link,
        address,
        config.num_orbits,
        config.num_sats,
        algorithm,
    );

    Ok(IslStack {
        router,
        app_link,
        address,
    })
}
