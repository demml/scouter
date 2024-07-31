from typing import List, Tuple

import polars as pl
from numpy.typing import NDArray


class PolarsConverter:
    def __init__(self, data: pl.DataFrame):
        self.data = data
        self.schema = data.schema
        self.numeric_columns: List[str] = []
        self.string_columns: List[str] = []

    def _check_for_non_numeric(self) -> None:
        for column in self.data.columns:
            if not self.schema[column].is_numeric():
                self.string_columns.append(column)
            else:
                self.numeric_columns.append(column)

    def prepare_data(self) -> Tuple[NDArray, NDArray]:
        self._check_for_non_numeric()

        # subset the data to only numeric columns
        return (
            self.data.select(self.numeric_columns).to_numpy(),
            self.data.select(self.string_columns).cast(pl.String).to_numpy(),
        )
