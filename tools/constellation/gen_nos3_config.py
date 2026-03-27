#!/usr/bin/env python3
"""Generate NOS3 configuration for a multi-satellite constellation.

Creates 42 orbit/spacecraft files, NOS3 simulator XML with per-spacecraft
bus names, and a start script that launches sims + N cFS processes.

Usage:
    python3 tools/constellation/gen_nos3_config.py \
        --orbits 3 --sats-per-orbit 3 \
        --altitude 550 --inclination 87 \
        --output-dir tools/constellation/generated
"""

import argparse
import os
import xml.etree.ElementTree as ET
from pathlib import Path
from textwrap import dedent


def generate_orbit_file(orbit_index: int, num_orbits: int,
                        altitude_km: float, inclination_deg: float) -> str:
    raan = orbit_index * (360.0 / num_orbits)
    return dedent(f"""\
        <<<<<<<<<<<<<<<<<  42: Orbit Description File   >>>>>>>>>>>>>>>>>
        Orbit {orbit_index}                    !  Description
        CENTRAL                       !  Orbit Type (ZERO, FLIGHT, CENTRAL, THREE_BODY)
        ::::::::::::::  Use these lines if ZERO           :::::::::::::::::
        MINORBODY_2                   !  World
        FALSE                         ! Use Polyhedron Gravity
        ::::::::::::::  Use these lines if FLIGHT         :::::::::::::::::
        0                             !  Region Number
        FALSE                         ! Use Polyhedron Gravity
        ::::::::::::::  Use these lines if CENTRAL        :::::::::::::::::
        EARTH                         !  Orbit Center
        FALSE                         !  Secular Orbit Drift Due to J2
        KEP                           !  Use Keplerian elements (KEP) or (RV) or FILE
        PA                            !  Use Peri/Apoapsis (PA) or min alt/ecc (AE)
        {altitude_km:.1f}      {altitude_km:.1f}              !  Periapsis & Apoapsis Altitude, km
        {altitude_km:.1f}  0.0                    !  Min Altitude (km), Eccentricity
        {inclination_deg:.1f}                          !  Inclination (deg)
        {raan:.1f}                         !  Right Ascension of Ascending Node (deg)
        0.0                           !  Argument of Periapsis (deg)
        0.0                           !  True Anomaly (deg)
    """)


def generate_sc_file(sc_index: int, sats_per_orbit: int) -> str:
    """Generate a minimal 42 spacecraft file.

    Offsets the true anomaly so satellites within the same orbit
    are evenly spaced.
    """
    sat_in_orbit = sc_index % sats_per_orbit
    true_anomaly = sat_in_orbit * (360.0 / sats_per_orbit)

    # Read the template SC file and modify the true anomaly
    # For now, generate a minimal config referencing standard 42 body files
    return dedent(f"""\
        <<<<<<<<<<<<<<<<<  42: Spacecraft Description File  >>>>>>>>>>>>>>>>>
        SC_{sc_index}                         !  Description
        ************************* Orbit Parameters ****************************
        {true_anomaly:.1f}                          !  True Anomaly (deg) - offset for spacing
    """)


def generate_inp_sim(num_orbits: int, sats_per_orbit: int,
                     altitude_km: float, inclination_deg: float) -> str:
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
                <simulator>
                    <name>time</name>
                    <active>true</active>
                    <library>libtime_driver.so</library>
                    <hardware-model>
                        <type>NOS_TIME_DRIVER</type>
                        <real-time-factor>1.0</real-time-factor>
                    </hardware-model>
                </simulator>

                <simulator>
                    <name>truth42sim</name>
                    <active>true</active>
                    <library>libtruth_42_sim.so</library>
                    <hardware-model>
                        <type>TRUTH42SIM</type>
                        <data-provider>
                            <type>TRUTH42PROVIDER</type>
                            <hostname>fortytwo</hostname>
                            <port>4245</port>
                            <max-connection-attempts>30</max-connection-attempts>
                            <retry-wait-seconds>1</retry-wait-seconds>
                        </data-provider>
                    </hardware-model>
                </simulator>

        {"".join(sims)}
            </simulators>
        </nos3-configuration>
    """)


def generate_start_script(num_orbits: int, sats_per_orbit: int) -> str:
    total = num_orbits * sats_per_orbit
    return dedent(f"""\
        #!/bin/bash
        set -e

        TOTAL={total}
        NUM_ORBITS={num_orbits}
        SATS_PER_ORBIT={sats_per_orbit}
        BASE=/cFS/libs/nos3/fsw/build/exe/cpu1

        echo "Starting $TOTAL satellites ($NUM_ORBITS orbits x $SATS_PER_ORBIT sats)"

        # Wait for sims to be ready
        sleep 15

        pids=()
        for orb in $(seq 0 $((NUM_ORBITS - 1))); do
            for sat in $(seq 1 $SATS_PER_ORBIT); do
                SCID=$(( (orb + 1) * 1000 + sat ))
                INST=/cFS/run/sat_$SCID
                mkdir -p "$INST"
                cp -a "$BASE"/. "$INST"/
                echo "Launching SCID=$SCID (orbit=$orb, sat=$sat)"
                cd "$INST"
                ./core-cpu1 --scid "$SCID" &
                pids+=($!)
            done
        done

        echo "All $TOTAL satellites launched."

        cleanup() {{
            echo "Shutting down..."
            for pid in "${{pids[@]}}"; do
                kill "$pid" 2>/dev/null || true
            done
            wait
        }}

        trap cleanup SIGTERM SIGINT
        wait
    """)


def main():
    parser = argparse.ArgumentParser(description="Generate NOS3 constellation config")
    parser.add_argument("--orbits", type=int, default=3)
    parser.add_argument("--sats-per-orbit", type=int, default=3)
    parser.add_argument("--altitude", type=float, default=550.0,
                        help="Orbital altitude in km")
    parser.add_argument("--inclination", type=float, default=87.0,
                        help="Orbital inclination in degrees")
    parser.add_argument("--output-dir", type=str,
                        default="tools/constellation/generated")
    args = parser.parse_args()

    out = Path(args.output_dir)
    inout = out / "InOut"
    inout.mkdir(parents=True, exist_ok=True)

    # 42 configs
    inp_sim = generate_inp_sim(args.orbits, args.sats_per_orbit,
                               args.altitude, args.inclination)
    (inout / "Inp_Sim.txt").write_text(inp_sim)

    for i in range(args.orbits):
        orb = generate_orbit_file(i, args.orbits, args.altitude, args.inclination)
        (inout / f"Orb_{i}.txt").write_text(orb)

    total = args.orbits * args.sats_per_orbit
    for sc in range(total):
        sc_file = generate_sc_file(sc, args.sats_per_orbit)
        (inout / f"SC_{sc}.txt").write_text(sc_file)

    # NOS3 simulator XML
    sim_xml = generate_simulator_xml(args.orbits, args.sats_per_orbit)
    (out / "nos3-simulator.xml").write_text(sim_xml)

    # Start script
    start_sh = generate_start_script(args.orbits, args.sats_per_orbit)
    start_path = out / "start_constellation.sh"
    start_path.write_text(start_sh)
    start_path.chmod(0o755)

    print(f"Generated config for {total} satellites "
          f"({args.orbits} orbits x {args.sats_per_orbit} sats)")
    print(f"  42 configs:     {inout}/")
    print(f"  Simulator XML:  {out}/nos3-simulator.xml")
    print(f"  Start script:   {start_path}")


if __name__ == "__main__":
    main()
