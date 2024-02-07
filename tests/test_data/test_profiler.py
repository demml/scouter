from scouter import Profiler
import numpy as np


import time


def test_array():
    import psutil

    array = np.random.rand(10_000, 100)
    cpu_percent = psutil.cpu_percent(interval=1)
    print(f"Array creation CPU Usage: {cpu_percent}%")
    memory_usage = psutil.virtual_memory()
    print(f"Array Memory Usage: {memory_usage.percent}%")

    start = time.time()

    stddev = np.std(array, axis=0)
    means = np.mean(array, axis=0)
    inf = np.isinf(array).sum(axis=0)
    inf_percent = inf / array.shape[0]
    min_ = np.min(array, axis=0)
    max_ = np.max(array, axis=0)
    unique = np.unique(array, axis=0)
    unique_percent = unique / array.shape[0]
    missing = np.isnan(array).sum(axis=0)
    missing_percent = missing / array.shape[0]
    _25th = np.quantile(array, 0.25, axis=0)
    _50th = np.quantile(array, 0.50, axis=0)
    _75th = np.quantile(array, 0.75, axis=0)
    _99th = np.quantile(array, 0.99, axis=0)

    # for col in array.T:
    #    unique = len(np.unique(np.array([1, 1, 0])))
    #    hist = np.histogram(col)

    cpu_percent = psutil.cpu_percent(interval=2)
    print(f"Numpy CPU Usage: {cpu_percent}%")
    memory_usage = psutil.virtual_memory()
    print(f"Numpy Memory Usage: {memory_usage.percent}%")
    print(f"numpy: {time.time() - start}")

    profiler = Profiler()
    start = time.time()
    profiler.create_data_profile(array)
    print(f"rust: {time.time() - start}")
    cpu_percent = psutil.cpu_percent(interval=1)
    print(f"Rust CPU Usage: {cpu_percent}%")
    memory_usage = psutil.virtual_memory()
    print(f"Rust Memory Usage: {memory_usage.percent}%")
    a
