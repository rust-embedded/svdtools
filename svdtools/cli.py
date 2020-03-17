import click

import svdtools


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


@click.command()
@click.argument("svd-file")
@click.option(
    "--gaps/--no-gaps",
    default=True,
    help="Whether to print gaps in interrupt number sequence",
)
def interrupts(svd_file, gaps):
    """Print list of all interrupts described by an SVD file."""
    print(svdtools.interrupts.main(svd_file, gaps))


@click.command()
@click.argument("svd-file")
def mmap(svd_file):
    """Generate text-based memory map of an SVD file."""
    print(svdtools.mmap.main(svd_file))


@click.command()
def version():
    """Version of svdtools library and tool."""
    print(svdtools.__version__)


svdtools_cli.add_command(patch)
svdtools_cli.add_command(makedeps)
svdtools_cli.add_command(interrupts)
svdtools_cli.add_command(mmap)
svdtools_cli.add_command(version)
