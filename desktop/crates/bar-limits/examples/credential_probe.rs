use bar_limits::{Credential, CredentialStore, KeyringStore};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [run_id, operation] = args.as_slice() else {
        return Err("expected synthetic UUID and absent/write/read/delete".into());
    };
    if run_id.len() != 36
        || !run_id.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
    {
        return Err("probe requires a fresh UUID namespace".into());
    }
    let store = KeyringStore::new(
        format!("com.t3codebar.migration-probe.{run_id}"),
        "synthetic-only",
    );
    let expected = Credential::new(39999, &format!("synthetic-probe-{run_id}"))?;
    match operation.as_str() {
        "absent" if store.load()?.is_none() => (),
        "absent" => return Err("synthetic namespace unexpectedly contains a credential".into()),
        "write" if store.load()?.is_none() => store.save(&expected)?,
        "write" => return Err("probe refuses to overwrite an existing entry".into()),
        "read" => {
            let actual = store.load()?.ok_or("synthetic credential missing")?;
            if serde_json::to_value(actual)? != serde_json::to_value(expected)? {
                return Err("synthetic credential mismatch".into());
            }
        }
        "session-present" => {
            let actual = store.load()?.ok_or("synthetic session credential missing")?;
            let expected = Credential::new(actual.port, "synthetic-native-acceptance-token")?;
            if actual.port == 0
                || serde_json::to_value(actual)? != serde_json::to_value(expected)?
            {
                return Err("synthetic session credential mismatch".into());
            }
        }
        "delete" => store.delete()?,
        _ => return Err("unknown probe operation".into()),
    }
    println!(
        "{}",
        serde_json::json!({"schemaVersion":1,"probe":"credential-separate-processes","runId":run_id,"operation":operation,"os":std::env::consts::OS,"verdict":"verified"})
    );
    Ok(())
}
