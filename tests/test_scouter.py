from scouter import Scouter
import numpy as np


def test_scouter():
    array = np.random.rand(10_000, 100)
    features = [f"feat{i}" for i in range(100)]

    scouter = Scouter(features=features)
