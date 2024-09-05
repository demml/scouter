# pylint: disable=pointless-statement,broad-exception-caught

from functools import cached_property
from typing import Any, Dict, List, Optional, Union

import numpy as np
from scouter.integrations.base import BaseProducer
from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.producer import DriftRecordProducer
from scouter.utils.logger import ScouterLogger

from ._scouter import (  # pylint: disable=no-name-in-module
    CommonCron,
    DriftProfile,
    DriftServerRecord,
    ScouterDrifter,
)

logger = ScouterLogger.get_logger()

CommonCrons = CommonCron()  # type: ignore


class MonitorQueue:
    def __init__(
        self,
        drift_profile: DriftProfile,
        config: Union[KafkaConfig, HTTPConfig],
    ) -> None:
        """Instantiate a monitoring queue to monitor data drift.

        Args:
            drift_profile:
                Monitoring profile containing feature drift profiles.
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.
        """
        self._monitor = ScouterDrifter()
        self._drift_profile = drift_profile

        self.feature_queue: Dict[str, List[float]] = {feature: [] for feature in self.feature_names}
        self._count = 0

        self._producer = self._get_producer(config)
        self._set_cached_properties()

    def _set_cached_properties(self) -> None:
        """Calls the cached properties to initialize the cache.
        Run during initialization to avoid lazy loading of properties during
        the first call.
        """
        logger.info("Initializing cache")
        self.mapped_features
        self.feature_map
        self.feature_names
        logger.info("Cache initialized")

    @cached_property
    def mapped_features(self) -> List[str]:
        """List of features that will need to be mapped to a numeric representation.
        This is precomputed during drift profile creation.
        """
        if self._drift_profile.config.feature_map is None:
            logger.info("Drift profile does not contain a feature map.")
            return []
        return list(self._drift_profile.config.feature_map.features.keys())

    @cached_property
    def feature_map(self) -> Dict[str, Dict[str, int]]:
        """Feature map from the drift profile. Used to map string values for a
        categorical feature to a numeric representation."""
        if self._drift_profile.config.feature_map is None:
            logger.warning("Feature map not found in drift profile. Returning empty map.")
            return {}
        return self._drift_profile.config.feature_map.features

    @cached_property
    def feature_names(self) -> List[str]:
        """Feature names in the monitoring profile."""
        return list(self._drift_profile.features.keys())

    def _get_producer(self, config: Union[KafkaConfig, HTTPConfig]) -> BaseProducer:
        """Get the producer based on the configuration."""
        return DriftRecordProducer.get_producer(config)

    def insert(self, data: Dict[Any, Any]) -> Optional[List[DriftServerRecord]]:
        """Insert data into the monitoring queue.

        Args:
            data:
                Dictionary of feature values to insert into the monitoring queue.

        Returns:
            List of drift records if the monitoring queue has enough data to compute
        """
        try:
            for feature, value in data.items():
                # attempt to map string values to numeric representation
                # fallback to missing value if not found. This is computed during
                # drift profile creation.
                if feature in self.mapped_features:
                    value = self.feature_map[feature].get(value, self.feature_map[feature]["missing"])

                self.feature_queue[feature].append(value)

            self._count += 1

            if self._count >= self._drift_profile.config.sample_size:
                return self.publish()

            return None

        except KeyError as exc:
            logger.error("Key error: {}", exc)
            return None

        except Exception as exc:
            logger.error("Failed to insert data into monitoring queue: {}. Passing", exc)
            return None

    def _clear_queue(self) -> None:
        """Clear the monitoring queue."""
        self.feature_queue = {feature: [] for feature in self.feature_names}
        self._count = 0

    def publish(self) -> List[DriftServerRecord]:
        """Publish drift records to the monitoring server."""
        try:
            # create array from items
            data = list(self.feature_queue.values())
            array = np.array(data, dtype=np.float64).T

            drift_records = self._monitor.sample_data_f64(self.feature_names, array, self._drift_profile)

            for record in drift_records:
                self._producer.publish(record)

            # clear items
            self._clear_queue()

            return drift_records

        except Exception as exc:
            logger.error("Failed to compute drift: {}", exc)
            raise ValueError(f"Failed to compute drift: {exc}") from exc
