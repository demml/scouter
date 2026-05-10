from __future__ import annotations

import uuid
from collections.abc import Generator

import pytest
from scouter.client import ScouterClient
from scouter.mock import ScouterTestServer

from .setup import ScouterUserJourney, create_scouter_user_journey


@pytest.fixture()
def fast_agent_eval_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SCOUTER_QUEUE_PUBLISH_INTERVAL_SECS", "1")
    monkeypatch.setenv("GENAI_MAX_RETRIES", "8")
    monkeypatch.setenv("GENAI_TRACE_WAIT_TIMEOUT_SECS", "15")
    monkeypatch.setenv("GENAI_TRACE_RESCHEDULE_DELAY_SECS", "1")


@pytest.fixture()
def scouter_user_journey(
    fast_agent_eval_env: None,
    isolated_server_config,
) -> Generator[ScouterUserJourney, None, None]:
    del fast_agent_eval_env
    run_id = uuid.uuid4().hex[:8]

    with ScouterTestServer(**isolated_server_config):
        journey = create_scouter_user_journey(
            client=ScouterClient(),
            run_id=run_id,
            profile_dir=isolated_server_config["base_path"] / "profiles",
        )
        try:
            yield journey
        finally:
            journey.shutdown()
