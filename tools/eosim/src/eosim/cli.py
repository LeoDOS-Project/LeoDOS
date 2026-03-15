"""CLI for eosim — synthetic Earth observation data generator."""

import click

from eosim.wildfire import Scenario, generate_scenario


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
    """Generate thermal IR rasters for wildfire detection."""
    scenario = Scenario.from_yaml(scenario_file)
    metadata = generate_scenario(scenario, output, seed=seed, fmt=fmt)

    total_fires = sum(len(m["fires"]) for m in metadata)
    click.echo(f"Generated {scenario.num_passes} passes in {output}/")
    click.echo(f"  {total_fires} fire detections across all passes")
    click.echo(f"  {len(scenario.fires)} fire event(s) defined")


if __name__ == "__main__":
    main()
