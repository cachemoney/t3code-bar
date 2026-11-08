//! Host-owned adapter. See crates/bar-updater/README.md before wiring commands or timers.
use bar_updater::{CheckKind, CheckOutcome, Controller, Failure, Offer, Settings, State};
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

pub struct HostUpdater<R: Runtime> {
    app: AppHandle<R>,
    controller: Controller,
    endpoint: Option<Url>,
    public_key: Option<&'static str>,
    update: Option<(Offer, Update)>,
    suffix: &'static str,
}

impl<R: Runtime> HostUpdater<R> {
    /// `packaged` must be derived in the host from the actual installed bundle.
    /// Only macOS app bundles and Linux AppImages are supported by this adapter.
    pub fn new(app: AppHandle<R>, packaged: bool) -> Self {
        let endpoint = option_env!("BAR_UPDATER_ENDPOINT")
            .and_then(|s| Url::parse(s).ok())
            .filter(|url| {
                url.scheme() == "https"
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none()
            });
        let public_key = option_env!("BAR_UPDATER_PUBLIC_KEY").filter(|s| !s.trim().is_empty());
        let suffix = if cfg!(target_os = "macos") {
            ".app.tar.gz"
        } else if cfg!(target_os = "linux") {
            ".AppImage.tar.gz"
        } else {
            ""
        };
        let supported = packaged
            && !cfg!(debug_assertions)
            && !suffix.is_empty()
            && endpoint.is_some()
            && public_key.is_some();
        let controller = Controller::new(&app.package_info().version.to_string(), supported);
        Self {
            app,
            controller,
            endpoint,
            public_key,
            update: None,
            suffix,
        }
    }

    pub fn state(&self) -> &State {
        self.controller.state()
    }
    pub fn prompt_shown(&mut self, offer: &Offer) {
        self.controller.prompt_shown(offer);
    }

    pub async fn check(
        &mut self,
        kind: CheckKind,
        settings: impl Fn() -> Settings,
    ) -> Option<CheckOutcome> {
        if !self.controller.begin_check(kind, settings()) {
            return None;
        }
        self.update = None;
        let result = match (&self.endpoint, self.public_key) {
            (Some(endpoint), Some(key)) => match self
                .app
                .updater_builder()
                .pubkey(key)
                .endpoints(vec![endpoint.clone()])
            {
                Ok(builder) => match builder.build() {
                    Ok(updater) => updater.check().await.map_err(classify),
                    Err(error) => Err(classify(error)),
                },
                Err(error) => Err(classify(error)),
            },
            _ => Err(Failure::BadResponse),
        };
        let mut found = None;
        let release = match result {
            Ok(Some(update)) => {
                let tag = update
                    .raw_json
                    .get("tag_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("v{}", update.version));
                if update.download_url.scheme() != "https"
                    || !update.download_url.path().ends_with(self.suffix)
                    || update.signature.is_empty()
                    || tag != format!("v{}", update.version)
                {
                    Err(Failure::BadResponse)
                } else {
                    let pair = (update.version.clone(), tag);
                    found = Some(update);
                    Ok(Some(pair))
                }
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        let outcome = self.controller.finish_check(release, settings());
        if let (State::Available(offer), Some(update)) = (self.controller.state(), found) {
            self.update = Some((offer.clone(), update));
        }
        Some(outcome)
    }

    /// Call only after explicit consent or for the exact `Effect::Install` returned by an automatic check.
    /// The plugin verifies the signature before it calls its platform installer.
    pub async fn install(&mut self, offer: &Offer) -> bool {
        if !matches!(&self.update, Some((current, _)) if current == offer)
            || !self.controller.begin_install(offer)
        {
            return false;
        }
        let (_, update) = self.update.take().expect("offer checked above");
        let success = update.download_and_install(|_, _| {}, || {}).await.is_ok();
        self.controller.finish_install(offer, success);
        if success {
            self.app.restart();
        }
        success
    }
}

fn classify(error: tauri_plugin_updater::Error) -> Failure {
    match error {
        tauri_plugin_updater::Error::Reqwest(_)
        | tauri_plugin_updater::Error::Network(_)
        | tauri_plugin_updater::Error::Io(_) => Failure::Network,
        _ => Failure::BadResponse,
    }
}
