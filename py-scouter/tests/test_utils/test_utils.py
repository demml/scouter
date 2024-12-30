from scouter import (
    AlertZone,
    CommonCrons,
    DriftType,
    Every1Minute,
    Every5Minutes,
    Every6Hours,
    Every12Hours,
    Every15Minutes,
    Every30Minutes,
    EveryDay,
    EveryHour,
    EveryWeek,
    SpcAlert,
    SpcAlertRule,
    SpcAlertType,
)


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


def test_crons():
    assert CommonCrons.EVERY_1_MINUTE == Every1Minute().cron
    assert CommonCrons.EVERY_5_MINUTES == Every5Minutes().cron
    assert CommonCrons.EVERY_15_MINUTES == Every15Minutes().cron
    assert CommonCrons.EVERY_30_MINUTES == Every30Minutes().cron
    assert CommonCrons.EVERY_HOUR == EveryHour().cron
    assert CommonCrons.EVERY_6_HOURS == Every6Hours().cron
    assert CommonCrons.EVERY_12_HOURS == Every12Hours().cron
    assert CommonCrons.EVERY_DAY == EveryDay().cron
    assert CommonCrons.EVERY_WEEK == EveryWeek().cron


def test_drift_type():

    assert DriftType.from_value("Spc") == DriftType.Spc
    assert DriftType.from_value("Psi") == DriftType.Psi
    assert DriftType.from_value("custom") == DriftType.Custom
