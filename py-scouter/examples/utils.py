import pandas as pd
import numpy as np


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""

    X_train = np.random.normal(-4, 2.0, size=(1000, 10))

    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"col_{i}")

    X = pd.DataFrame(X_train, columns=col_names)
    X["col_11"] = np.random.randint(1, 20, size=(1000, 1))
    X["target"] = np.random.randint(1, 10, size=(1000, 1))

    return X
