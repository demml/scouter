from scouter import Scouter
import numpy as np
from datetime import datetime

scouter = Scouter()

array1 = np.random.rand(100, 2)

# date
time = [datetime.now()] * 100

# add to array
array1 = np.append(array1, time)


if __name__ == "__main__":
    print(array1)
