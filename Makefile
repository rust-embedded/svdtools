.PHONY: example

# setup development environment
setup: update-venv

# example usage
example:
	venv/bin/svd patch example/incomplete-stm32l4x2.yaml

# ensure this passes before commiting
check: check-black check-isort

# automatic code fixes
fix: apply-black apply-isort

check-black:
	black --check svdtools/

apply-black:
	black svdtools/

apply-isort:
	isort -y --recursive svdtools/

check-isort:
	isort --check-only --recursive svdtools/

semi-clean:
	rm -rf **/__pycache__

clean: semi-clean
	rm -rf venv
	rm -rf dist


# Package management

VERSION_FILE := "svdtools/VERSION"
VERSION := $(shell cat $(VERSION_FILE))
tag:
	git tag -a $(VERSION) -m"v$(VERSION)"

build: check
	flit build

publish: check
	flit --repository pypi publish

venv:
	python3 -m venv venv

# re-run if dev or runtime dependencies change,
# or when adding new scripts
update-venv: venv
	venv/bin/pip install -U pip
	venv/bin/pip install -U -r dev-requirements.txt
	venv/bin/flit install --symlink
