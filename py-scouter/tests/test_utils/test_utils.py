from scouter import Alert, AlertType, AlertZone, AlertRules


def test_alert_types():
    alert = Alert(
        alert_type=AlertType.OutOfBounds.value,
        alert_zone=AlertZone.OutOfBounds.value,
    )

    assert alert.alert_type == AlertType.OutOfBounds.value
    assert alert.zone == AlertZone.OutOfBounds.value

    alert = Alert(
        alert_type=AlertType.Consecutive.value,
        alert_zone=AlertZone.Zone1.value,
    )

    assert alert.alert_type == AlertType.Consecutive.value
    assert alert.zone == AlertZone.Zone1.value

    alert = Alert(
        alert_type=AlertType.Alternating.value,
        alert_zone=AlertZone.Zone2.value,
    )

    assert alert.alert_type == AlertType.Alternating.value
    assert alert.zone == AlertZone.Zone2.value

    alert = Alert(
        alert_type=AlertType.AllGood.value,
        alert_zone=AlertZone.Zone3.value,
    )

    assert alert.alert_type == AlertType.AllGood.value
    assert alert.zone == AlertZone.Zone3.value

    alert = Alert(
        alert_type=AlertType.Trend.value,
        alert_zone=AlertZone.NotApplicable.value,
    )

    assert alert.alert_type == AlertType.Trend.value
    assert alert.zone == AlertZone.NotApplicable.value


def test_alert_rules():
    assert AlertRules.Standard.value == "8 16 4 8 2 4 1 1"
