from pydantic import BaseModel
from scouter.queue import Feature, Features


class MyFeatures(BaseModel):
    feature_1: int
    feature_2: float
    feature_3: str


features = MyFeatures(feature_1=1, feature_2=2.0, feature_3="value").model_dump()


def create_features_static_methods():
    """Method 1: Using static methods"""
    return Features(
        [
            Feature.int("feature_4", 3),
            Feature.float("feature_5", 4.0),
            Feature.string("feature_6", "value2"),
        ]
    )


def create_features_constructor():
    """Method 2: Using constructor"""
    return Features(
        [
            Feature("feature_1", 1),
            Feature("feature_2", 2.0),
            Feature("feature_3", "value"),
        ]
    )


def create_features_dict():
    """Method 3: Using dictionary from Pydantic model"""
    return Features(features)


def test_benchmark_static_methods(benchmark):
    """Benchmark Features creation using static methods"""
    result = benchmark(create_features_static_methods)
    assert result is not None


def test_benchmark_constructor(benchmark):
    """Benchmark Features creation using constructor"""
    result = benchmark(create_features_constructor)
    assert result is not None


def test_benchmark_dict(benchmark):
    """Benchmark Features creation using dictionary"""
    result = benchmark(create_features_dict)
    assert result is not None
