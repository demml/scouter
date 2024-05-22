PROJECT=scouter
PYTHON_VERSION=3.11.2
SOURCE_OBJECTS=python/scouter

cargo.format:
	cargo fmt
cargo.lints:
	cargo clippy --workspace --all-targets -- -D warnings
cargo.test:
	cargo test

cargo.bench:
	cargo bench

format.isort:
	poetry run isort ${SOURCE_OBJECTS}
format.black:
	poetry run black ${SOURCE_OBJECTS}
format.ruff:
	poetry run ruff check --silent --exit-zero ${SOURCE_OBJECTS}
format: format.isort format.ruff format.black

lints.format_check:
	poetry run black --check ${SOURCE_OBJECTS}
lints.ruff:
	poetry run ruff check ${SOURCE_OBJECTS}
lints.mypy:
	poetry run mypy ${SOURCE_OBJECTS}
lints.pylint:
	poetry run pylint ${SOURCE_OBJECTS}
lints: lints.ruff lints.pylint lints.mypy
lints.ci: lints.format_check lints.ruff lints.pylint lints.mypy

setup.project:
	poetry install --all-extras --with dev --no-root
	pip install maturin
	maturin develop

test.unit:
	poetry run pytest \
		--cov \
		--cov-fail-under=0 \
		--cov-report xml:./coverage.xml \
		--cov-report term 

poetry.pre.patch:
	poetry version prepatch

poetry.sub.pre.tag:
	$(eval VER = $(shell grep "^version =" pyproject.toml | tr -d '"' | sed "s/^version = //"))
	$(eval TS = $(shell date +%s))
	$(eval REL_CANDIDATE = $(subst a0,rc.$(TS),$(VER)))
	@sed -i "s/$(VER)/$(REL_CANDIDATE)/" pyproject.toml

prep.pre.patch: poetry.pre.patch poetry.sub.pre.tag