PROJECT=scouter
PYTHON_VERSION=3.11.2
SOURCE_OBJECTS=python/scouter tests examples

format.isort:
	uv run isort ${SOURCE_OBJECTS}
format.black:
	uv run black ${SOURCE_OBJECTS}
format.ruff:
	uv run ruff check --silent --exit-zero ${SOURCE_OBJECTS}
format: format.isort format.ruff format.black

lints.format_check:
	uv run black --check ${SOURCE_OBJECTS}
lints.ruff:
	uv run ruff check ${SOURCE_OBJECTS}
lints.mypy:
	uv run mypy ${SOURCE_OBJECTS}
lints.pylint:
	uv run pylint ${SOURCE_OBJECTS}
lints: lints.ruff lints.pylint lints.mypy
lints.ci: lints.format_check lints.ruff lints.pylint lints.mypy

setup.project:
	uv sync --all-extras --group dev --group docs
	uv run maturin develop --features server

setup.docs:
	uv sync --all-extras --group dev --group docs

test.unit:
	uv run pytest \
		--ignore tests/integration \
		--cov \
		--cov-fail-under=0 \
		--cov-report xml:./coverage.xml \
		--cov-report term 

test.integration.api:
	uv run pytest -s tests/integration/api

test.integration.queue:
	uv run pytest -s tests/integration/queue

test.integration.client:
	uv run pytest -s tests/integration/client

test.integration: test.integration.queue test.integration.api test.integration.client

uv.pre.patch:

	uv version prepatch

uv.sub.pre.tag:
	$(eval VER = $(shell grep "^version =" pyproject.toml | tr -d '"' | sed "s/^version = //"))
	$(eval TS = $(shell date +%s))
	$(eval REL_CANDIDATE = $(subst a0,rc.$(TS),$(VER)))
	@sed -i "s/$(VER)/$(REL_CANDIDATE)/" pyproject.toml

prep.pre.patch: uv.pre.patch uv.sub.pre.tag

build.docs:
	uv run mkdocs build

serve.docs:
	uv run mkdocs serve

publish.docs:
	uv run mkdocs gh-deploy --force


