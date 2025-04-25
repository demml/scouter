from typing import Type, get_type_hints, Union, Any
from scouter.queue import Feature, Features


# decorator to add Classes to add to_features() method
def scouter_features(cls: Type[Any]):
    """
    A decorator that adds a to_features method to a class by inspecting its type annotations.

    Args:
        cls: The class to decorate (must be a Pydantic BaseModel)

    Returns:
        The decorated class with an added to_features method
    """

    # Get the type hints of the class
    type_hints = get_type_hints(cls)

    def get_base_type(type_hint: Any) -> Type:
        """Extract the base type from Optional/Union types"""
        if hasattr(type_hint, "__origin__"):
            if type_hint.__origin__ is Union:
                # Handle Optional[Type] which is Union[Type, None]
                types = [t for t in type_hint.__args__ if t is not type(None)]
                if len(types) == 1:
                    return types[0]
            return type_hint.__origin__
        return type_hint

    def to_features(self) -> Features:
        feature_list = []

        for field_name, field_type in type_hints.items():
            value = getattr(self, field_name)

            base_type = get_base_type(field_type)

            # Map Python types to Feature factory methods
            if base_type in (int, bool):  # bool is subclass of int
                feature = Feature.int(field_name, int(value))
            elif base_type is float:
                feature = Feature.float(field_name, float(value))
            elif base_type is str:
                feature = Feature.string(field_name, str(value))
            else:
                # Skip unsupported types instead of raising an error
                continue

            feature_list.append(feature)

        return Features(features=feature_list)

    setattr(cls, "to_features", to_features)

    return cls
