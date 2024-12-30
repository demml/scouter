import pytest
from numpy.typing import NDArray
from scouter import Drifter
from scouter._scouter import SpcDriftProfile


def test_drift_profile_methods(array: NDArray):
    drifter = Drifter()
    profile: SpcDriftProfile = drifter.create_drift_profile(array)

    profile_dict = profile.model_dump()

    assert isinstance(profile_dict, dict)
    assert profile_dict["features"]["feature_0"]["center"] == pytest.approx(1.5, 0.1)

    new_profile = SpcDriftProfile.model_validate(profile_dict)

    # check if the new profile is the same as the original
    assert new_profile.model_dump() == profile_dict
