# Contributing to demml/scouter

## Welcome

Hello! We're glad and grateful that you're interested in contributing to scouter :tada:! Below you will find the general guidelines for setting up your environment and creating/submitting `pull requests` and `issues`.

## Table of contents

- [Contributing to demml/scouter](#contributing-to-demmlscouter)
  - [Welcome](#welcome)
  - [Table of contents](#table-of-contents)
  - [Submitting Issues](#submitting-issues)
  - [Pull Requests](#pull-requests)
    - [Environment Setup](#environment-setup)
    - [Contributing Changes](#contributing-changes)
    - [Community Guidelines](#community-guidelines)
  - [_Thank you!_](#thank-you)

## Submitting Issues

Documentation issues, bugs, and feature requests are all welcome! We want to make scouter as useful as possible, so please let us know if you find something that doesn't work or if you have an idea for a new feature. To create a new issue, click [here](https://github.com/demml/scouter/issues/new/choose) and select the appropriate issue template.

## Pull Requests

There's always something to improve in scouter, and we want to make it as easy as possible for you to contribute. We welcome all contributions, big or small, and we appreciate your help in making scouter better. The following sections will guide you through the process of contributing to scouter.

### Environment Setup

Scouter uses a Rust backend and exposes a Python API via PyO3. For Python environment management, scouter leverages [uv](https://docs.astral.sh/uv/).

1. Install Rust and Cargo by following the instructions [here](https://www.rust-lang.org/tools/install).
2. Install uv by following the instructions [here](https://docs.astral.sh/uv/getting-started/installation/).
3. Install Python 3.10 or higher (e.g. `uv python install 3.12`).
4. Install Docker (needed for PostgreSQL and server integration tests).

**Ensure everything works**:

From the root directory, start the server to verify your setup:

```console
$ make start.server
```

To make sure the Python client is working, run the unit tests:

```console
$ cd py-scouter
$ make setup.project
$ make test.unit
```

The above will set up the Python environment, build the Python wheel, and run the unit tests.

**You're now ready to start contributing!**

Feel free to explore the makefile and codebase to get a better sense of how tests and lints are run, but the above commands should be enough to get you started.

### Contributing Changes

1. Create a new branch for your addition
   * General naming conventions (we're not picky):
      * `/username/<featureName>`: for features
      * `/username/<fixName>`: for general refactoring or bug fixes
2. Test your changes:
   - Testing Rust changes:
     - Make sure you are in the `scouter` root directory
     - Run `cargo fmt --all` to format the code
     - Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` to run the linter
     - Run `make test.unit` to run unit tests (no Docker needed)
     - Run `make test.needs_sql` for SQL, server, and drift executor tests (requires Docker)
   - Testing Python changes:
     - Make sure you are in the `py-scouter` directory
     - Run `make setup.project` to rebuild the Python wheel after any Rust changes
     - Run `make format` to format the code
     - Run `make lints` to run the linter
     - Run `make test.unit` to run the Python unit tests
3. Submit a Draft Pull Request early and mark it `WIP` so a maintainer knows it's not ready for review just yet.
4. Move the `pull_request` out of draft state.
   * Make sure you fill out the `pull_request` template (included with every `pull_request`)
5. Request review from one of our maintainers (this should happen automatically via `.github/CODEOWNERS`).
6. Get approval. We'll let you know if there are any changes needed.
7. Merge your changes into scouter!

### Community Guidelines

1. Be Kind
   - Working with us should be a fun learning opportunity (for all parties!), and we want it to be a good experience for everyone. Please treat each other with respect.
   - If something looks outdated or incorrect, please let us know! We want to make scouter as useful as possible.
2. Own Your Work
   - Creating a PR for scouter is your first step to becoming a contributor, so make sure that you own your changes.
   - Our maintainers will do their best to respond to you in a timely manner, but we ask the same from you as the contributor.
   - We've added agent resources ([CLAUDE.md](./CLAUDE.md), [.claude/skills/](./.claude/skills/), [.github/instructions/](./.github/instructions/)) to help you understand and navigate the codebase. AI code assistants are absolutely fine to use — the only rule is that code you submit is code you understand and stand behind.

## _Thank you!_
