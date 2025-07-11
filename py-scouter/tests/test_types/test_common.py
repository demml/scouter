from scouter import Feature


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
