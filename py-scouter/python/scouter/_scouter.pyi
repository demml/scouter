# pylint: disable=invalid-name, too-many-lines

import datetime
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple, Union

from numpy.typing import NDArray

class DriftServerRecord:
    def __init__(
        self,
        name: str,
        repository: str,
        version: str,
        feature: str,
        value: float,
    ):
        """Initialize drift server record

        Args:
            name:
                Model name
            repository:
                Model repository
            version:
                Model version
            feature:
                Feature name
            value:
                Feature value
        """

    @property
    def created_at(self) -> datetime.datetime:
        """Return the created at timestamp."""

    @property
    def name(self) -> str:
        """Return the name."""

    @property
    def repository(self) -> str:
        """Return the repository."""

    @property
    def version(self) -> str:
        """Return the version."""

    @property
    def feature(self) -> str:
        """Return the feature."""

    @property
    def value(self) -> float:
        """Return the sample value."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def to_dict(self) -> Dict[str, str]:
        """Return the dictionary representation of the record."""

class Every30Minutes:
    def __init__(self) -> None:
        """Initialize the cron schedule"""

    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class EveryHour:
    def __init__(self) -> None:
        """Initialize the cron schedule"""

    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class Every6Hours:
    def __init__(self) -> None:
        """Initialize the cron schedule"""

    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class Every12Hours:
    def __init__(self) -> None:
        """Initialize the cron schedule"""

    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class EveryDay:
    def __init__(self) -> None:
        """Initialize the cron schedule"""

    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class EveryWeek:
    def __init__(self) -> None:
        """Initialize the cron schedule"""

    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class CommonCron:
    def __init__(self) -> None:
        """Initialize the common cron class from rust"""

    @property
    def EVERY_30_MINUTES(self) -> str:
        """Every 30 minutes cron schedule"""

    @property
    def EVERY_HOUR(self) -> str:
        """Every hour cron schedule"""

    @property
    def EVERY_6_HOURS(self) -> str:
        """Every 6 hours cron schedule"""

    @property
    def EVERY_12_HOURS(self) -> str:
        """Every 12 hours cron schedule"""

    @property
    def EVERY_DAY(self) -> str:
        """Every day cron schedule"""

    @property
    def EVERY_WEEK(self) -> str:
        """Every week cron schedule"""

class PercentageAlertRule:
    def __init__(self, rule: Optional[float] = None) -> None:
        """Initialize alert rule

        Args:
            rule:
                Rule to use for percentage alerting (float)
        """

    @property
    def rule(self) -> float:
        """Return the alert rule"""

    @rule.setter
    def rule(self, rule: float) -> None:
        """Set the alert rule"""

class ProcessAlertRule:
    def __init__(
        self,
        rule: Optional[str] = None,
        zones_to_monitor: Optional[List[str]] = None,
    ) -> None:
        """Initialize alert rule

        Args:
            rule:
                Rule to use for alerting. Eight digit integer string.
                Defaults to '8 16 4 8 2 4 1 1'
            zones_to_monitor:
                List of zones to monitor. Defaults to all zones.
        """

    @property
    def rule(self) -> str:
        """Return the alert rule"""

    @rule.setter
    def rule(self, rule: str) -> None:
        """Set the alert rule"""

    @property
    def zones_to_monitor(self) -> List[str]:
        """Return the zones to monitor"""

    @zones_to_monitor.setter
    def zones_to_monitor(self, zones_to_monitor: List[str]) -> None:
        """Set the zones to monitor"""

class AlertRule:
    def __init__(
        self,
        percentage_rule: Optional[PercentageAlertRule] = None,
        process_rule: Optional[ProcessAlertRule] = None,
    ) -> None:
        """Initialize alert rule

        Args:
            rule:
                Rule to use for alerting.
        """

    @property
    def process(self) -> Optional[ProcessAlertRule]:
        """Return the control alert rule"""

    @process.setter
    def process(self, process: ProcessAlertRule) -> None:
        """ "Set the control alert rule"""

    @property
    def percentage(self) -> Optional[PercentageAlertRule]:
        """Return the percentage alert rule"""

    @percentage.setter
    def percentage(self, percentage: PercentageAlertRule) -> None:
        """Set the percentage alert rule"""

class AlertDispatchType(str, Enum):
    Email = "Email"
    Console = "Console"
    Slack = "Slack"
    OpsGenie = "OpsGenie"

class AlertConfig:
    def __init__(
        self,
        alert_rule: Optional[AlertRule] = None,
        alert_dispatch_type: Optional[AlertDispatchType] = None,
        schedule: Optional[str] = None,
        features_to_monitor: Optional[List[str]] = None,
        alert_kwargs: Optional[Dict[str, Any]] = None,
    ):
        """Initialize alert config

        Args:
            alert_rule:
                Alert rule to use. Defaults to Standard
            alert_dispatch_type:
                Alert dispatch type to use. Defaults to console
            schedule:
                Schedule to run monitor. Defaults to daily at midnight
            features_to_monitor:
                List of features to monitor. Defaults to empty list, which means all features
            alert_kwargs:
                Additional alert kwargs to pass to the alerting service

                Supported alert_kwargs:
                Slack:
                    - channel: str (channel to send slack message)
                OpsGenie:
                    - team: str (team to send opsgenie message)
                    - priority: str (priority for opsgenie alerts)

        """

    @property
    def alert_dispatch_type(self) -> str:
        """Return the alert dispatch type"""

    @alert_dispatch_type.setter
    def alert_dispatch_type(self, alert_dispatch_type: str) -> None:
        """Set the alert dispatch type"""

    @property
    def alert_rule(self) -> AlertRule:
        """Return the alert rule"""

    @alert_rule.setter
    def alert_rule(self, alert_rule: AlertRule) -> None:
        """Set the alert rule"""

    @property
    def schedule(self) -> str:
        """Return the schedule"""

    @schedule.setter
    def schedule(self, schedule: str) -> None:
        """Set the schedule"""

    @property
    def features_to_monitor(self) -> List[str]:
        """Return the features to monitor"""

    @features_to_monitor.setter
    def features_to_monitor(self, features_to_monitor: List[str]) -> None:
        """Set the features to monitor"""

    @property
    def alert_kwargs(self) -> Dict[str, Any]:
        """Return the alert kwargs"""

    @alert_kwargs.setter
    def alert_kwargs(self, alert_kwargs: Dict[str, Any]) -> None:
        """Set the alert kwargs"""

class Alert:
    def __init__(self, alert_type: str, zone: str):
        """Initialize alert"""

    @property
    def kind(self) -> str:
        """Alert kind"""

    @property
    def zone(self) -> str:
        """Zone associated with alert"""

class FeatureAlert:
    @property
    def feature(self) -> str:
        """Return the feature."""

    @property
    def alerts(self) -> List[Alert]:
        """Return the alerts."""

    @property
    def indices(self) -> Dict[Union[str, int], List[List[int]]]:
        """Return the alert indices"""

class FeatureAlerts:
    @property
    def features(self) -> Dict[str, FeatureAlert]:
        """Return the feature alerts."""

class FeatureMap:
    @property
    def features(self) -> Dict[str, Dict[str, int]]:
        """Return the feature map."""

class FeatureDriftProfile:
    @property
    def id(self) -> str:
        """Return the id."""

    @property
    def center(self) -> float:
        """Return the center."""

    @property
    def one_ucl(self) -> float:
        """Return the zone 1 ucl."""

    @property
    def one_lcl(self) -> float:
        """Return the zone 1 lcl."""

    @property
    def two_ucl(self) -> float:
        """Return the zone 2 ucl."""

    @property
    def two_lcl(self) -> float:
        """Return the zone 2 lcl."""

    @property
    def three_ucl(self) -> float:
        """Return the zone 3 ucl."""

    @property
    def three_lcl(self) -> float:
        """Return the zone 3 lcl."""

    @property
    def timestamp(self) -> str:
        """Return the timestamp."""

class DriftConfig:
    def __init__(
        self,
        name: Optional[str] = None,
        repository: Optional[str] = None,
        version: Optional[str] = None,
        sample: bool = True,
        sample_size: int = 25,
        feature_map: Optional[FeatureMap] = None,
        targets: Optional[List[str]] = None,
        alert_config: Optional[AlertConfig] = None,
        config_path: Optional[Path] = None,
    ):
        """Initialize monitor config

        Args:
            name:
                Model name
            repository:
                Model repository
            version:
                Model version. Defaults to 0.1.0
            sample:
                Whether to sample or not
            sample_size:
                Sample size
            feature_map:
                Feature map
            targets:
                List of features that are targets in your dataset.
                This is typically the name of your dependent variable(s).
                This primarily used for monitoring and UI purposes.
            alert_config:
                Alert configuration
            config_path:
                Optional path to load config from.
        """

    @property
    def sample_size(self) -> int:
        """Return the sample size."""

    @sample_size.setter
    def sample_size(self, sample_size: int) -> None:
        """Set the sample size."""

    @property
    def sample(self) -> bool:
        """Whether to sample or not"""

    @sample.setter
    def sample(self, sample: bool) -> None:
        """Set whether to sample or not"""

    @property
    def name(self) -> str:
        """Model Name"""

    @name.setter
    def name(self, name: str) -> None:
        """Set model name"""

    @property
    def repository(self) -> str:
        """Model repository"""

    @repository.setter
    def repository(self, repository: str) -> None:
        """Set model repository"""

    @property
    def version(self) -> str:
        """Model version"""

    @version.setter
    def version(self, version: str) -> None:
        """Set model version"""

    @property
    def feature_map(self) -> Optional[FeatureMap]:
        """Feature map"""

    @feature_map.setter
    def feature_map(self, feature_map: FeatureMap) -> None:
        """Set feature map"""

    @property
    def targets(self) -> List[str]:
        """List of target features to monitor"""

    @targets.setter
    def targets(self, targets: List[str]) -> None:
        """Set list of target features to monitor"""

    @property
    def alert_config(self) -> AlertConfig:
        """Alert configuration"""

    @alert_config.setter
    def alert_config(self, alert_config: AlertConfig) -> None:
        """Set alert configuration"""

    def update_feature_map(self, feature_map: FeatureMap) -> None:
        """Update feature map"""

    @staticmethod
    def load_from_json_file(path: Path) -> "DriftConfig":
        """Load config from json file

        Args:
            path:
                Path to json file to load config from.
        """

    def __str__(self) -> str:
        """Return the string representation of the config."""

    def model_dump_json(self) -> str:
        """Return the json representation of the config."""

    def update_config_args(
        self,
        name: Optional[str] = None,
        repository: Optional[str] = None,
        version: Optional[str] = None,
        sample: Optional[bool] = None,
        sample_size: Optional[int] = None,
        feature_map: Optional[FeatureMap] = None,
        targets: Optional[List[str]] = None,
        alert_config: Optional[AlertConfig] = None,
    ) -> None:
        """Inplace operation that updates config args

        Args:
            name:
                Model name
            repository:
                Model repository
            version:
                Model version
            sample:
                Whether to sample or not
            sample_size:
                Sample size
            feature_map:
                Feature map
            targets:
                List of features that are targets in your dataset.
                This is typically the name of your dependent variable(s).
                This primarily used for monitoring and UI purposes.
            alert_config:
                Alert configuration
        """

class DriftProfile:
    def __init__(
        self,
        features: Dict[str, FeatureDriftProfile],
        config: DriftConfig,
        scouter_version: Optional[str] = None,
    ):
        """Initialize drift profile

        Args:
            features:
                Dictionary of features and their drift profiles
            config:
                Monitor config
            scouter_version:
                version of scouter used to generate profile
        """

    @property
    def scouter_version(self) -> str:
        """Return scouter version used to create DriftProfile"""

    @property
    def features(self) -> Dict[str, FeatureDriftProfile]:
        """Return the list of features."""

    @features.setter
    def features(self, features: Dict[str, FeatureDriftProfile]) -> None:
        """Set the list of features."""

    @property
    def config(self) -> DriftConfig:
        """Return the monitor config."""

    @config.setter
    def config(self, config: DriftConfig) -> None:
        """Set the monitor config."""

    def model_dump_json(self) -> str:
        """Return json representation of drift profile"""

    def model_dump(self) -> Dict[str, Any]:
        """Return dictionary representation of drift profile"""

    def save_to_json(self, path: Optional[Path] = None) -> None:
        """Save drift profile to json file

        Args:
            path:
                Optional path to save the drift profile. If None, outputs to "drift_profile.json.
        """

    @staticmethod
    def model_validate_json(json_string: str) -> "DriftProfile":
        """Load drift profile from json

        Args:
            json_string:
                JSON string representation of the drift profile

        """

    @staticmethod
    def model_validate(data: Dict[str, Any]) -> "DriftProfile":
        """Load drift profile from dictionary

        Args:
            data:
                DriftProfile dictionary
        """

    def update_config_args(
        self,
        name: Optional[str] = None,
        repository: Optional[str] = None,
        version: Optional[str] = None,
        sample: Optional[bool] = None,
        sample_size: Optional[int] = None,
        feature_map: Optional[FeatureMap] = None,
        targets: Optional[List[str]] = None,
        alert_config: Optional[AlertConfig] = None,
    ) -> None:
        """Inplace operation that updates config args

        Args:
            name:
                Model name
            repository:
                Model repository
            version:
                Model version
            sample:
                Whether to sample or not
            sample_size:
                Sample size
            feature_map:
                Feature map
            targets:
                List of features that are targets in your dataset.
                This is typically the name of your dependent variable(s).
                This primarily used for monitoring and UI purposes.
            alert_config:
                Alert configuration
        """

    def __str__(self) -> str:
        """Sting representation of DriftProfile"""

class Distinct:
    @property
    def count(self) -> int:
        """total unqiue value counts"""

    @property
    def percent(self) -> float:
        """percent value uniqueness"""

class Quantiles:
    @property
    def q25(self) -> float:
        """25th quantile"""

    @property
    def q50(self) -> float:
        """50th quantile"""

    @property
    def q75(self) -> float:
        """75th quantile"""

    @property
    def q99(self) -> float:
        """99th quantile"""

class Histogram:
    @property
    def bins(self) -> List[float]:
        """Bin values"""

    @property
    def bin_counts(self) -> List[int]:
        """Bin counts"""

class NumericStats:
    @property
    def mean(self) -> float:
        """Return the mean."""

    @property
    def stddev(self) -> float:
        """Return the stddev."""

    @property
    def min(self) -> float:
        """Return the min."""

    @property
    def max(self) -> float:
        """Return the max."""

    @property
    def distinct(self) -> Distinct:
        """Distinct value counts"""

    @property
    def quantiles(self) -> Quantiles:
        """Value quantiles"""

    @property
    def histogram(self) -> Histogram:
        """Value histograms"""

class CharStats:
    @property
    def min_length(self) -> int:
        """Minimum string length"""

    @property
    def max_length(self) -> int:
        """Maximum string length"""

    @property
    def median_length(self) -> int:
        """Median string length"""

    @property
    def mean_length(self) -> float:
        """Mean string length"""

class WordStats:
    @property
    def words(self) -> Dict[str, Distinct]:
        """Distinct word counts"""

class StringStats:
    @property
    def distinct(self) -> Distinct:
        """Distinct value counts"""

    @property
    def char_stats(self) -> CharStats:
        """Character statistics"""

    @property
    def word_stats(self) -> WordStats:
        """word statistics"""

class FeatureProfile:
    @property
    def id(self) -> str:
        """Return the id."""

    @property
    def numeric_stats(self) -> Optional[NumericStats]:
        """Return the numeric stats."""

    @property
    def string_stats(self) -> Optional[StringStats]:
        """Return the string stats."""

    @property
    def timestamp(self) -> str:
        """Return the timestamp."""

class DataProfile:
    """Data profile of features"""

    @property
    def features(self) -> Dict[str, FeatureProfile]:
        """Returns dictionary of features and their data profiles"""

    def __str__(self) -> str:
        """Return string representation of the data profile"""

    def model_dump_json(self) -> str:
        """Return json representation of data profile"""

    @staticmethod
    def model_validate_json(json_string: str) -> "DataProfile":
        """Load Data profile from json

        Args:
            json_string:
                JSON string representation of the data profile
        """

    def save_to_json(self, path: Optional[Path] = None) -> None:
        """Save data profile to json file

        Args:
            path:
                Optional path to save the data profile. If None, outputs to "data_profile.json.

        """

class FeatureDrift:
    @property
    def samples(self) -> List[float]:
        """Return list of samples"""

    @property
    def drift(self) -> List[float]:
        """Return list of drift values"""

    def __str__(self) -> str:
        """Return string representation of feature drift"""

class DriftMap:
    """Drift map of features"""

    def __init__(self, service_name: Optional[str]) -> None:
        """Initialize data profile

        Args:
            service_name:
                Optional name of service associated with drift map
        """

    @property
    def name(self) -> str:
        """name to associate with drift map"""

    @property
    def repository(self) -> str:
        """Repository to associate with drift map"""

    @property
    def version(self) -> str:
        """Version to associate with drift map"""

    @property
    def features(self) -> Dict[str, FeatureDrift]:
        """Returns dictionary of features and their data profiles"""

    def __str__(self) -> str:
        """Return string representation of data drift"""

    def model_dump_json(self) -> str:
        """Return json representation of data drift"""

    @staticmethod
    def model_validate_json(json_string: str) -> "DriftMap":
        """Load drift map from json file.

        Args:
            json_string:
                JSON string representation of the drift map
        """

    def save_to_json(self, path: Optional[Path] = None) -> None:
        """Save drift map to json file

        Args:
            path:
                Optional path to save the drift map. If None, outputs to "drift_map.json.

        """

    def to_numpy(self) -> Tuple[NDArray, NDArray, List[str]]:
        """Return drift map as a a tuple of sample_array, drift_array and list of features"""

    def to_service_record(self) -> List[DriftServerRecord]:
        """Return drift map as a drift server record"""

class ScouterProfiler:
    def __init__(self) -> None:
        """Instantiate Rust ScouterProfiler class that is
        used to profile data"""

    def create_data_profile_f32(
        self,
        numeric_array: NDArray,
        string_array: List[List[str]],
        numeric_features: List[str],
        string_features: List[str],
        bin_size: int,
    ) -> DataProfile:
        """Create a data profile from a f32 numpy array.

        Args:
            numeric_array:
                Numpy array to profile.
            string_array:
                List of string arrays to profile.
            numeric_features:
                List of numeric feature names.
            string_features:
                List of string feature names.
            bin_size:
                Optional bin size for histograms. Defaults to 20 bins.

        Returns:
            Monitoring profile
        """

    def create_data_profile_f64(
        self,
        numeric_array: NDArray,
        string_array: List[List[str]],
        numeric_features: List[str],
        string_features: List[str],
        bin_size: int,
    ) -> DataProfile:
        """Create a data profile from a f32 numpy array.

        Args:
            numeric_array:
                Numpy array to profile.
            string_array:
                List of string arrays to profile.
            numeric_features:
                List of numeric feature names.
            string_features:
                List of string feature names.
            bin_size:
                Optional bin size for histograms. Defaults to 20 bins.

        Returns:
            Monitoring profile
        """

class ScouterDrifter:
    def __init__(self) -> None:
        """Instantiate Rust ScouterMonitor class that is
        used to create monitoring profiles and compute drifts.
        """

    def convert_strings_to_numpy_f32(
        self,
        features: List[str],
        array: List[List[str]],
        drift_profile: DriftProfile,
    ) -> NDArray[Any]:
        """Convert string array to numpy f32 array

        Args:
            features:
                List of feature names.
            array:
                List of string arrays to convert.
            drift_profile:
                Monitoring profile.
        """

    def convert_strings_to_numpy_f64(
        self,
        features: List[str],
        array: List[List[str]],
        drift_profile: DriftProfile,
    ) -> NDArray[Any]:
        """Convert string array to numpy f64 array

        Args:
            features:
                List of feature names.
            array:
                List of string arrays to convert.
            drift_profile:
                Monitoring profile.
        """

    def create_string_drift_profile(
        self,
        features: List[str],
        array: List[List[str]],
        drift_config: DriftConfig,
    ) -> DriftProfile:
        """Create a monitoring profile from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                List of string arrays to profile.
            drift_config:
                Monitor config.

        Returns:
            Monitoring profile.
        """

    def create_numeric_drift_profile_f32(
        self,
        features: List[str],
        array: NDArray,
        drift_config: DriftConfig,
    ) -> DriftProfile:
        """Create a monitoring profile from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_config:
                Monitor config.

        Returns:
            Monitoring profile.
        """

    def create_numeric_drift_profile_f64(
        self,
        features: List[str],
        array: NDArray,
        drift_config: DriftConfig,
    ) -> DriftProfile:
        """Create a monitoring profile from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_config:
                monitor config.

        Returns:
            Monitoring profile.
        """

    def compute_drift_f32(
        self,
        features: List[str],
        array: NDArray,
        drift_profile: DriftProfile,
    ) -> DriftMap:
        """Compute drift from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_profile:
                Monitoring profile.


        Returns:
            DriftMap
        """

    def compute_drift_f64(
        self,
        features: List[str],
        array: NDArray,
        drift_profile: DriftProfile,
    ) -> DriftMap:
        """Compute drift from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_profile:
                Monitoring profile.


        Returns:
            DriftMap.
        """

    def generate_alerts(
        self,
        drift_array: NDArray,
        features: List[str],
        alert_rule: AlertRule,
    ) -> FeatureAlerts:
        """Generate alerts from a drift array and feature list

        Args:
            drift_array:
                Numpy array of drift values.
            features:
                List of feature names. Must match drift array.
            alert_rule:
                Alert rule to use.

        Returns:
            List of alerts.
        """

    def sample_data_f32(
        self,
        features: List[str],
        array: NDArray,
        drift_profile: DriftProfile,
    ) -> List[DriftServerRecord]:
        """Sample data from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_profile:
                Monitoring profile.

        Returns:
            List of server records
        """

    def sample_data_f64(
        self,
        features: List[str],
        array: NDArray,
        drift_profile: DriftProfile,
    ) -> List[DriftServerRecord]:
        """Sample data from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_profile:
                Monitoring profile.

        Returns:
            List of server records
        """
