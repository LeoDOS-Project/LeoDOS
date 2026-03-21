"""Simple circular orbit propagator for Walker Delta constellations.

Computes the sub-satellite point (lat, lon) for each satellite at
each pass time, assuming circular orbits and a spherical Earth with
J0 rotation (no J2 perturbation — sufficient for simulation).
"""

from dataclasses import dataclass

import numpy as np


# WGS-84
EARTH_RADIUS_KM = 6371.0
EARTH_MU_KM3S2 = 398600.4418
EARTH_ROTATION_RAD_S = 7.2921159e-5


@dataclass
class WalkerConstellation:
    """Walker Delta constellation parameters."""

    num_orbits: int = 3
    sats_per_orbit: int = 3
    altitude_km: float = 550.0
    inclination_deg: float = 53.0
    raan_spread_deg: float = 360.0

    @property
    def total_sats(self) -> int:
        return self.num_orbits * self.sats_per_orbit

    @property
    def orbital_radius_km(self) -> float:
        return EARTH_RADIUS_KM + self.altitude_km

    @property
    def period_s(self) -> float:
        return 2 * np.pi * np.sqrt(self.orbital_radius_km**3 / EARTH_MU_KM3S2)

    def sub_satellite_point(
        self, orbit_idx: int, sat_idx: int, time_s: float
    ) -> tuple[float, float]:
        """Compute (lat_deg, lon_deg) of a satellite at a given time.

        Uses Keplerian circular orbit with Earth rotation.
        """
        r = self.orbital_radius_km
        period = self.period_s
        inc = np.radians(self.inclination_deg)

        # RAAN for this orbital plane
        raan = np.radians(
            orbit_idx * self.raan_spread_deg / self.num_orbits
        )

        # True anomaly: initial phase offset + orbital motion
        phase_offset = sat_idx * 2 * np.pi / self.sats_per_orbit
        mean_motion = 2 * np.pi / period
        nu = phase_offset + mean_motion * time_s

        # Position in orbital frame
        x_orb = np.cos(nu)
        y_orb = np.sin(nu)

        # Rotate to ECI (Earth-Centered Inertial)
        cos_raan = np.cos(raan)
        sin_raan = np.sin(raan)
        cos_inc = np.cos(inc)
        sin_inc = np.sin(inc)

        x_eci = cos_raan * x_orb - sin_raan * cos_inc * y_orb
        y_eci = sin_raan * x_orb + cos_raan * cos_inc * y_orb
        z_eci = sin_inc * y_orb

        # ECI to ECEF (rotate by Earth rotation)
        theta = EARTH_ROTATION_RAD_S * time_s
        cos_t = np.cos(theta)
        sin_t = np.sin(theta)
        x_ecef = cos_t * x_eci + sin_t * y_eci
        y_ecef = -sin_t * x_eci + cos_t * y_eci
        z_ecef = z_eci

        # ECEF to lat/lon
        lat = np.degrees(np.arcsin(z_ecef))
        lon = np.degrees(np.arctan2(y_ecef, x_ecef))

        return float(lat), float(lon)

    def ground_track(
        self,
        orbit_idx: int,
        sat_idx: int,
        t_start: float,
        t_end: float,
        dt: float = 60.0,
    ) -> list[tuple[float, float, float]]:
        """Compute ground track as [(time_s, lat_deg, lon_deg), ...]."""
        track = []
        t = t_start
        while t <= t_end:
            lat, lon = self.sub_satellite_point(orbit_idx, sat_idx, t)
            track.append((t, lat, lon))
            t += dt
        return track

    def fov_box(
        self,
        orbit_idx: int,
        sat_idx: int,
        time_s: float,
        fov_deg: float = 10.0,
    ) -> tuple[float, float, float, float]:
        """Compute the ground FOV bounding box (west, south, east, north).

        Uses a simple angular projection from nadir.
        """
        lat, lon = self.sub_satellite_point(orbit_idx, sat_idx, time_s)
        half_swath_deg = np.degrees(
            np.arctan(
                np.tan(np.radians(fov_deg / 2))
                * self.altitude_km
                / EARTH_RADIUS_KM
            )
        )
        # Adjust for latitude (longitude degrees shrink toward poles)
        lon_stretch = half_swath_deg / max(np.cos(np.radians(lat)), 0.01)

        west = lon - lon_stretch
        east = lon + lon_stretch
        south = lat - half_swath_deg
        north = lat + half_swath_deg

        return float(west), float(south), float(east), float(north)
