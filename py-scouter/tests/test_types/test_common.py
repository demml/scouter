from pydantic import BaseModel
from scouter.queue import Feature, Features, Metric, Metrics
from scouter.util import FeatureMixin


class MyFeatures(BaseModel, FeatureMixin):
    feature_1: int
    feature_2: float
    feature_3: str


class MyMetrics(BaseModel):
    metric_1: int
    metric_2: float


def test_feature():
    """This is a simple test to ensure Feature can be instantiated in different ways."""

    # Using init
    Feature("feature_1", 1)
    Feature("feature_2", 2.0)
    Feature("feature_3", "value")

    # Using static method
    Feature.int("feature_4", 3)
    Feature.float("feature_5", 4.0)
    Feature.string("feature_6", "value2")


def test_features():
    """This is a simple test to ensure Features can be instantiated in different ways."""

    # First way (pass list of Feature instances, static method)

    Features(
        [
            Feature.int("feature_4", 3),
            Feature.float("feature_5", 4.0),
            Feature.string("feature_6", "value2"),
        ]
    )

    # 2nd way (instantiate Feature with constructor)
    Features(
        [
            Feature("feature_1", 1),
            Feature("feature_2", 2.0),
            Feature("feature_3", "value"),
        ]
    )

    # 3rd way (pass a dictionary of key-value pairs)
    features = MyFeatures(feature_1=1, feature_2=2.0, feature_3="value")
    Features(features.model_dump())

    features.to_features()


def test_metric():
    """This is a simple test to ensure Metric can be instantiated in different ways."""

    # Using init
    Metric("metric_1", 1)
    Metric("metric_2", 2.0)


def test_metrics():
    """This is a simple test to ensure Metrics can be instantiated in different ways."""

    # First way (pass list of Metric instances, static method)
    Metrics(
        [
            Metric("metric_1", 1),
            Metric("metric_2", 2.0),
        ]
    )

    my_metrics = MyMetrics(metric_1=1, metric_2=2.0)
    Metrics(my_metrics.model_dump())
