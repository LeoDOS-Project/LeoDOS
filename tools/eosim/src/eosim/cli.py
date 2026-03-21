"""CLI for eosim — synthetic Earth observation data generator."""

import click

from eosim.orbit import WalkerConstellation
from eosim.wildfire import Scenario, generate_scenario, generate_constellation_scenario


@click.group()
def main():
    """Generate synthetic sensor data for LeoDOS workflow simulation."""


@main.command()
@click.argument("scenario_file", type=click.Path(exists=True))
@click.option("-o", "--output", default="output", help="Output directory")
@click.option("--seed", default=42, help="Random seed")
@click.option("--fmt", default="both", type=click.Choice(["tif", "bin", "both"]),
              help="Output format: tif (GeoTIFF), bin (raw binary for NOS3), both")
def wildfire(scenario_file: str, output: str, seed: int, fmt: str):
    """Generate thermal IR rasters for wildfire detection (single AOI)."""
    scenario = Scenario.from_yaml(scenario_file)
    metadata = generate_scenario(scenario, output, seed=seed, fmt=fmt)

    total_fires = sum(len(m["fires"]) for m in metadata)
    click.echo(f"Generated {scenario.num_passes} passes in {output}/")
    click.echo(f"  {total_fires} fire detections across all passes")
    click.echo(f"  {len(scenario.fires)} fire event(s) defined")


@main.command()
@click.argument("scenario_file", type=click.Path(exists=True))
@click.option("-o", "--output", default="output", help="Output directory")
@click.option("--num-orbits", default=3, help="Number of orbital planes")
@click.option("--sats-per-orbit", default=3, help="Satellites per orbital plane")
@click.option("--altitude", default=550.0, help="Orbital altitude in km")
@click.option("--inclination", default=53.0, help="Inclination in degrees")
@click.option("--fov", default=10.0, help="Sensor field of view in degrees")
@click.option("--seed", default=42, help="Random seed")
@click.option("--fmt", default="both", type=click.Choice(["tif", "bin", "both"]),
              help="Output format")
def constellation(
    scenario_file: str,
    output: str,
    num_orbits: int,
    sats_per_orbit: int,
    altitude: float,
    inclination: float,
    fov: float,
    seed: int,
    fmt: str,
):
    """Generate per-satellite thermal IR rasters for a constellation."""
    scenario = Scenario.from_yaml(scenario_file)
    walker = WalkerConstellation(
        num_orbits=num_orbits,
        sats_per_orbit=sats_per_orbit,
        altitude_km=altitude,
        inclination_deg=inclination,
    )

    click.echo(
        f"Constellation: {walker.total_sats} sats "
        f"({num_orbits} orbits x {sats_per_orbit} sats), "
        f"altitude={altitude} km, inc={inclination}°"
    )
    click.echo(f"Orbital period: {walker.period_s:.0f} s")

    metadata = generate_constellation_scenario(
        scenario, walker, output,
        fov_deg=fov, seed=seed, fmt=fmt,
    )

    total_fires = sum(len(m["fires"]) for m in metadata)
    total_rasters = len(metadata)
    click.echo(f"Generated {total_rasters} rasters in {output}/")
    click.echo(f"  {total_fires} fire detections across all sats/passes")


@main.command("list-datasets")
def list_datasets():
    """List all available satellite datasets."""
    from eosim.datasource import DATASETS
    for key, info in DATASETS.items():
        click.echo(f"  {key:20s}  {info.resolution_m:>6.0f}m  {info.data_type:15s}  {info.description}")


@main.command()
@click.argument("dataset", type=str)
@click.option("--aoi", required=True, type=str,
              help="Bounding box: west,south,east,north")
@click.option("--dates", required=True, type=str,
              help="Date range: YYYY-MM-DD,YYYY-MM-DD")
@click.option("-o", "--output", default="data", help="Output directory")
@click.option("--max-results", default=10, help="Maximum items to download")
def download(dataset: str, aoi: str, dates: str, output: str, max_results: int):
    """Download satellite data. Use 'list-datasets' to see options.

    Examples:

      eosim download sentinel-2 --aoi "-122.5,38,-121.5,39" --dates "2020-08-15,2020-08-25"

      eosim download modis-thermal --aoi "-122.5,38,-121.5,39" --dates "2020-08-15,2020-08-25"

      eosim download firms --aoi "-122.5,38,-121.5,39" --dates "2020-08-15,2020-08-25"
    """
    from eosim.datasource import DATASETS, search, download as dl, download_firms_csv

    if dataset == "firms":
        box = tuple(float(x) for x in aoi.split(","))
        date_range = tuple(dates.split(","))
        click.echo(f"Downloading FIRMS fire CSV...")
        path = download_firms_csv(box, date_range, output_dir=output)
        click.echo(f"Saved to {path}")
        return

    if dataset not in DATASETS:
        click.echo(f"Unknown dataset: {dataset}")
        click.echo(f"Available: {', '.join(DATASETS.keys())}, firms")
        return

    info = DATASETS[dataset]
    box = tuple(float(x) for x in aoi.split(","))
    date_range = tuple(dates.split(","))

    click.echo(f"Searching {info.name} for {date_range[0]} to {date_range[1]}...")
    results = search(dataset, box, date_range, max_results)
    click.echo(f"Found {len(results)} items")

    if results:
        files = dl(dataset, results, output)
        click.echo(f"Downloaded {len(files)} files to {output}/")


if __name__ == "__main__":
    main()
