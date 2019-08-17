import click

import svdtools
import svdtools.patch


@click.group()
def svdtools_cli():
    pass


@click.command()
@click.argument("yaml-file")
def patch(yaml_file):
    """Patches an SVD file as specified by a YAML file"""
    svdtools.patch.main(yaml_file)


@click.command()
def version():
    """Version of svdtools library and tool."""
    print(svdtools.__version__)


svdtools_cli.add_command(patch)
svdtools_cli.add_command(version)
