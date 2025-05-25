.PHONY: example

# setup development environment
setup: venv

install-svd2rust-form-rustfmt:
	rustup component add rustfmt
	cargo install svd2rust form

# example usage
example:
	uv run svd patch example/incomplete-stm32l4x2.yaml

# ensure this passes before commiting
check: check-black check-isort

# automatic code fixes
fix: apply-black apply-isort

check-black:
	uv run black --check --diff svdtools/

apply-black:
	uv run black svdtools/

apply-isort:
	uv run isort svdtools/

check-isort:
	uv run isort --check-only svdtools/

semi-clean:
	uv cache clean

clean: semi-clean
	rm -rf .venv
	rm -rf dist


# Package management

VERSION_FILE := "svdtools/VERSION"
VERSION := $(shell cat $(VERSION_FILE))
tag:
	git tag -a $(VERSION) -m"v$(VERSION)"

build: check
	uv build

publish: check
	uv publish

# UV automatically uses a venv located at ./.venv for all commands.
# This venv can be activated to put the tools and project
# in PATH
venv:
	uv venv

# re-run if dev or runtime dependencies change,
# or when adding new scripts
update-venv:
	uv lock
	uv sync
