import pandas as pd
import numpy as np
import random
import string


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 100

    X_train = np.random.normal(-4, 2.0, size=(n, 10))

    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"col_{i}")

    X = pd.DataFrame(X_train, columns=col_names)
    X["col_10"] = [''.join(random.choices(['a', 'b', 'c', 'd'])) for _ in range(n)]
    X["col_11"] = np.random.randint(1, 20, size=(n,))
    X["col_12"] = [''.join(random.choices(['a', 'b', 'c', 'd'])) for _ in range(n)]
    X["col_13"] = X["col_11"] > 10
    X["target"] = np.random.randint(1, 10, size=(n,))
    return X
