#!/usr/bin/env python3
"""Generate NOS3 configuration for a multi-satellite constellation.

Creates 42 orbit/spacecraft files and NOS3 simulator XML with
per-spacecraft bus names (spi_sc0, usart_sc0, etc.).

Usage:
    python3 tools/constellation/gen_nos3_config.py \
        --orbits 3 --sats-per-orbit 3
"""

import argparse
from pathlib import Path
from textwrap import dedent


def copy_orbit_file(src_dir: Path, orbit_index: int, num_orbits: int,
                    dest_dir: Path) -> None:
    """Copies the existing orbit template, adjusting RAAN for the orbit plane."""
    src = src_dir / "Orb_LEO.txt"
    if not src.exists():
        raise FileNotFoundError(f"Orbit template not found: {src}")
    content = src.read_text()
    raan = orbit_index * (360.0 / num_orbits)
    # Replace the RAAN line (follows "Inclination" line)
    lines = content.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if "Right Ascension" in line:
            parts = line.split("!")
            lines[i] = f"{raan:.1f}                         !{parts[1]}"
            break
    (dest_dir / f"Orb_{orbit_index}.txt").write_text("".join(lines))


def copy_sc_file(src_dir: Path, sc_index: int, sats_per_orbit: int,
                 dest_dir: Path) -> None:
    """Copies the existing SC template, adjusting true anomaly for spacing."""
    src = src_dir / "SC_NOS3.txt"
    if not src.exists():
        raise FileNotFoundError(f"SC template not found: {src}")
    content = src.read_text()
    sat_in_orbit = sc_index % sats_per_orbit
    true_anomaly = sat_in_orbit * (360.0 / sats_per_orbit)
    # Replace the initial true anomaly (usually "True Anomaly" line in orbit section)
    lines = content.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if "True Anomaly" in line and "Initial" in line:
            parts = line.split("!")
            lines[i] = f"{true_anomaly:.1f}                           !{parts[1]}"
            break
    (dest_dir / f"SC_{sc_index}.txt").write_text("".join(lines))


def generate_inp_sim(num_orbits: int, sats_per_orbit: int) -> str:
    total = num_orbits * sats_per_orbit
    orbit_lines = []
    for i in range(num_orbits):
        orbit_lines.append(f"TRUE   Orb_{i}.txt              !  Input file name for Orb {i}")

    sc_lines = []
    for sc in range(total):
        orbit_ref = sc // sats_per_orbit
        sc_lines.append(f"TRUE  {orbit_ref} SC_{sc}.txt             !  Existence, RefOrb, Input file for SC {sc}")

    return dedent(f"""\
        <<<<<<<<<<<<<<<<<  42: The Mostly Harmless Simulator  >>>>>>>>>>>>>>>>>
        ************************** Simulation Control **************************
        NOS3                            !  Time Mode (FAST, REAL, EXTERNAL, or NOS3)
        604800.0   0.01                 !  Sim Duration, Step Size [sec]
        1.0                             !  File Output Interval [sec]
        0                               !  RNG Seed
        FALSE                           !  Graphics Front End?
        Inp_Cmd.txt                     !  Command Script File Name
        **************************  Reference Orbits  **************************
        {num_orbits}                               !  Number of Reference Orbits
        {chr(10).join(orbit_lines)}
        *****************************  Spacecraft  *****************************
        {total}                               !  Number of Spacecraft
        {chr(10).join(sc_lines)}
        ***************************** Environment  *****************************
        10 20 2025                      !  Date (UTC) (Month, Day, Year)
        17 43 20.00                     !  Time (UTC) (Hr,Min,Sec)
        37.0                            !  Leap Seconds (sec)
        USER                            !  F10.7, Ap (USER, NOMINAL or TWOSIGMA)
        230.0                           !  USER-provided F10.7
        100.0                           !  USER-provided Ap
        IGRF                            !  Magfield (NONE,DIPOLE,IGRF)
        8   8                           !  IGRF Degree and Order (<=10)
        8   8                           !  Earth Gravity Model N and M (<=18)
        2   0                           !  Mars Gravity Model N and M (<=18)
        2   0                           !  Luna Gravity Model N and M (<=18)
        FALSE   FALSE                   !  Aerodynamic Forces & Torques (Shadows)
        FALSE                           !  Gravity Gradient Torques
        FALSE   FALSE                   !  Solar Pressure Forces & Torques (Shadows)
        FALSE                           !  Residual Magnetic Moment Torques
    """)


def generate_simulator_xml(num_orbits: int, sats_per_orbit: int) -> str:
    total = num_orbits * sats_per_orbit

    sims = []
    for sc in range(total):
        spi_bus = f"spi_sc{sc}"
        usart_bus = f"usart_sc{sc}"

        sims.append(dedent(f"""\
            <simulator>
                <name>thermal-cam-sim-sc{sc}</name>
                <active>true</active>
                <library>libthermal_cam_sim.so</library>
                <hardware-model>
                    <type>THERMAL_CAM</type>
                    <connections>
                        <connection>
                            <type>command</type>
                            <bus-name>command</bus-name>
                            <node-name>thermal-cam-command-sc{sc}</node-name>
                        </connection>
                    </connections>
                    <spi>
                        <bus>{spi_bus}</bus>
                        <chip_select>3</chip_select>
                    </spi>
                    <data-provider>
                        <type>THERMALCAMPROVIDER</type>
                        <hostname>fortytwo</hostname>
                        <port>4245</port>
                        <max-connection-attempts>30</max-connection-attempts>
                        <retry-wait-seconds>1</retry-wait-seconds>
                        <spacecraft>{sc}</spacecraft>
                    </data-provider>
                    <aoi>
                        <west>-122.5</west>
                        <south>38.0</south>
                        <east>-121.5</east>
                        <north>39.0</north>
                    </aoi>
                    <data-dir>/sim/thermal_data</data-dir>
                </hardware-model>
            </simulator>
        """))

        sims.append(dedent(f"""\
            <simulator>
                <name>gps-sim-sc{sc}</name>
                <active>true</active>
                <library>libgps_sim.so</library>
                <hardware-model>
                    <type>OEM615</type>
                    <connections>
                        <connection>
                            <type>usart</type>
                            <bus-name>{usart_bus}</bus-name>
                            <node-port>1</node-port>
                        </connection>
                    </connections>
                    <data-provider>
                        <type>GPS42SOCKET</type>
                        <hostname>fortytwo</hostname>
                        <port>4245</port>
                        <max-connection-attempts>30</max-connection-attempts>
                        <retry-wait-seconds>1</retry-wait-seconds>
                        <spacecraft>{sc}</spacecraft>
                        <GPS>0</GPS>
                        <leap-seconds>37</leap-seconds>
                    </data-provider>
                </hardware-model>
            </simulator>
        """))

    return dedent(f"""\
        <?xml version="1.0" encoding="utf-8"?>
        <nos3-configuration>
            <common>
                <log-config>nos3-log.xml</log-config>
                <time>
                    <type>NOS3</type>
                    <bus-name>command</bus-name>
                    <tick-topic>SIMTIME</tick-topic>
                </time>
                <absolute-start-time>0</absolute-start-time>
                <sim-microseconds-per-tick>250000</sim-microseconds-per-tick>
                <connection>
                    <type>TCP</type>
                    <hostname>nos-engine-server</hostname>
                    <port>12001</port>
                </connection>
            </common>
            <simulators>
        {"".join(sims)}
            </simulators>
        </nos3-configuration>
    """)


def main():
    parser = argparse.ArgumentParser(description="Generate NOS3 constellation config")
    parser.add_argument("--orbits", type=int, default=3)
    parser.add_argument("--sats-per-orbit", type=int, default=3)
    parser.add_argument("--output-dir", type=str,
                        default="tools/constellation/generated")
    parser.add_argument("--template-dir", type=str,
                        default="libs/nos3/cfg/InOut",
                        help="Path to existing 42 InOut directory with templates")
    args = parser.parse_args()

    out = Path(args.output_dir)
    inout = out / "InOut"
    inout.mkdir(parents=True, exist_ok=True)
    src_42 = Path(args.template_dir)

    # 42 configs
    inp_sim = generate_inp_sim(args.orbits, args.sats_per_orbit)
    (inout / "Inp_Sim.txt").write_text(inp_sim)

    for i in range(args.orbits):
        copy_orbit_file(src_42, i, args.orbits, inout)

    total = args.orbits * args.sats_per_orbit
    for sc in range(total):
        copy_sc_file(src_42, sc, args.sats_per_orbit, inout)

    # NOS3 simulator XML
    sim_xml = generate_simulator_xml(args.orbits, args.sats_per_orbit)
    (out / "nos3-simulator.xml").write_text(sim_xml)

    print(f"Generated config for {total} satellites "
          f"({args.orbits} orbits x {args.sats_per_orbit} sats)")
    print(f"  42 configs:     {inout}/")
    print(f"  Simulator XML:  {out}/nos3-simulator.xml")


if __name__ == "__main__":
    main()
