from typing import Any, List, Optional, Union

import numpy as np
import pandas as pd
import polars as pl
from numpy.typing import NDArray
from pydantic import BaseModel, ConfigDict
from scouter.utils.logger import ScouterLogger
from scouter.utils.types import DataType

logger = ScouterLogger.get_logger()


class ArrayData(BaseModel):
    string_features: Optional[List[str]] = None
    numeric_features: Optional[List[str]] = None
    numeric_array: Optional[np.ndarray] = None
    string_array: Optional[np.ndarray] = None

    model_config = ConfigDict(arbitrary_types_allowed=True)


class Converter:
    def __init__(self, data: Any):
        self.data = data
        self.numeric_columns: List[str] = []
        self.string_columns: List[str] = []

    def _check_for_non_numeric(self) -> None:
        raise NotImplementedError

    def prepare_data(self) -> ArrayData:
        raise NotImplementedError

    def _convert_numeric(self, array: np.ndarray) -> np.ndarray:
        try:
            dtype = str(array.dtype)

            if dtype in [
                DataType.INT8.value,
                DataType.INT16.value,
                DataType.INT32.value,
                DataType.INT64.value,
            ]:
                logger.warning(
                    "Scouter only supports float32 and float64 arrays. Converting integer array to float32."
                )
                array = array.astype("float32")

            return array
        except KeyError as exc:
            raise ValueError(f"Unsupported data type: {dtype}") from exc


class PandasConverter(Converter):
    def __init__(self, data: pd.DataFrame):
        super().__init__(data)

    def _check_for_non_numeric(self) -> None:
        all_columns = self.data.columns.tolist()

        # Get numeric column names
        self.numeric_columns = self.data.select_dtypes(
            include=[np.number]
        ).columns.tolist()

        self.string_columns = list(set(all_columns) - set(self.numeric_columns))

    def prepare_data(self) -> ArrayData:
        self._check_for_non_numeric()
        array_data = ArrayData()

        if self.numeric_columns:
            array_data.numeric_array = self._convert_numeric(
                self.data[self.numeric_columns].to_numpy()
            )
            array_data.numeric_features = self.numeric_columns

        if self.string_columns:
            array_data.string_array = (
                self.data[self.string_columns].astype(str).to_numpy()
            )
            array_data.string_features = self.string_columns

        return array_data


class PolarsConverter(Converter):
    def __init__(self, data: pl.DataFrame):
        super().__init__(data)
        self.schema = data.schema

    def _check_for_non_numeric(self) -> None:
        for column in self.data.columns:
            if not self.schema[column].is_numeric():
                self.string_columns.append(column)
            else:
                self.numeric_columns.append(column)

    def prepare_data(self) -> ArrayData:
        self._check_for_non_numeric()

        # subset the data to only numeric columns
        array_data = ArrayData()

        if self.numeric_columns:
            array_data.numeric_array = self._convert_numeric(
                self.data[self.numeric_columns].to_numpy()
            )
            array_data.numeric_features = self.numeric_columns

        if self.string_columns:
            array_data.string_array = self.data.select(self.string_columns).to_numpy()
            array_data.string_features = self.string_columns

        return array_data


class NumpyConverter(Converter):
    def __init__(self, data: NDArray):
        super().__init__(data)

    def _check_for_non_numeric(self) -> None:
        assert isinstance(self.data, np.ndarray)
        dtypes = self.data.dtype

        # Do not support mixed types
        if dtypes.kind in "U":
            self.string_columns = [f"feature_{i}" for i in range(self.data.shape[1])]

        else:
            self.numeric_columns = [f"feature_{i}" for i in range(self.data.shape[1])]

    def prepare_data(self) -> ArrayData:
        self._check_for_non_numeric()
        array_data = ArrayData()

        if self.numeric_columns:
            array_data.numeric_array = self._convert_numeric(self.data)
            array_data.numeric_features = self.numeric_columns

        if self.string_columns:
            array_data.string_array = self.data.astype(str)
            array_data.string_features = self.string_columns

        return array_data


def _convert_data_to_array(
    data: Union[pd.DataFrame, pl.DataFrame, NDArray],
) -> ArrayData:
    if isinstance(data, pl.DataFrame):
        return PolarsConverter(data).prepare_data()
    if isinstance(data, pd.DataFrame):
        return PandasConverter(data).prepare_data()
    return NumpyConverter(data).prepare_data()
