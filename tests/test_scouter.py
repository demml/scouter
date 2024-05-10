from scouter import Scouter
import numpy as np


def test_scouter():
    array = np.random.rand(1_000_000, 200)
    features = [f"feat{i}" for i in range(200)]

    scouter = Scouter(features=features)
    profile = scouter.create_monitoring_profile(array)

    print(len(profile.features))


if __name__ == "__main__":
    test_scouter()
