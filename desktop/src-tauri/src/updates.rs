use crate::{emit_snapshot, updater_adapter::HostUpdater, SharedState, UpdateState};
use bar_updater::{CheckKind, Effect, Settings, State};
use std::time::Duration;
use tauri::{Emitter, Manager};

struct PendingPrompt(std::sync::Mutex<Option<bar_updater::Offer>>);

type Updater = tokio::sync::Mutex<HostUpdater<tauri::Wry>>;

fn preferences(app: &tauri::AppHandle) -> Settings {
    let state = app.state::<SharedState>();
    let snapshot = state.snapshot.lock().unwrap();
    Settings {
        automatic: snapshot.settings.automatically_checks_for_updates,
        install_without_asking: snapshot.settings.installs_without_asking,
    }
}
fn publish(app: &tauri::AppHandle, value: &State) {
    let (status, message, version, can_install, busy) = match value {
        State::Unsupported(message) => ("unavailable", message.clone(), None, false, false),
        State::Idle => (
            "idle",
            "T3 Code Bar is up to date.".into(),
            None,
            false,
            false,
        ),
        State::Checking => (
            "checking",
            "Checking for updates…".into(),
            None,
            false,
            true,
        ),
        State::Available(offer) => (
            "available",
            format!("Version {} is available.", offer.version),
            Some(offer.version.clone()),
            true,
            false,
        ),
        State::Installing(offer) => (
            "installing",
            "Installing the signed update…".into(),
            Some(offer.version.clone()),
            false,
            true,
        ),
        State::Failed(_) => (
            "failed",
            "Could not complete the update. Try again manually.".into(),
            None,
            false,
            false,
        ),
    };
    let state = app.state::<SharedState>();
    state.snapshot.lock().unwrap().update = Some(UpdateState {
        status: status.into(),
        message,
        version,
        offer_id: match value {
            State::Available(offer) | State::Installing(offer) => Some(offer.id),
            _ => None,
        },
        can_install,
        busy,
    });
    emit_snapshot(app, &state);
}
pub async fn check(app: &tauri::AppHandle, kind: CheckKind) -> Option<Duration> {
    let mutex = app.state::<Updater>();
    let Ok(mut updater) = mutex.try_lock() else {
        return None;
    };
    if matches!(
        updater.state(),
        State::Idle | State::Available(_) | State::Failed(_)
    ) && (kind == CheckKind::Manual || preferences(app).automatic)
    {
        publish(app, &State::Checking);
    }
    let outcome = updater.check(kind, || preferences(app)).await;
    publish(app, updater.state());
    let outcome = outcome?;
    match outcome.effect {
        Effect::Install(offer) => {
            updater.install(&offer).await;
            publish(app, updater.state());
        }
        Effect::Prompt(offer) => {
            *app.state::<PendingPrompt>().0.lock().unwrap() = Some(offer);
        }
        Effect::None => {}
    }
    drop(updater);
    show_pending(app);
    outcome.retry_after
}
pub async fn install(app: &tauri::AppHandle, offer_id: u64, version: &str) -> Result<(), String> {
    let mutex = app.state::<Updater>();
    let Ok(mut updater) = mutex.try_lock() else {
        return Err("An update operation is already running".into());
    };
    let State::Available(offer) = updater.state().clone() else {
        return Err("No checked update is available".into());
    };
    if !same_offer(&offer, offer_id, version) {
        return Err(
            "The update offer has changed. Review the current version before installing.".into(),
        );
    }
    publish(app, &State::Installing(offer.clone()));
    let success = updater.install(&offer).await;
    publish(app, updater.state());
    if success {
        Ok(())
    } else {
        Err("Update installation failed".into())
    }
}
pub fn initialize(app: &tauri::AppHandle) {
    let state = app.state::<SharedState>();
    let executable = std::env::current_exe().ok();
    let packaged = !state.fixture
        && if cfg!(target_os = "macos") {
            executable.as_ref().is_some_and(|p| {
                p.ancestors()
                    .any(|p| p.extension().is_some_and(|s| s == "app"))
            })
        } else {
            std::env::var_os("APPIMAGE").is_some()
        };
    let updater = HostUpdater::new(app.clone(), packaged);
    publish(app, updater.state());
    app.manage(tokio::sync::Mutex::new(updater));
    app.manage(PendingPrompt(std::sync::Mutex::new(None)));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut delay = bar_updater::INITIAL_DELAY;
        loop {
            tokio::time::sleep(delay).await;
            delay = check(&app, CheckKind::Automatic)
                .await
                .unwrap_or(bar_updater::CHECK_INTERVAL);
        }
    });
}

pub fn show_pending(app: &tauri::AppHandle) {
    if !crate::VISIBLE.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let Some(pending) = app.try_state::<PendingPrompt>() else {
        return;
    };
    let Some(offer) = pending.0.lock().unwrap().clone() else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mutex = app.state::<Updater>();
        let updater = mutex.lock().await;
        if updater.state() != &State::Available(offer) {
            return;
        }
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if crate::VISIBLE.load(std::sync::atomic::Ordering::Acquire) {
                let _ = handle.emit("bar-settings", ());
            }
        });
    });
}
pub async fn acknowledge_prompt(app: &tauri::AppHandle, id: u64, version: &str) {
    if !crate::VISIBLE.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let mutex = app.state::<Updater>();
    let mut updater = mutex.lock().await;
    if !crate::VISIBLE.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let pending = app.state::<PendingPrompt>();
    let mut pending = pending.0.lock().unwrap();
    let Some(offer) = pending.as_ref() else {
        return;
    };
    if !same_offer(offer, id, version) || updater.state() != &State::Available(offer.clone()) {
        return;
    }
    updater.prompt_shown(offer);
    *pending = None;
}

fn same_offer(offer: &bar_updater::Offer, id: u64, version: &str) -> bool {
    offer.id == id && offer.version == version
}
#[cfg(test)]
mod tests {
    #[test]
    fn stale_displayed_offer_cannot_install_a_newly_checked_version() {
        let current = bar_updater::Offer {
            id: 2,
            version: "2.1.0".into(),
            tag: "v2.1.0".into(),
        };
        assert!(!super::same_offer(&current, 1, "2.0.0"));
        assert!(!super::same_offer(&current, 1, "2.1.0"));
        assert!(super::same_offer(&current, 2, "2.1.0"));
    }
}
