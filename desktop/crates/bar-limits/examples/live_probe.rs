use bar_limits::{
    ConnectionError, Credential, CredentialStore, LimitsConnection, LiveStatus, WindowKind,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
struct Memory(Mutex<Option<Credential>>);
impl CredentialStore for Memory {
    fn load(&self) -> Result<Option<Credential>, ConnectionError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, credential: &Credential) -> Result<(), ConnectionError> {
        *self.0.lock().unwrap() = Some(credential.clone());
        Ok(())
    }
    fn delete(&self) -> Result<(), ConnectionError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = std::env::args()
        .nth(1)
        .ok_or("expected synthetic fixture server port")?
        .parse()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let store = Arc::new(Memory::default());
        let connection = LimitsConnection::with_default_transport(store.clone());
        connection.connect(port, "synthetic-native-acceptance-token").await?;
        let result = tokio::time::timeout(Duration::from_secs(20), async {
            let mut saw_initial = false;
            loop {
                let snapshot = connection.snapshot();
                if snapshot.status == LiveStatus::Live {
                    let used = snapshot.data.as_ref().and_then(|data| data.accounts.iter().find(|account| account.driver == "codex")).and_then(|account| account.windows.iter().find(|window| window.kind == WindowKind::Session)).map(|window| window.used_percent);
                    if used == Some(24.0) { saw_initial = true; }
                    if saw_initial && used == Some(42.0) { break; }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await;
        connection.disconnect().await?;
        result.map_err(|_| "synthetic snapshot/update deadline exceeded")?;
        if store.load()?.is_some() || connection.snapshot().data.is_some() { return Err("disconnect did not clear synthetic data".into()); }
        println!("{}", serde_json::json!({"probe":"live-fixture-transport","os":std::env::consts::OS,"snapshotUsedPercent":24,"updateUsedPercent":42,"credentialCleanupVerified":true,"verdict":"verified"}));
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}
