from typing import Optional

import pandas as pd
from scouter import (
    Drifter,
    Feature,
    Features,
    PsiDriftConfig,
    PsiDriftProfile,
    DriftType,
    ServerRecords,
    SpcDriftConfig,
    SpcDriftProfile,
    CustomDriftProfile,
    CustomMetric,
)
from scouter.client import DriftRequest, HTTPConfig, ScouterClient, TimeInterval, BinnedPsiFeatureMetrics, BinnedSpcFeatureMetrics
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import ScouterQueue, KafkaConfig
import time

RustyLogger.setup_logging(
    LoggingConfig(log_level=LogLevel.Debug),
)


def test_psi_monitor_pandas_http(
    pandas_dataframe: pd.DataFrame,
    psi_drift_config: PsiDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()
     
    profile: PsiDriftProfile = scouter.create_drift_profile(pandas_dataframe, psi_drift_config)
    client.register_profile(profile)

    queue = ScouterQueue(drift_profile=profile, config=HTTPConfig())
    records = pandas_dataframe.to_dict(orient="records")

    for record in records:
        features = Features(
            features=[
                Feature.float("column_0", record["column_0"]),
                Feature.float("column_1", record["column_1"]),
                Feature.float("column_2", record["column_2"]),
            ]
        )
        queue.insert(features)

    queue.flush()


    binned_records: BinnedPsiFeatureMetrics = client.get_binned_drift(
        DriftRequest(
            name=profile.config.name,
            repository=profile.config.repository,
            version=profile.config.version,
            time_interval=TimeInterval.FifteenMinutes,
            max_data_points=1000,
            drift_type=DriftType.Psi,
        )
    )
    
    assert binned_records is not None
    
  
def test_spc_monitor_pandas_kafka(
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()
     
    profile: SpcDriftProfile = scouter.create_drift_profile(pandas_dataframe, drift_config)
    client.register_profile(profile)

    queue = ScouterQueue(drift_profile=profile, config=KafkaConfig())
    records = pandas_dataframe.to_dict(orient="records")

    for record in records:
        features = Features(
            features=[
                Feature.float("column_0", record["column_0"]),
                Feature.float("column_1", record["column_1"]),
                Feature.float("column_2", record["column_2"]),
            ]
        )
        queue.insert(features)

    queue.flush()
    
     # wait for kafka to process the message
    time.sleep(5)

    binned_records: BinnedSpcFeatureMetrics = client.get_binned_drift(
        DriftRequest(
            name=profile.config.name,
            repository=profile.config.repository,
            version=profile.config.version,
            time_interval=TimeInterval.FifteenMinutes,
            max_data_points=1000,
            drift_type=DriftType.Spc,
        )
    )
    
   
    assert len(binned_records.features["column_0"].values) > 0
    
    

def test_custom_monitor_pandas_rabbitmq(
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()
     
    profile: SpcDriftProfile = scouter.create_drift_profile(pandas_dataframe, drift_config)
    client.register_profile(profile)

    queue = ScouterQueue(drift_profile=profile, config=KafkaConfig())
    records = pandas_dataframe.to_dict(orient="records")

    for record in records:
        features = Features(
            features=[
                Feature.float("column_0", record["column_0"]),
                Feature.float("column_1", record["column_1"]),
                Feature.float("column_2", record["column_2"]),
            ]
        )
        queue.insert(features)

    queue.flush()
    
     # wait for kafka to process the message
    time.sleep(5)

    binned_records: BinnedSpcFeatureMetrics = client.get_binned_drift(
        DriftRequest(
            name=profile.config.name,
            repository=profile.config.repository,
            version=profile.config.version,
            time_interval=TimeInterval.FifteenMinutes,
            max_data_points=1000,
            drift_type=DriftType.Spc,
        )
    )
    
   
    assert len(binned_records.features["column_0"].values) > 0
    