use bar_updater::{
    CheckKind::*, Controller, Effect, Failure, Offer, Settings, State, CHECK_INTERVAL,
    INITIAL_DELAY,
};
use std::time::Duration;

fn settings(auto: bool, install: bool) -> Settings {
    Settings {
        automatic: auto,
        install_without_asking: install,
    }
}
fn check(
    c: &mut Controller,
    kind: bar_updater::CheckKind,
    result: Result<Option<(&str, &str)>, Failure>,
    s: Settings,
) -> bar_updater::CheckOutcome {
    assert!(c.begin_check(kind, s));
    c.finish_check(result.map(|r| r.map(|(v, t)| (v.into(), t.into()))), s)
}
fn offered(c: &Controller) -> Offer {
    match c.state() {
        State::Available(o) => o.clone(),
        x => panic!("unexpected {x:?}"),
    }
}

#[test]
fn timing_manual_and_unsupported() {
    assert_eq!(INITIAL_DELAY, Duration::from_secs(10));
    assert_eq!(CHECK_INTERVAL, Duration::from_secs(21600));
    let mut c = Controller::new("1.0.0", true);
    assert!(!c.begin_check(Automatic, settings(false, true)));
    let r = check(
        &mut c,
        Manual,
        Ok(Some(("1.1.0", "v1.1.0"))),
        settings(true, true),
    );
    assert_eq!(r.effect, Effect::None);
    assert!(matches!(c.state(), State::Available(_)));
    let mut c = Controller::new("1.0.0", false);
    assert!(!c.begin_check(Manual, settings(true, true)));
    assert!(matches!(c.state(), State::Unsupported(_)));
}

#[test]
fn prompt_later_and_exact_offer() {
    let mut c = Controller::new("1.0.0", true);
    let s = settings(true, false);
    let first = check(&mut c, Automatic, Ok(Some(("1.1.0", "v1.1.0"))), s);
    let offer = offered(&c);
    assert_eq!(first.effect, Effect::Prompt(offer.clone()));
    let again = check(&mut c, Automatic, Ok(Some(("1.1.0", "v1.1.0"))), s);
    assert!(matches!(again.effect, Effect::Prompt(_)));
    assert!(!c.begin_install(&offer));
    let current = offered(&c);
    c.prompt_shown(&current);
    assert_eq!(
        check(&mut c, Automatic, Ok(Some(("1.1.0", "v1.1.0"))), s).effect,
        Effect::None
    );
    let now = offered(&c);
    assert!(c.begin_install(&now));
    assert!(!c.begin_install(&now));
}

#[test]
fn exact_version_and_tag() {
    let mut c = Controller::new("1.0.0", true);
    let s = settings(true, false);
    for (v, tag) in [
        ("1.2.0", "v1.2.1"),
        ("1.2.0", "1.2.0"),
        ("1.2.0-beta", "v1.2.0-beta"),
    ] {
        check(&mut c, Manual, Ok(Some((v, tag))), s);
        assert_eq!(*c.state(), State::Failed(Failure::BadResponse));
    }
    check(&mut c, Manual, Ok(Some(("1.0.0", "v1.0.0"))), s);
    assert_eq!(*c.state(), State::Idle);
}

#[test]
fn retry_and_install_failure() {
    let mut c = Controller::new("1.0.0", true);
    let s = settings(true, true);
    for expected in [Some(300), Some(900), Some(3600), None] {
        let result = check(&mut c, Automatic, Err(Failure::Network), s);
        assert_eq!(result.retry_after.map(|d| d.as_secs()), expected);
        assert!(result.cancel_retry);
    }
    let result = check(&mut c, Manual, Err(Failure::Network), s);
    assert!(!result.cancel_retry);
    let result = check(&mut c, Automatic, Ok(Some(("1.1.0", "v1.1.0"))), s);
    let offer = offered(&c);
    assert_eq!(result.effect, Effect::Install(offer.clone()));
    assert!(result.cancel_retry);
    assert!(c.begin_install(&offer));
    c.finish_install(&offer, false);
    assert_eq!(*c.state(), State::Failed(Failure::Install));
    let result = check(&mut c, Automatic, Ok(Some(("1.1.0", "v1.1.0"))), s);
    assert!(matches!(result.effect, Effect::Prompt(_)));
    let new_offer = offered(&c);
    assert!(c.begin_install(&new_offer));
}

#[test]
fn wrong_signature_is_rejected() {
    let public_key =
        "RVdRZjZMUkNHOXQ5aTNtbFllY080SXpUNTFUR1Bwdld1Y05TQ2gxQ0JN MFFD".replace(' ', "");
    let signature = "d3Jvbmc=";
    assert!(!bar_updater::verify_signed_artifact(
        b"fixture",
        &public_key,
        signature
    ));
}

#[test]
fn automatic_check_uses_settings_at_completion() {
    let mut controller = Controller::new("1.0.0", true);
    assert!(controller.begin_check(
        bar_updater::CheckKind::Automatic,
        Settings {
            automatic: true,
            install_without_asking: true
        }
    ));
    let outcome = controller.finish_check(
        Ok(Some(("2.0.0".into(), "v2.0.0".into()))),
        Settings {
            automatic: false,
            install_without_asking: false,
        },
    );
    assert_eq!(outcome.effect, Effect::None);
    assert!(matches!(controller.state(), State::Available(_)));
}

#[test]
fn authentic_artifact_verifies_but_modified_bytes_and_other_keys_do_not() {
    let bytes = include_bytes!("fixtures/artifact.txt");
    let signature = include_str!("fixtures/artifact.txt.sig").trim();
    let key = include_str!("fixtures/correct.pub").trim();
    let other = include_str!("fixtures/other.pub").trim();
    assert!(bar_updater::verify_signed_artifact(bytes, key, signature));
    assert!(!bar_updater::verify_signed_artifact(
        b"modified artifact",
        key,
        signature
    ));
    assert!(!bar_updater::verify_signed_artifact(
        bytes, other, signature
    ));
}
