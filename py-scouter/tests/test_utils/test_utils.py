from scouter import (
    Alert,
    AlertType,
    AlertZone,
    AlertRule,
    CommonCrons,
    Every30Minutes,
    EveryHour,
    Every6Hours,
    Every12Hours,
    EveryDay,
    EveryWeek,
)


def test_kinds():
    alert = Alert(
        kind=AlertType.OutOfBounds.value,
        zone=AlertZone.OutOfBounds.value,
    )

    assert alert.kind == AlertType.OutOfBounds.value
    assert alert.zone == AlertZone.OutOfBounds.value

    alert = Alert(
        kind=AlertType.Consecutive.value,
        zone=AlertZone.Zone1.value,
    )

    assert alert.kind == AlertType.Consecutive.value
    assert alert.zone == AlertZone.Zone1.value

    alert = Alert(
        kind=AlertType.Alternating.value,
        zone=AlertZone.Zone2.value,
    )

    assert alert.kind == AlertType.Alternating.value
    assert alert.zone == AlertZone.Zone2.value

    alert = Alert(
        kind=AlertType.AllGood.value,
        zone=AlertZone.Zone3.value,
    )

    assert alert.kind == AlertType.AllGood.value
    assert alert.zone == AlertZone.Zone3.value

    alert = Alert(
        kind=AlertType.Trend.value,
        zone=AlertZone.NotApplicable.value,
    )

    assert alert.kind == AlertType.Trend.value
    assert alert.zone == AlertZone.NotApplicable.value


def test_alert_rules():
    assert AlertRule().percentage is None
    assert AlertRule().process.rule == "8 16 4 8 2 4 1 1"


def test_crons():
    assert CommonCrons.EVERY_30_MINUTES == Every30Minutes().cron
    assert CommonCrons.EVERY_HOUR == EveryHour().cron
    assert CommonCrons.EVERY_6_HOURS == Every6Hours().cron
    assert CommonCrons.EVERY_12_HOURS == Every12Hours().cron
    assert CommonCrons.EVERY_DAY == EveryDay().cron
    assert CommonCrons.EVERY_WEEK == EveryWeek().cron
