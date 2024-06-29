from scouter import MonitorQueue, DriftConfig, DriftProfile, Drifter
from numpy.typing import NDArray
import pytest
import pandas as pd


def test_monitor_pandas(pandas_dataframe: pd.DataFrame, monitor_config: DriftConfig):
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(
        pandas_dataframe, monitor_config
    )

    # assert features are relatively centered

    queue = MonitorQueue(drift_profile=profile)

    records = pandas_dataframe[0:30].to_dict(orient="records")

    for record in records:
        drift_map = queue.insert(record)

        if drift_map:
            break

    a
