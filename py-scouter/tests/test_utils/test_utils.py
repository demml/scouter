from scouter import (
    SpcAlert,
    SpcAlertType,
    AlertZone,
    SpcAlertRule,
    CommonCrons,
    Every1Minute,
    Every5Minutes,
    Every15Minutes,
    Every30Minutes,
    EveryHour,
    Every6Hours,
    Every12Hours,
    EveryDay,
    EveryWeek,
)


def test_kinds():
    alert = SpcAlert(
        kind=SpcAlertType.OutOfBounds.value,
        zone=SpcAlertType.OutOfBounds.value,
    )

    assert alert.kind == SpcAlertType.OutOfBounds.value
    assert alert.zone == AlertZone.OutOfBounds.value

    alert = SpcAlert(
        kind=SpcAlertType.Consecutive.value,
        zone=AlertZone.Zone1.value,
    )

    assert alert.kind == SpcAlertType.Consecutive.value
    assert alert.zone == AlertZone.Zone1.value

    alert = SpcAlert(
        kind=SpcAlertType.Alternating.value,
        zone=AlertZone.Zone2.value,
    )

    assert alert.kind == SpcAlertType.Alternating.value
    assert alert.zone == AlertZone.Zone2.value

    alert = SpcAlert(
        kind=SpcAlertType.AllGood.value,
        zone=AlertZone.Zone3.value,
    )

    assert alert.kind == SpcAlertType.AllGood.value
    assert alert.zone == AlertZone.Zone3.value

    alert = SpcAlert(
        kind=SpcAlertType.Trend.value,
        zone=AlertZone.NotApplicable.value,
    )

    assert alert.kind == SpcAlertType.Trend.value
    assert alert.zone == AlertZone.NotApplicable.value


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
