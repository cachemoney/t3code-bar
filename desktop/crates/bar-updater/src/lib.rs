use semver::Version;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::Duration};

pub const INITIAL_DELAY: Duration = Duration::from_secs(10);
pub const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub id: u64,
    pub version: String,
    pub tag: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Failure {
    Network,
    BadResponse,
    RateLimited,
    Install,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Unsupported(String),
    Idle,
    Checking,
    Available(Offer),
    Installing(Offer),
    Failed(Failure),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Settings {
    pub automatic: bool,
    pub install_without_asking: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckKind {
    Manual,
    Automatic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    Prompt(Offer),
    Install(Offer),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckOutcome {
    pub effect: Effect,
    pub retry_after: Option<Duration>,
    pub cancel_retry: bool,
}

pub struct Controller {
    state: State,
    running: Version,
    next_id: u64,
    checking: Option<CheckKind>,
    failures: usize,
    announced: HashSet<String>,
    failed_installs: HashSet<String>,
}

impl Controller {
    /// Only packaged builds with a trusted key and a dedicated Tauri endpoint may enable updates.
    pub fn new(running: &str, supported: bool) -> Self {
        let version = Version::parse(running);
        let state = if !supported {
            State::Unsupported("Updater is not configured for this build".into())
        } else if version.is_err() {
            State::Unsupported("Running version is invalid".into())
        } else {
            State::Idle
        };
        Self {
            state,
            running: version.unwrap_or_else(|_| Version::new(0, 0, 0)),
            next_id: 0,
            checking: None,
            failures: 0,
            announced: HashSet::new(),
            failed_installs: HashSet::new(),
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn begin_check(&mut self, kind: CheckKind, settings: Settings) -> bool {
        if kind == CheckKind::Automatic && !settings.automatic {
            return false;
        }
        if !matches!(
            self.state,
            State::Idle | State::Available(_) | State::Failed(_)
        ) {
            return false;
        }
        self.state = State::Checking;
        self.checking = Some(kind);
        true
    }

    /// `release` comes from the Tauri plugin's checked manifest. The host must retain the
    /// corresponding plugin Update until install, and must never reconstruct it from a tag.
    pub fn finish_check(
        &mut self,
        result: Result<Option<(String, String)>, Failure>,
        settings: Settings,
    ) -> CheckOutcome {
        let Some(kind) = self.checking.take() else {
            return CheckOutcome {
                effect: Effect::None,
                retry_after: None,
                cancel_retry: false,
            };
        };
        let result = result.and_then(|release| match release {
            Some((version, tag)) => {
                let parsed = Version::parse(&version).map_err(|_| Failure::BadResponse)?;
                if parsed.to_string() != version
                    || tag != format!("v{version}")
                    || !parsed.pre.is_empty()
                    || !parsed.build.is_empty()
                {
                    return Err(Failure::BadResponse);
                }
                if parsed <= self.running {
                    Ok(None)
                } else {
                    Ok(Some((version, tag)))
                }
            }
            None => Ok(None),
        });
        match result {
            Err(failure) => {
                self.state = State::Failed(failure.clone());
                let automatic = kind == CheckKind::Automatic;
                let retry_after = if automatic && failure != Failure::Install {
                    self.failures += 1;
                    [300, 900, 3600]
                        .get(self.failures - 1)
                        .copied()
                        .map(Duration::from_secs)
                } else {
                    None
                };
                CheckOutcome {
                    effect: Effect::None,
                    retry_after,
                    cancel_retry: automatic,
                }
            }
            Ok(release) => {
                self.failures = 0;
                let mut effect = Effect::None;
                self.state = if let Some((version, tag)) = release {
                    self.next_id = self.next_id.checked_add(1).expect("offer id exhausted");
                    let offer = Offer {
                        id: self.next_id,
                        version,
                        tag,
                    };
                    if kind == CheckKind::Automatic && settings.automatic {
                        if settings.install_without_asking
                            && !self.failed_installs.contains(&offer.version)
                        {
                            effect = Effect::Install(offer.clone());
                        } else if !self.announced.contains(&offer.version) {
                            effect = Effect::Prompt(offer.clone());
                        }
                    }
                    State::Available(offer)
                } else {
                    State::Idle
                };
                CheckOutcome {
                    effect,
                    retry_after: None,
                    cancel_retry: true,
                }
            }
        }
    }

    /// Call only after the prompt actually appears. Later is represented by leaving the offer available.
    pub fn prompt_shown(&mut self, offer: &Offer) {
        if self.state == State::Available(offer.clone()) {
            self.announced.insert(offer.version.clone());
        }
    }

    pub fn begin_install(&mut self, offer: &Offer) -> bool {
        if self.state != State::Available(offer.clone()) {
            return false;
        }
        self.state = State::Installing(offer.clone());
        true
    }

    pub fn finish_install(&mut self, offer: &Offer, success: bool) {
        if self.state != State::Installing(offer.clone()) {
            return;
        }
        if success {
            self.state = State::Idle;
        } else {
            self.failed_installs.insert(offer.version.clone());
            self.state = State::Failed(Failure::Install);
        }
    }
}

/// Verifies the same base64-wrapped Minisign values used by tauri-plugin-updater.
/// The public key is compiled into the host and the signature comes from the checked manifest.
pub fn verify_signed_artifact(bytes: &[u8], public_key: &str, signature: &str) -> bool {
    use base64::Engine;
    let decode = |value: &str| base64::engine::general_purpose::STANDARD.decode(value).ok();
    let Some(key) = decode(public_key).and_then(|value| String::from_utf8(value).ok()) else {
        return false;
    };
    let Some(sig) = decode(signature).and_then(|value| String::from_utf8(value).ok()) else {
        return false;
    };
    let (Ok(key), Ok(sig)) = (
        minisign_verify::PublicKey::decode(&key),
        minisign_verify::Signature::decode(&sig),
    ) else {
        return false;
    };
    key.verify(bytes, &sig, true).is_ok()
}
