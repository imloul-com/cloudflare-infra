use std::collections::HashMap;
use std::time::Duration;
use worker::*;

use crate::views::{Counts, Hit};

/// Dedupe keys are kept for today and yesterday so a visitor crossing midnight
/// is not counted twice, then pruned.
const DEDUPE_RETAIN_DAYS: u64 = 2;
const PRUNE_BATCH: usize = 128;

fn count_key(path: &str) -> String {
    format!("c:{path}")
}

/// Zero-padded so the day bucket sorts lexicographically, which lets the prune
/// pass select expired keys with a single ranged list.
fn seen_key(day: u64, token: &str) -> String {
    format!("s:{day:010}:{token}")
}

#[durable_object]
pub struct Counters {
    state: State,
}

impl DurableObject for Counters {
    fn new(state: State, _env: Env) -> Self {
        Self { state }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let hit: Hit = req.json().await?;
        let storage = self.state.storage();

        let current_key = count_key(&hit.path);
        let seen_key = seen_key(hit.day, &hit.token);

        let mut current: u64 = storage.get(&current_key).await?.unwrap_or(0);

        if storage.get::<u8>(&seen_key).await?.is_none() {
            current += 1;
            storage.put(&current_key, current).await?;
            storage.put(&seen_key, 1u8).await?;

            if storage.get_alarm().await?.is_none() {
                storage.set_alarm(Duration::from_secs(86_400)).await?;
            }
        }

        let mut lookup = HashMap::with_capacity(hit.lookup.len());
        for path in hit.lookup {
            let value = if path == hit.path {
                current
            } else {
                storage.get(&count_key(&path)).await?.unwrap_or(0)
            };
            lookup.insert(path, value);
        }

        Response::from_json(&Counts { current, lookup })
    }

    async fn alarm(&self) -> Result<Response> {
        let storage = self.state.storage();
        let today = crate::views::day_bucket(Date::now().as_millis() as f64);
        let cutoff = today.saturating_sub(DEDUPE_RETAIN_DAYS - 1);
        let end = format!("s:{cutoff:010}:");

        let expired = storage
            .list_with_options(ListOptions::new().start("s:").end(&end).limit(PRUNE_BATCH))
            .await?;

        let keys: Vec<String> = expired
            .keys()
            .into_iter()
            .filter_map(|k| k.ok().and_then(|k| k.as_string()))
            .collect();

        let drained = keys.len();
        if drained > 0 {
            storage.delete_multiple(keys).await?;
        }

        // A full batch means there is more to remove; come back promptly.
        // Otherwise the next write re-arms the alarm.
        if drained == PRUNE_BATCH {
            storage.set_alarm(Duration::from_secs(5)).await?;
        }

        Response::ok("pruned")
    }
}

#[cfg(test)]
mod tests {
    use super::{count_key, seen_key};

    #[test]
    fn seen_keys_sort_by_day() {
        let older = seen_key(19_900, "aaaa");
        let newer = seen_key(19_912, "aaaa");
        assert!(older < newer);
        assert!(older.as_str() < "s:0000019912:");
        assert!(newer.as_str() > "s:0000019912:");
    }

    #[test]
    fn count_keys_sort_before_seen_keys() {
        assert!(count_key("/blog/post") < seen_key(0, "aaaa"));
    }
}
