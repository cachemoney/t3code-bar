use crate::{Record, Speed};
use rusqlite::{types::ValueRef, Connection, OpenFlags, Row};
use std::{collections::HashMap, path::Path, time::Duration};

fn sqlite_integer(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<i64>> {
    Ok(match row.get_ref(index)? {
        ValueRef::Integer(value) => Some(value),
        _ => None,
    })
}

fn sqlite_blob(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<Vec<u8>>> {
    Ok(match row.get_ref(index)? {
        ValueRef::Blob(value) => Some(value.to_vec()),
        _ => None,
    })
}
#[derive(Clone)]
enum Field {
    Number(u64),
    Bytes(Vec<u8>),
}
#[derive(Default)]
struct Proto(HashMap<u64, Vec<Field>>);
impl Proto {
    fn parse(data: &[u8]) -> Result<Self, ()> {
        fn varint(data: &[u8], pos: &mut usize) -> Result<u64, ()> {
            let mut n = 0;
            for shift in (0..=63).step_by(7) {
                let b = *data.get(*pos).ok_or(())?;
                *pos += 1;
                if shift == 63 && b > 1 {
                    return Err(());
                }
                n |= u64::from(b & 127) << shift;
                if b < 128 {
                    return Ok(n);
                }
            }
            Err(())
        }
        let mut p = Self::default();
        let mut pos = 0;
        while pos < data.len() {
            let tag = varint(data, &mut pos)?;
            if tag <= 7 || tag >= i64::MAX as u64 {
                return Err(());
            }
            let field = tag / 8;
            let wire = tag % 8;
            match wire {
                0 => {
                    let n = varint(data, &mut pos)?;
                    p.0.entry(field).or_default().push(Field::Number(n));
                }
                1 | 2 | 5 => {
                    let len = if wire == 2 {
                        usize::try_from(varint(data, &mut pos)?).map_err(|_| ())?
                    } else if wire == 1 {
                        8
                    } else {
                        4
                    };
                    let end = pos
                        .checked_add(len)
                        .filter(|end| *end <= data.len())
                        .ok_or(())?;
                    if wire == 2 {
                        p.0.entry(field)
                            .or_default()
                            .push(Field::Bytes(data[pos..end].to_vec()));
                    }
                    pos = end;
                }
                _ => return Err(()),
            }
        }
        Ok(p)
    }
    fn bytes(&self, key: u64) -> Option<&[u8]> {
        match self.0.get(&key)?.first()? {
            Field::Bytes(v) => Some(v),
            _ => None,
        }
    }
    fn number(&self, key: u64) -> i64 {
        match self.0.get(&key).and_then(|v| v.first()) {
            Some(Field::Number(n)) if *n <= 9_007_199_254_740_991 => *n as i64,
            _ => 0,
        }
    }
    fn nested(&self, key: u64) -> Result<Self, ()> {
        Self::parse(self.bytes(key).unwrap_or(&[]))
    }
    fn text(&self, key: u64) -> Result<String, ()> {
        self.bytes(key)
            .map(|s| {
                std::str::from_utf8(s)
                    .map(|s| s.trim().to_owned())
                    .map_err(|_| ())
            })
            .unwrap_or(Ok(String::new()))
    }
    fn timestamp(&self) -> Option<f64> {
        let sec = self.number(1);
        (sec > 0).then(|| sec as f64 * 1000.0 + (self.number(2) / 1_000_000) as f64)
    }
}
fn model_id(id: i64) -> Option<&'static str> {
    Some(match id {
        246 => "gemini-2.5-pro",
        312 => "gemini-2.5-flash",
        313 | 329 => "gemini-2.5-flash-thinking",
        330 => "gemini-2.5-flash-lite",
        281 | 282 => "claude-sonnet-4",
        290 | 291 => "claude-opus-4",
        333 | 334 => "claude-sonnet-4-5",
        340 | 341 => "claude-haiku-4-5",
        1026 => "claude-opus-4-6",
        1035 => "claude-sonnet-4-6",
        1016 | 1036 | 1037 => "gemini-3.1-pro",
        1018 | 1084 | 1047 => "gemini-3-flash-preview",
        _ => return None,
    })
}
fn model(name: &str, id: i64) -> String {
    if name.is_empty() {
        return model_id(id).map(str::to_owned).unwrap_or_else(|| {
            if id > 0 {
                format!("antigravity-model-{id}")
            } else {
                String::new()
            }
        });
    }
    let mut s = name.trim().to_lowercase();
    if s.ends_with(')') {
        if let Some(i) = s.rfind('(') {
            s.truncate(i);
            s = s.trim_end().into();
        }
    }
    s = s.replace(' ', "-");
    if let Some(tail) = s.strip_prefix("claude-4") {
        let mut split = tail.splitn(2, '-');
        let version = split.next().unwrap_or("");
        let rest = split.next().unwrap_or("");
        if ["sonnet", "opus", "haiku"]
            .iter()
            .any(|p| rest == *p || rest.starts_with(&format!("{p}-")))
        {
            s = format!("claude-{rest}-4{version}");
        }
    }
    if s.starts_with("claude-") {
        s = s.replace('.', "-");
    }
    s
}
struct Entry {
    index: i64,
    model: String,
    timestamp: Option<f64>,
    usages: Vec<Proto>,
}
fn entry(data: &[u8], index: i64, step: bool) -> Result<Entry, ()> {
    let root = Proto::parse(data)?;
    if !step && root.bytes(1).is_none() {
        return Err(());
    }
    let data = if step { root } else { root.nested(1)? };
    let model_data = if step {
        data.nested(24)?
    } else {
        Proto(data.0.clone())
    };
    let mut usages = Vec::new();
    if let Some(v) = data.bytes(if step { 9 } else { 4 }) {
        usages.push(Proto::parse(v)?);
    }
    for retry in data
        .0
        .get(&(if step { 28 } else { 17 }))
        .into_iter()
        .flatten()
    {
        let Field::Bytes(bytes) = retry else {
            return Err(());
        };
        if let Some(v) = Proto::parse(bytes)?.bytes(2) {
            usages.push(Proto::parse(v)?);
        }
    }
    let name = model_data.text(if step { 12 } else { 19 })?;
    let alt = model_data.text(if step { 8 } else { 21 })?;
    let timestamp = if step {
        match data.nested(8)?.timestamp() {
            Some(timestamp) => Some(timestamp),
            None => data.nested(1)?.timestamp(),
        }
    } else {
        data.nested(9)?.nested(4)?.timestamp()
    };
    Ok(Entry {
        index,
        model: model(
            if name.is_empty() { &alt } else { &name },
            model_data.number(if step { 1 } else { 3 }),
        ),
        timestamp,
        usages,
    })
}
#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub record: Record,
    pub identities: Vec<String>,
    pub quality: u8,
}
pub(crate) fn read(path: &Path) -> Result<(Vec<Candidate>, i64), ()> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )
    .map_err(|_| ())?;
    db.busy_timeout(Duration::from_millis(250))
        .map_err(|_| ())?;
    db.execute_batch("BEGIN DEFERRED").map_err(|_| ())?;
    let table = |name: &str| -> Result<bool, ()> {
        db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
            [name],
            |r| r.get(0),
        )
        .map_err(|_| ())
    };
    let has_steps = table("steps")?;
    let has_generations = table("gen_metadata")?;
    if !has_steps && !has_generations {
        return Err(());
    }
    let mut malformed = 0;
    let mut entries = |sql: &str, step: bool| -> Result<Vec<Entry>, ()> {
        let mut statement = db.prepare(sql).map_err(|_| ())?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    sqlite_integer(r, 0)?,
                    sqlite_blob(r, 1)?,
                    matches!(r.get_ref(1)?, ValueRef::Null),
                ))
            })
            .map_err(|_| ())?;
        let mut values = Vec::new();
        for row in rows {
            let (idx, blob, is_null) = row.map_err(|_| ())?;
            if step && is_null {
                continue;
            }
            match idx
                .zip(blob)
                .and_then(|(idx, blob)| entry(&blob, idx, step).ok())
            {
                Some(v) => values.push(v),
                None => malformed += 1,
            };
        }
        values.sort_by_key(|e| e.index);
        Ok(values)
    };
    let generations = if has_generations {
        entries("SELECT idx,data FROM gen_metadata ORDER BY rowid", false)?
    } else {
        vec![]
    };
    let steps = if has_steps {
        entries("SELECT idx,metadata FROM steps ORDER BY rowid", true)?
    } else {
        vec![]
    };
    let mut trajectory = None;
    if table("trajectory_metadata_blob")? {
        let mut st = db
            .prepare("SELECT data FROM trajectory_metadata_blob")
            .map_err(|_| ())?;
        let rows = st.query_map([], |r| sqlite_blob(r, 0)).map_err(|_| ())?;
        for row in rows {
            match row.map_err(|_| ())? {
                None => malformed += 1,
                Some(blob) if trajectory.is_none() => {
                    match Proto::parse(&blob).and_then(|p| p.nested(2)) {
                        Ok(value) => trajectory = value.timestamp(),
                        Err(()) => malformed += 1,
                    }
                }
                Some(_) => (),
            }
        }
    }
    let fallback = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(chrono::DateTime::<chrono::Utc>::from)
        .map(|d| d.timestamp_millis() as f64)
        .unwrap_or(0.0);
    let session = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let generation_models: HashMap<_, _> = generations
        .iter()
        .map(|e| (e.index, e.model.clone()))
        .collect();
    let mut candidates = Vec::new();
    for (source, entries) in [("step", steps), ("generation", generations)] {
        for (index, e) in entries.iter().enumerate() {
            for (usage_index, u) in e.usages.iter().enumerate() {
                let identities: Result<Vec<_>, ()> = [11, 12, 7]
                    .into_iter()
                    .map(|f| u.text(f).map(|v| (f, v)))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|v| {
                        v.into_iter()
                            .filter(|(_, s)| !s.is_empty())
                            .map(|(f, s)| format!("antigravity:{f}:{s}"))
                            .collect()
                    });
                let identities = match identities {
                    Ok(v) => v,
                    Err(()) => {
                        malformed += 1;
                        continue;
                    }
                };
                let model = model_id(u.number(1))
                    .map(str::to_owned)
                    .or_else(|| {
                        if !e.model.is_empty() {
                            Some(e.model.clone())
                        } else if source == "step" {
                            generation_models
                                .get(&e.index)
                                .filter(|s| !s.is_empty())
                                .cloned()
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| {
                        let name = model("", u.number(1));
                        if name.is_empty() {
                            "antigravity-unknown".into()
                        } else {
                            name
                        }
                    });
                let reasoning = u.number(9);
                let output = u.number(3).max(reasoning.saturating_add(u.number(10)));
                let timestamp = e.timestamp.or(trajectory).unwrap_or(fallback);
                let rate_model = ["-low", "-medium", "-high"]
                    .iter()
                    .find_map(|s| model.strip_suffix(s))
                    .unwrap_or(&model)
                    .to_string();
                let record = Record {
                    timestamp_ms: timestamp,
                    model,
                    rate_model: Some(rate_model),
                    session: session.into(),
                    uncached_input: u.number(2),
                    cached_input: u.number(5),
                    cache_creation: u.number(4),
                    output,
                    reasoning: output.min(reasoning),
                    reported_cost_usd: None,
                    speed: Speed::Standard,
                    dedupe_key: Some(identities.first().cloned().unwrap_or_else(|| {
                        format!("antigravity:{session}:{source}:{index}:{usage_index}")
                    })),
                    unresolved_tier_turn: None,
                };
                if record.processed() > 0 {
                    candidates.push(Candidate {
                        record,
                        identities,
                        quality: if e.timestamp.is_some() {
                            2
                        } else if trajectory.is_some() {
                            1
                        } else {
                            0
                        },
                    });
                }
            }
        }
    }
    Ok((candidates, malformed))
}
pub(crate) fn merge(input: Vec<(Candidate, usize)>) -> Vec<(Record, usize)> {
    let mut candidates: Vec<_> = input.iter().map(|v| v.0.clone()).collect();
    let mut owners: Vec<_> = (0..input.len()).collect();
    let mut parents = owners.clone();
    let mut keys = HashMap::new();
    fn root(parents: &[usize], mut i: usize) -> usize {
        while parents[i] != i {
            i = parents[i];
        }
        i
    }
    for i in 0..candidates.len() {
        for key in candidates[i].identities.clone() {
            if let Some(&previous) = keys.get(&key) {
                let left = root(&parents, i);
                let right = root(&parents, previous);
                if left != right {
                    let first = if owners[left] < owners[right] {
                        left
                    } else {
                        right
                    };
                    let other = if first == left { right } else { left };
                    let best = if candidates[left].quality > candidates[right].quality {
                        left
                    } else if candidates[left].quality < candidates[right].quality {
                        right
                    } else if candidates[left].record.timestamp_ms
                        <= candidates[right].record.timestamp_ms
                    {
                        left
                    } else {
                        right
                    };
                    let mut r = candidates[first].record.clone();
                    if r.model == "antigravity-unknown" {
                        r.model = candidates[other].record.model.clone();
                        r.rate_model = candidates[other].record.rate_model.clone();
                    }
                    r.timestamp_ms = candidates[best].record.timestamp_ms;
                    r.uncached_input = candidates[left]
                        .record
                        .uncached_input
                        .max(candidates[right].record.uncached_input);
                    r.cached_input = candidates[left]
                        .record
                        .cached_input
                        .max(candidates[right].record.cached_input);
                    r.cache_creation = candidates[left]
                        .record
                        .cache_creation
                        .max(candidates[right].record.cache_creation);
                    r.output = candidates[left]
                        .record
                        .output
                        .max(candidates[right].record.output);
                    r.reasoning = candidates[left]
                        .record
                        .reasoning
                        .max(candidates[right].record.reasoning);
                    candidates[left].record = r;
                    candidates[left].quality = candidates[best].quality;
                    owners[left] = owners[first];
                    parents[right] = left;
                }
            }
            keys.insert(key, i);
        }
    }
    candidates
        .into_iter()
        .enumerate()
        .filter_map(|(i, c)| (parents[i] == i).then(|| (c.record, input[owners[i]].1)))
        .collect()
}
