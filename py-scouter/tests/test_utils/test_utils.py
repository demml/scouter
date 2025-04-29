from scouter.alert import AlertZone, SpcAlert, SpcAlertRule, SpcAlertType
from scouter.types import DriftType


def test_kinds():
    alert = SpcAlert(
        kind=SpcAlertType.OutOfBounds,
        zone=AlertZone.Zone1,
    )

    assert alert.kind == SpcAlertType.OutOfBounds
    assert alert.zone == AlertZone.Zone1

    alert = SpcAlert(
        kind=SpcAlertType.Consecutive,
        zone=AlertZone.Zone1,
    )

    assert alert.kind == SpcAlertType.Consecutive
    assert alert.zone == AlertZone.Zone1

    alert = SpcAlert(
        kind=SpcAlertType.Alternating,
        zone=AlertZone.Zone2,
    )

    assert alert.kind == SpcAlertType.Alternating
    assert alert.zone == AlertZone.Zone2

    alert = SpcAlert(
        kind=SpcAlertType.AllGood,
        zone=AlertZone.Zone3,
    )

    assert alert.kind == SpcAlertType.AllGood
    assert alert.zone == AlertZone.Zone3

    alert = SpcAlert(
        kind=SpcAlertType.Trend,
        zone=AlertZone.NotApplicable,
    )

    assert alert.kind == SpcAlertType.Trend
    assert alert.zone == AlertZone.NotApplicable


def test_alert_rules():
    assert SpcAlertRule().rule == "8 16 4 8 2 4 1 1"


def test_drift_type():
    assert DriftType.from_value("Spc") == DriftType.Spc
    assert DriftType.from_value("Psi") == DriftType.Psi
    assert DriftType.from_value("custom") == DriftType.Custom
