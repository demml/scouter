import numpy as np
import pandas as pd
from scouter import DataProfile, DataProfiler  # type: ignore[attr-defined]


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000
    x_train = np.random.normal(-4, 2.0, size=(n, 4))
    col_names = []
    for i in range(0, x_train.shape[1]):
        col_names.append(f"feature_{i}")
    x = pd.DataFrame(x_train, columns=col_names)

    # create string column (with 10 unique values)
    x["categorical_feature"] = np.random.choice(["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"], size=n)

    return x


data = generate_data()

# create data profiler
profiler = DataProfiler()

# create data profile
profile: DataProfile = profiler.create_data_profile(data)

print(profile)
