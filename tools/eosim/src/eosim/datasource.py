"""Download and preprocess real satellite imagery for eosim.

Supports multiple data sources:
- NASA Earthdata (MODIS, VIIRS, Landsat) via earthaccess
- Copernicus (Sentinel-1, Sentinel-2, Sentinel-3) via STAC
- FIRMS active fire CSV (lightweight)

Credentials:
- NASA Earthdata: handled by earthaccess (interactive or ~/.netrc)
- Copernicus: via Copernicus Data Space STAC (no auth for search,
  auth for download via cdse tokens)
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import rasterio
from rasterio.transform import from_bounds


# ── Data catalog ─────────────────────────────────────────────

@dataclass
class DatasetInfo:
    """Metadata about a downloadable dataset."""
    name: str
    source: str
    short_name: str
    description: str
    bands: list[str]
    resolution_m: float
    data_type: str


DATASETS: dict[str, DatasetInfo] = {
    # ── NASA Earthdata ──
    "modis-thermal": DatasetInfo(
        name="MODIS L1B 1km",
        source="earthdata",
        short_name="MOD021KM",
        description="MODIS Terra calibrated radiances, 36 bands at 1km",
        bands=["band21_mir", "band31_tir"],
        resolution_m=1000,
        data_type="thermal",
    ),
    "viirs-fire": DatasetInfo(
        name="VIIRS Active Fire",
        source="earthdata",
        short_name="VNP14IMG",
        description="VIIRS 375m active fire product with FRP",
        bands=["fire_mask", "brightness_temp"],
        resolution_m=375,
        data_type="thermal",
    ),
    "landsat-sr": DatasetInfo(
        name="Landsat 8/9 Surface Reflectance",
        source="earthdata",
        short_name="LANDSAT/C2/L2/T1",
        description="Landsat Collection 2 Level-2 surface reflectance + thermal",
        bands=["blue", "green", "red", "nir", "swir1", "swir2", "tir"],
        resolution_m=30,
        data_type="multispectral",
    ),
    # ── Copernicus STAC ──
    "sentinel-2": DatasetInfo(
        name="Sentinel-2 L2A",
        source="stac",
        short_name="sentinel-2-l2a",
        description="Sentinel-2 multispectral surface reflectance, 13 bands",
        bands=["B02_blue", "B03_green", "B04_red", "B08_nir",
               "B11_swir1", "B12_swir2", "B01_coastal", "B09_cirrus"],
        resolution_m=10,
        data_type="multispectral",
    ),
    "sentinel-1": DatasetInfo(
        name="Sentinel-1 GRD",
        source="stac",
        short_name="sentinel-1-grd",
        description="Sentinel-1 SAR Ground Range Detected, C-band",
        bands=["VV", "VH"],
        resolution_m=10,
        data_type="sar",
    ),
    "sentinel-3-thermal": DatasetInfo(
        name="Sentinel-3 SLSTR",
        source="stac",
        short_name="sentinel-3-slstr-lst",
        description="Sentinel-3 Sea and Land Surface Temperature",
        bands=["lst"],
        resolution_m=1000,
        data_type="thermal",
    ),
}


def list_datasets() -> list[DatasetInfo]:
    """List all available datasets."""
    return list(DATASETS.values())


def list_datasets_by_type(data_type: str) -> list[DatasetInfo]:
    """List datasets of a given type (thermal, multispectral, sar)."""
    return [d for d in DATASETS.values() if d.data_type == data_type]


# ── Search & Download ────────────────────────────────────────

def search(
    dataset: str,
    aoi: tuple[float, float, float, float],
    date_range: tuple[str, str],
    max_results: int = 10,
) -> list[Any]:
    """Search for granules/items matching the query.

    Returns a list of results (earthaccess granules or STAC items).
    """
    info = DATASETS[dataset]

    if info.source == "earthdata":
        return _search_earthdata(info.short_name, aoi, date_range, max_results)
    elif info.source == "stac":
        return _search_stac(info.short_name, aoi, date_range, max_results)
    else:
        raise ValueError(f"Unknown source: {info.source}")


def download(
    dataset: str,
    results: list[Any],
    output_dir: str | Path,
) -> list[Path]:
    """Download search results to a local directory."""
    info = DATASETS[dataset]
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    if info.source == "earthdata":
        return _download_earthdata(results, output_dir)
    elif info.source == "stac":
        return _download_stac(results, output_dir, info)
    else:
        raise ValueError(f"Unknown source: {info.source}")


# ── NASA Earthdata backend ───────────────────────────────────

def _search_earthdata(
    short_name: str,
    aoi: tuple[float, float, float, float],
    date_range: tuple[str, str],
    max_results: int,
) -> list:
    import earthaccess
    earthaccess.login(strategy="interactive")
    return earthaccess.search_data(
        short_name=short_name,
        bounding_box=aoi,
        temporal=date_range,
        count=max_results,
    )


def _download_earthdata(results: list, output_dir: Path) -> list[Path]:
    import earthaccess
    files = earthaccess.download(results, str(output_dir))
    return [Path(f) for f in files]


# ── STAC backend (Copernicus, Planetary Computer) ────────────

STAC_CATALOGS = {
    "sentinel-2-l2a": "https://planetarycomputer.microsoft.com/api/stac/v1",
    "sentinel-1-grd": "https://planetarycomputer.microsoft.com/api/stac/v1",
    "sentinel-3-slstr-lst": "https://planetarycomputer.microsoft.com/api/stac/v1",
}


def _search_stac(
    collection: str,
    aoi: tuple[float, float, float, float],
    date_range: tuple[str, str],
    max_results: int,
) -> list:
    from pystac_client import Client
    import planetary_computer

    catalog_url = STAC_CATALOGS.get(
        collection,
        "https://planetarycomputer.microsoft.com/api/stac/v1",
    )

    catalog = Client.open(catalog_url, modifier=planetary_computer.sign_inplace)

    west, south, east, north = aoi
    bbox = [west, south, east, north]
    start, end = date_range
    datetime_str = f"{start}/{end}"

    search_result = catalog.search(
        collections=[collection],
        bbox=bbox,
        datetime=datetime_str,
        max_items=max_results,
    )

    return list(search_result.items())


def _download_stac(
    items: list,
    output_dir: Path,
    info: DatasetInfo,
) -> list[Path]:
    import urllib.request

    downloaded = []
    for item in items:
        item_dir = output_dir / item.id
        item_dir.mkdir(parents=True, exist_ok=True)

        for asset_key, asset in item.assets.items():
            if asset.media_type and "tiff" in asset.media_type.lower():
                out_path = item_dir / f"{asset_key}.tif"
                if not out_path.exists():
                    urllib.request.urlretrieve(asset.href, out_path)
                downloaded.append(out_path)

    return downloaded


# ── FIRMS (active fire CSV) ──────────────────────────────────

FIRMS_BASE = "https://firms.modaps.eosdis.nasa.gov/api/area/csv"


def download_firms_csv(
    aoi: tuple[float, float, float, float],
    date_range: tuple[str, str],
    source: str = "VIIRS_SNPP_NRT",
    api_key: str = "DEMO_KEY",
    output_dir: str | Path = ".",
) -> Path:
    """Download FIRMS active fire CSV.

    Get a free API key at https://firms.modaps.eosdis.nasa.gov/api/area/
    The DEMO_KEY has limited requests.
    """
    import urllib.request

    west, south, east, north = aoi
    start, end = date_range

    url = (
        f"{FIRMS_BASE}/{api_key}/{source}/"
        f"{west},{south},{east},{north}/1/{start}"
    )

    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    out_path = output_dir / f"firms_{source}_{start}_{end}.csv"

    urllib.request.urlretrieve(url, out_path)
    return out_path


# ── Raster utilities ─────────────────────────────────────────

def crop_raster(
    src_path: str | Path,
    aoi: tuple[float, float, float, float],
    output_size: tuple[int, int] = (512, 512),
) -> tuple[np.ndarray, ...]:
    """Crop a georeferenced raster to the AOI and resample."""
    from rasterio.windows import from_bounds as window_from_bounds

    with rasterio.open(src_path) as src:
        west, south, east, north = aoi
        window = window_from_bounds(
            west, south, east, north, src.transform,
        )
        bands = []
        for i in range(1, src.count + 1):
            data = src.read(
                i, window=window,
                out_shape=output_size,
                resampling=rasterio.enums.Resampling.bilinear,
            )
            bands.append(data.astype(np.float32))
        return tuple(bands)


def write_thermal_raster(
    mwir: np.ndarray,
    lwir: np.ndarray | None,
    path: str | Path,
    aoi: tuple[float, float, float, float],
) -> None:
    """Write thermal bands as a GeoTIFF."""
    west, south, east, north = aoi
    h, w = mwir.shape
    transform = from_bounds(west, south, east, north, w, h)
    n_bands = 2 if lwir is not None else 1

    with rasterio.open(
        path, "w", driver="GTiff",
        height=h, width=w, count=n_bands,
        dtype=mwir.dtype, crs="EPSG:4326",
        transform=transform,
    ) as dst:
        dst.write(mwir, 1)
        if lwir is not None:
            dst.write(lwir, 2)
