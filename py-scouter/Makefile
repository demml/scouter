PROJECT=scouter
PYTHON_VERSION=3.11.2
SOURCE_OBJECTS=python/scouter

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
	maturin develop --extras kafka,rabbitmq,fastapi

setup.docs:
	poetry install --all-extras --with dev --no-root

test.unit:
	poetry run pytest \
		--ignore tests/integration \
		--cov \
		--cov-fail-under=0 \
		--cov-report xml:./coverage.xml \
		--cov-report term 

setup.integration:
	poetry install --all-extras --with dev --no-root
	pip install maturin
	maturin develop --extras kafka,rabbitmq,fastapi 

test.integration:
	poetry run pytest -s tests/integration

poetry.pre.patch:

	poetry version prepatch

poetry.sub.pre.tag:
	$(eval VER = $(shell grep "^version =" pyproject.toml | tr -d '"' | sed "s/^version = //"))
	$(eval TS = $(shell date +%s))
	$(eval REL_CANDIDATE = $(subst a0,rc.$(TS),$(VER)))
	@sed -i "s/$(VER)/$(REL_CANDIDATE)/" pyproject.toml

prep.pre.patch: poetry.pre.patch poetry.sub.pre.tag

publish.docs:
	poetry run pdoc -t docs/docs/api ./python/scouter -o docs/docs/api --docformat google
	rm -f ./python/scouter/_scouter.cpython-*.so
	cd docs && poetry run mkdocs gh-deploy --force
