//! When triggers fire, when they run next, and how failures switch them off.
use kortex_flow::triggers::{Config, Kind, MAX_FAILURES, check, min_every};

const MIN: u64 = 60_000;
const H: u64 = 60 * MIN;
// 2026-10-20 12:00:00 UTC
const NOON: u64 = 1_792_497_600_000;

#[test]
fn an_interval_waits_for_its_period_and_never_fires_the_moment_it_is_switched_on() {
    let c = Config::new(Kind::Every { minutes: 30 }, NOON);
    assert!(!c.due(NOON, 0, false));
    assert!(!c.due(NOON + 29 * MIN, 0, false));
    assert!(c.due(NOON + 30 * MIN, 0, false));
    assert_eq!(c.next_run(NOON + 10 * MIN, 0), Some(NOON + 30 * MIN));
    // After a run the clock restarts from that run; a long absence fires once, not many times.
    let mut c = c;
    c.finished(NOON + 5 * H, true, None);
    assert!(!c.due(NOON + 5 * H + 29 * MIN, 0, false));
    assert!(c.due(NOON + 5 * H + 30 * MIN, 0, false));
    assert_eq!(
        c.next_run(NOON + 9 * H, 0),
        Some(NOON + 9 * H),
        "overdue means at the next opportunity"
    );
}

#[test]
fn a_daily_trigger_follows_the_local_clock_and_catches_up_once_after_being_closed() {
    // 08:30 local time in Paris summer time (UTC+2) is 06:30 UTC.
    let paris = 120;
    let mut c = Config::new(
        Kind::Daily {
            hour: 8,
            minute: 30,
        },
        NOON - 2 * H,
    ); // switched on at 10:00 UTC (12:00 local)
    assert!(
        !c.due(NOON, paris, false),
        "today's 08:30 had already passed when it was switched on"
    );
    let next_morning = 1_792_497_600_000 + 18 * H + 30 * MIN; // 2026-10-21 06:30 UTC
    assert!(!c.due(next_morning - MIN, paris, false));
    assert!(c.due(next_morning, paris, false));
    assert_eq!(c.next_run(NOON, paris), Some(next_morning));
    c.finished(next_morning + 3 * MIN, true, None);
    assert!(
        !c.due(next_morning + 10 * H, paris, false),
        "done for today"
    );
    assert_eq!(
        c.next_run(next_morning + 10 * H, paris),
        Some(next_morning + 24 * H)
    );
    // Closed for two days: one catch-up run, then back to normal.
    assert!(c.due(next_morning + 50 * H, paris, false));
    c.finished(next_morning + 50 * H, true, None);
    assert!(!c.due(next_morning + 51 * H, paris, false));
    // The same wall-clock time elsewhere is another instant: 08:30 in UTC-5 is 13:30 UTC.
    let ny = Config::new(
        Kind::Daily {
            hour: 8,
            minute: 30,
        },
        NOON - 2 * H,
    );
    assert!(!ny.due(NOON + H + 29 * MIN, -300, false));
    assert!(ny.due(NOON + H + 30 * MIN, -300, false));
}

#[test]
fn open_fires_once_per_session_and_webhooks_never_by_the_clock() {
    let open = Config::new(Kind::Open, NOON);
    assert!(open.due(NOON, 0, false));
    assert!(!open.due(NOON, 0, true), "already run in this session");
    assert_eq!(open.next_run(NOON, 0), None);
    let hook = Config::new(
        Kind::Webhook {
            port: 50000,
            token: "abc".into(),
        },
        NOON,
    );
    assert!(!hook.due(NOON + 100 * H, 0, false));
    assert_eq!(
        hook.webhook_url().as_deref(),
        Some("http://127.0.0.1:50000/hook/abc")
    );
    assert_eq!(Config::new(Kind::Open, 0).webhook_url(), None);
}

#[test]
fn five_failures_in_a_row_switch_a_trigger_off_and_a_success_resets_the_count() {
    let mut c = Config::new(Kind::Every { minutes: 5 }, 0);
    for i in 1..MAX_FAILURES {
        c.finished(u64::from(i) * 1000, false, Some("http_status (500)".into()));
        assert!(c.enabled, "still on after {i} failures");
    }
    c.finished(10_000, true, None);
    assert_eq!(c.failures, 0);
    for _ in 0..MAX_FAILURES {
        c.finished(20_000, false, Some("boom".into()));
    }
    assert!(!c.enabled, "switched itself off");
    assert!(!c.due(u64::MAX / 2, 0, false) && c.next_run(NOON, 0).is_none());
    assert_eq!(
        c.last.as_ref().and_then(|l| l.error.as_deref()),
        Some("boom")
    );
}

#[test]
fn what_can_be_saved_depends_on_the_device_and_on_the_cards() {
    assert_eq!(
        (
            min_every(false, true),
            min_every(false, false),
            min_every(true, true)
        ),
        (1, 5, 15)
    );
    assert_eq!(
        check(&Kind::Every { minutes: 4 }, false, false),
        Err("interval_too_short")
    );
    assert_eq!(check(&Kind::Every { minutes: 5 }, false, false), Ok(()));
    assert_eq!(
        check(&Kind::Every { minutes: 10 }, true, true),
        Err("interval_too_short"),
        "AI cards run at most every 15 minutes"
    );
    assert_eq!(
        check(&Kind::Every { minutes: 20_000 }, false, true),
        Err("interval_too_short"),
        "at most a week"
    );
    assert_eq!(
        check(
            &Kind::Daily {
                hour: 24,
                minute: 0
            },
            false,
            true
        ),
        Err("bad_time")
    );
    assert_eq!(
        check(
            &Kind::Webhook {
                port: 50000,
                token: "t".into()
            },
            false,
            false
        ),
        Err("webhook_web")
    );
    assert_eq!(
        check(
            &Kind::Webhook {
                port: 50000,
                token: "t".into()
            },
            false,
            true
        ),
        Ok(())
    );
    let json =
        serde_json::to_string(&Config::new(Kind::Daily { hour: 8, minute: 5 }, 1)).expect("json");
    assert!(json.contains("\"kind\":\"daily\""));
    assert_eq!(
        serde_json::from_str::<Config>(&json).expect("round trip"),
        Config::new(Kind::Daily { hour: 8, minute: 5 }, 1)
    );
}
