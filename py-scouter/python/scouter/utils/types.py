from enum import StrEnum


class AlertZone(StrEnum):
    Zone1 = "Zone 1"
    Zone2 = "Zone 2"
    Zone3 = "Zone 3"
    OutOfBounds = "Out of Bounds"
    NotApplicable = "NA"


class AlertType(StrEnum):
    OutOfBounds = "Out of Bounds"
    Consecutive = "Consecutive"
    Alternating = "Alternating"
    AllGood = "All Good"
    Trend = "Trend"
