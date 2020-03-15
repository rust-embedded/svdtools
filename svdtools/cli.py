import click

import svdtools
import svdtools.patch


@click.group()
@click.version_option(svdtools.__version__, prog_name="svdtools")
def svdtools_cli():
    pass


@click.command()
@click.argument("yaml-file")
def patch(yaml_file):
    """Patches an SVD file as specified by a YAML file"""
    svdtools.patch.main(yaml_file)


@click.command()
@click.argument("yaml-file")
@click.argument("deps-file")
def makedeps(yaml_file, deps_file):
    """Generate Make dependency file listing dependencies for a YAML file."""
    svdtools.makedeps.main(yaml_file, deps_file)


svdtools_cli.add_command(patch)
svdtools_cli.add_command(makedeps)
