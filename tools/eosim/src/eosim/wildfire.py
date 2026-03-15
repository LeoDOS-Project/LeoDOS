"""Synthetic thermal IR raster generator for wildfire detection testing.

Generates brightness temperature (BT) rasters that mimic what a
satellite thermal IR sensor would see. The MWIR band (~3.9 µm) is
the primary fire detection channel — fires are extremely bright
against a ~300 K background. The LWIR band (~11 µm) provides context
for the MODIS/VIIRS contextual algorithm.

Output: single-band or dual-band GeoTIFF rasters in Kelvin,
indexed by pass number.
"""

from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import rasterio
from rasterio.transform import from_bounds
import yaml


@dataclass
class Fire:
    """A single fire ignition event."""

    lat: float
    lon: float
    onset_pass: int
    peak_temp_k: float = 600.0
    spread_rate_px: float = 2.0
    initial_radius_px: float = 3.0

    def radius_at(self, pass_num: int) -> float:
        elapsed = max(0, pass_num - self.onset_pass)
        return self.initial_radius_px + self.spread_rate_px * elapsed

    def temp_at(self, pass_num: int) -> float:
        if pass_num < self.onset_pass:
            return 0.0
        return self.peak_temp_k


@dataclass
class Scenario:
    """A wildfire detection scenario."""

    name: str
    aoi: tuple[float, float, float, float]  # (west, south, east, north)
    width_px: int = 512
    height_px: int = 512
    background_temp_k: float = 300.0
    background_std_k: float = 3.0
    sensor_nedt_k: float = 0.5
    fires: list[Fire] = field(default_factory=list)
    num_passes: int = 10

    @classmethod
    def from_yaml(cls, path: str | Path) -> "Scenario":
        with open(path) as f:
            data = yaml.safe_load(f)

        s = data["scenario"]
        fires = [Fire(**f) for f in s.get("fires", [])]
        return cls(
            name=s["name"],
            aoi=tuple(s["aoi"]),
            width_px=s.get("width_px", 512),
            height_px=s.get("height_px", 512),
            background_temp_k=s.get("background_temp_k", 300.0),
            background_std_k=s.get("background_std_k", 3.0),
            sensor_nedt_k=s.get("sensor_nedt_k", 0.5),
            fires=fires,
            num_passes=s.get("num_passes", 10),
        )


def generate_background(scenario: Scenario, rng: np.random.Generator) -> np.ndarray:
    """Generate a spatially-varying background BT field.

    Uses smooth Perlin-like variation (low-freq noise) to simulate
    land cover differences, plus high-freq noise for sensor NEdT.
    """
    h, w = scenario.height_px, scenario.width_px

    # Low-frequency spatial variation (land cover, terrain)
    freq = 8
    y = np.linspace(0, freq * np.pi, h)
    x = np.linspace(0, freq * np.pi, w)
    xx, yy = np.meshgrid(x, y)
    spatial = (
        np.sin(xx * 0.7 + 1.3) * np.cos(yy * 0.5 + 0.7)
        + np.sin(xx * 0.3 + 2.1) * np.cos(yy * 0.9 + 1.1)
    )
    spatial = spatial / spatial.std() * scenario.background_std_k

    # Sensor noise
    noise = rng.normal(0, scenario.sensor_nedt_k, (h, w))

    return scenario.background_temp_k + spatial + noise


def inject_fires(
    bt: np.ndarray,
    scenario: Scenario,
    pass_num: int,
) -> tuple[np.ndarray, list[dict]]:
    """Inject fire hotspots into a BT raster. Returns (modified BT, fire metadata)."""
    h, w = bt.shape
    west, south, east, north = scenario.aoi
    lon_per_px = (east - west) / w
    lat_per_px = (north - south) / h

    metadata = []
    for fire in scenario.fires:
        if pass_num < fire.onset_pass:
            continue

        # Convert geo coords to pixel coords
        col = (fire.lon - west) / lon_per_px
        row = (north - fire.lat) / lat_per_px

        if not (0 <= col < w and 0 <= row < h):
            continue

        radius = fire.radius_at(pass_num)
        temp = fire.temp_at(pass_num)

        # Create circular fire footprint with radial falloff
        yy, xx = np.ogrid[:h, :w]
        dist = np.sqrt((xx - col) ** 2 + (yy - row) ** 2)
        mask = dist < radius

        # Core is peak temp, edges fall off
        falloff = np.clip(1.0 - dist / radius, 0, 1)
        fire_contribution = falloff * (temp - scenario.background_temp_k)
        bt = np.where(mask, bt + fire_contribution, bt)

        metadata.append({
            "lat": fire.lat,
            "lon": fire.lon,
            "radius_px": radius,
            "temp_k": temp,
            "pass": pass_num,
        })

    return bt, metadata


def generate_pass(
    scenario: Scenario,
    pass_num: int,
    seed: int | None = None,
) -> tuple[np.ndarray, list[dict]]:
    """Generate a single-pass BT raster with fire injections."""
    rng = np.random.default_rng(seed)
    bt = generate_background(scenario, rng)
    bt, fire_meta = inject_fires(bt, scenario, pass_num)
    return bt.astype(np.float32), fire_meta


def write_raster(
    bt: np.ndarray,
    path: str | Path,
    scenario: Scenario,
) -> None:
    """Write a BT raster as a GeoTIFF with geographic coordinates."""
    west, south, east, north = scenario.aoi
    transform = from_bounds(west, south, east, north, bt.shape[1], bt.shape[0])

    with rasterio.open(
        path,
        "w",
        driver="GTiff",
        height=bt.shape[0],
        width=bt.shape[1],
        count=1,
        dtype=bt.dtype,
        crs="EPSG:4326",
        transform=transform,
    ) as dst:
        dst.write(bt, 1)
        dst.update_tags(units="Kelvin", band_name="MWIR_BT_3.9um")


def write_binary(
    bt: np.ndarray,
    path: str | Path,
) -> None:
    """Write a BT raster as a raw binary file for the NOS3 thermal camera sim.

    Format: [u32 width LE] [u32 height LE] [width*height f32 LE values]
    """
    h, w = bt.shape
    with open(path, "wb") as f:
        f.write(np.uint32(w).tobytes())
        f.write(np.uint32(h).tobytes())
        f.write(bt.tobytes())


def generate_scenario(
    scenario: Scenario,
    output_dir: str | Path,
    seed: int = 42,
    fmt: str = "both",
) -> list[dict]:
    """Generate all passes for a scenario. Returns fire metadata per pass.

    fmt: "tif" for GeoTIFF only, "bin" for binary only, "both" for both.
    """
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    all_metadata = []
    for pass_num in range(scenario.num_passes):
        bt, fire_meta = generate_pass(scenario, pass_num, seed=seed + pass_num)
        if fmt in ("tif", "both"):
            write_raster(bt, output_dir / f"pass_{pass_num:04d}.tif", scenario)
        if fmt in ("bin", "both"):
            write_binary(bt, output_dir / f"pass_{pass_num:04d}.bin")
        all_metadata.append({"pass": pass_num, "fires": fire_meta})

    meta_path = output_dir / "metadata.yaml"
    with open(meta_path, "w") as f:
        yaml.dump(all_metadata, f, default_flow_style=False)

    return all_metadata
