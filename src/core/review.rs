use chrono::{Datelike, NaiveDate};

use super::model::{Memo, MemoId, ReviewConfig, ReviewScope, tag_matches};
use super::store::Store;
use super::time;

pub struct SeededRng(u64);

impl SeededRng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

pub fn review_pool<'a>(store: &'a Store, cfg: &ReviewConfig) -> Vec<&'a Memo> {
    let today = time::today();
    let cutoff = cfg
        .range
        .days()
        .map(|d| time::start_of_day(today - chrono::Duration::days(d)));
    store
        .active_memos()
        .filter(|m| time::date_of(m.created_at) < today)
        .filter(|m| cutoff.is_none_or(|c| m.created_at >= c))
        .filter(|m| match cfg.scope {
            ReviewScope::All => true,
            ReviewScope::Untagged => m.tags.is_empty(),
            ReviewScope::IncludeTags => cfg
                .tags
                .iter()
                .any(|t| m.tags.iter().any(|mt| tag_matches(mt, t))),
            ReviewScope::ExcludeTags => !cfg
                .tags
                .iter()
                .any(|t| m.tags.iter().any(|mt| tag_matches(mt, t))),
        })
        .collect()
}

/// Deterministic for a given day and `batch`, so the review stays stable until "换一批".
pub fn daily_review(store: &Store, cfg: &ReviewConfig, date: NaiveDate, batch: u32) -> Vec<MemoId> {
    let mut pool: Vec<&Memo> = review_pool(store, cfg);
    pool.sort_by(|a, b| a.id.cmp(&b.id));
    let seed = (date.num_days_from_ce() as u64) << 16 | batch as u64;
    let mut rng = SeededRng::new(seed);
    rng.shuffle(&mut pool);
    pool.into_iter()
        .take(cfg.count.max(1))
        .map(|m| m.id.clone())
        .collect()
}

#[derive(Clone, Debug, Default)]
pub struct WalkSummary {
    pub steps: usize,
    pub chars: usize,
    pub span_days: i64,
    pub earliest: i64,
    pub latest: i64,
    pub biggest_leap: Option<usize>,
}

/// `similarities[i]` is the similarity of the step that led to `path[i + 1]`.
pub fn walk_summary(store: &Store, path: &[MemoId], similarities: &[f32]) -> WalkSummary {
    let memos: Vec<&Memo> = path.iter().filter_map(|id| store.get(id)).collect();
    if memos.is_empty() {
        return WalkSummary::default();
    }
    let earliest = memos.iter().map(|m| m.created_at).min().unwrap();
    let latest = memos.iter().map(|m| m.created_at).max().unwrap();
    let biggest_leap = similarities
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i + 2);
    WalkSummary {
        steps: memos.len(),
        chars: memos.iter().map(|m| m.char_count).sum(),
        span_days: (time::date_of(latest) - time::date_of(earliest)).num_days(),
        earliest,
        latest,
        biggest_leap,
    }
}
