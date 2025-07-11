from scouter import Feature, Features
from pydantic import BaseModel


class MyFeatures(BaseModel):
    feature_1: int
    feature_2: float
    feature_3: str


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
