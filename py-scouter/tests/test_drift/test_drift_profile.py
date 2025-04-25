import numpy as np
import pytest
from scouter import Drifter, SpcDriftProfile


def test_drift_profile_methods(array: np.ndarray):
    drifter = Drifter()
    profile = drifter.create_drift_profile(array)

    profile_dict = profile.model_dump()

    assert isinstance(profile_dict, dict)
    assert profile_dict["features"]["feature_0"]["center"] == pytest.approx(1.5, 0.1)

    new_profile = SpcDriftProfile.model_validate(profile_dict)

    # check if the new profile is the same as the original
    assert new_profile.model_dump() == profile_dict
