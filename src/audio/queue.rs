use std::collections::VecDeque;
use rand::RngExt;
use crate::db::models::TrackRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShuffleStyle {
    ReducedRepeat,
    Normal,
    Favffle,
}

impl Default for ShuffleStyle {
    fn default() -> Self {
        Self::ReducedRepeat
    }
}

impl ShuffleStyle {
    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => Self::Normal,
            2 => Self::Favffle,
            _ => Self::ReducedRepeat,
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::ReducedRepeat => 0,
            Self::Normal => 1,
            Self::Favffle => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Off,
    All,
    One,
}

impl Default for LoopMode {
    fn default() -> Self {
        Self::Off
    }
}

impl LoopMode {
    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => Self::All,
            2 => Self::One,
            _ => Self::Off,
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::Off => 0,
            Self::All => 1,
            Self::One => 2,
        }
    }
}

pub struct PlaybackQueue {
    items: Vec<TrackRecord>,
    current_index: Option<usize>,
    shuffle_enabled: bool,
    shuffle_style: ShuffleStyle,
    loop_mode: LoopMode,
    history: VecDeque<i64>, // Ring buffer of track IDs
}

impl Default for PlaybackQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackQueue {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            current_index: None,
            shuffle_enabled: false,
            shuffle_style: ShuffleStyle::ReducedRepeat,
            loop_mode: LoopMode::Off,
            history: VecDeque::new(),
        }
    }

    pub fn set_items(&mut self, items: Vec<TrackRecord>, start_index: Option<usize>) {
        if items.is_empty() {
            self.items.clear();
            self.current_index = None;
            self.history.clear();
            return;
        }

        let start = start_index.unwrap_or(0).min(items.len() - 1);
        let selected_item = items[start].clone();

        let mut ordered_items = Vec::with_capacity(items.len());
        ordered_items.push(selected_item);

        let mut remaining: Vec<TrackRecord> = items
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| *idx != start)
            .map(|(_, item)| item)
            .collect();

        if self.shuffle_enabled {
            let mut rng = rand::rng();
            for i in (1..remaining.len()).rev() {
                let j = rng.random_range(0..=i);
                remaining.swap(i, j);
            }
        }

        ordered_items.extend(remaining);

        self.items = ordered_items;
        self.current_index = Some(0);
        self.history.clear();
        self.record_history(self.items[0].id);
    }

    pub fn current_track(&self) -> Option<&TrackRecord> {
        self.current_index.and_then(|idx| self.items.get(idx))
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[TrackRecord] {
        &self.items
    }

    pub fn set_current_index(&mut self, index: usize) -> Option<&TrackRecord> {
        if index < self.items.len() {
            let selected_item = self.items.remove(index);
            self.items.insert(0, selected_item);
            self.current_index = Some(0);
            self.record_history(self.items[0].id);
            Some(&self.items[0])
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
        self.history.clear();
    }

    /// Clears upcoming tracks in queue, keeping the currently playing track intact at index 0.
    pub fn clear_upcoming(&mut self) {
        if let Some(cur) = self.current_index {
            if cur < self.items.len() {
                let current_song = self.items[cur].clone();
                self.items.clear();
                self.items.push(current_song);
                self.current_index = Some(0);
                return;
            }
        }
        self.items.clear();
        self.current_index = None;
        self.history.clear();
    }

    pub fn remove_at(&mut self, index: usize) -> Option<TrackRecord> {
        if index >= self.items.len() {
            return None;
        }

        let removed = self.items.remove(index);

        if self.items.is_empty() {
            self.current_index = None;
        } else if let Some(cur) = self.current_index {
            if index == cur {
                // If current item is removed, clamp current_index to valid range
                if cur >= self.items.len() {
                    self.current_index = Some(self.items.len() - 1);
                }
            } else if index < cur {
                self.current_index = Some(cur - 1);
            }
        }

        Some(removed)
    }

    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        let len = self.items.len();
        if from >= len || to >= len || from == to {
            return false;
        }

        let item = self.items.remove(from);
        self.items.insert(to, item);

        // Adjust current_index pointer if needed
        if let Some(cur) = self.current_index {
            if cur == from {
                self.current_index = Some(to);
            } else if from < cur && to >= cur {
                self.current_index = Some(cur - 1);
            } else if from > cur && to <= cur {
                self.current_index = Some(cur + 1);
            }
        }

        true
    }

    pub fn reshuffle_upcoming(&mut self) {
        if self.items.len() <= 1 {
            return;
        }

        // If we have a current_index, keep it intact and reshuffle the rest!
        if let Some(cur) = self.current_index {
            let cur_item = self.items[cur].clone();
            let mut remaining: Vec<TrackRecord> = self
                .items
                .iter()
                .enumerate()
                .filter(|(idx, _)| *idx != cur)
                .map(|(_, item)| item.clone())
                .collect();

            let mut rng = rand::rng();
            // Fisher-Yates shuffle on remaining
            for i in (1..remaining.len()).rev() {
                let j = rng.random_range(0..=i);
                remaining.swap(i, j);
            }

            let mut new_items = Vec::with_capacity(self.items.len());
            new_items.push(cur_item);
            new_items.extend(remaining);
            self.items = new_items;
            self.current_index = Some(0);
        } else {
            let mut rng = rand::rng();
            for i in (1..self.items.len()).rev() {
                let j = rng.random_range(0..=i);
                self.items.swap(i, j);
            }
        }
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        self.shuffle_enabled = enabled;
    }

    pub fn is_shuffle(&self) -> bool {
        self.shuffle_enabled
    }

    pub fn set_shuffle_style(&mut self, style: ShuffleStyle) {
        self.shuffle_style = style;
    }

    pub fn shuffle_style(&self) -> ShuffleStyle {
        self.shuffle_style
    }

    pub fn set_loop_mode(&mut self, mode: LoopMode) {
        self.loop_mode = mode;
    }

    pub fn loop_mode(&self) -> LoopMode {
        self.loop_mode
    }

    pub fn next(&mut self) -> Option<&TrackRecord> {
        if self.items.is_empty() {
            return None;
        }

        // Loop One repeats current track
        if self.loop_mode == LoopMode::One {
            return self.current_track();
        }

        if self.items.len() == 1 {
            if self.loop_mode == LoopMode::All {
                return self.current_track();
            } else {
                return None;
            }
        }

        // Rotate: The finished track at index 0 moves to the end of the queue,
        // and index 1 becomes index 0 (the new current playing track)!
        let prev = self.items.remove(0);

        if self.loop_mode == LoopMode::All {
            self.items.push(prev);
        } else {
            // Loop Off: Check if we have more items.
            // If items is not empty, prev is rotated to the back as history/finished,
            // or if we reached the original queue end, we handle standard queue behavior.
            // Putting it at the back keeps all items in the queue structure.
            self.items.push(prev);
        }

        self.current_index = Some(0);
        let track_id = self.items[0].id;
        self.record_history(track_id);
        Some(&self.items[0])
    }

    pub fn previous(&mut self) -> Option<&TrackRecord> {
        if self.items.is_empty() {
            return None;
        }

        if self.loop_mode == LoopMode::One {
            return self.current_track();
        }

        if self.items.len() == 1 {
            return self.current_track();
        }

        // Pop last item and put it at index 0 (rotate backward)
        let last = self.items.pop().unwrap();
        self.items.insert(0, last);

        self.current_index = Some(0);
        let track_id = self.items[0].id;
        self.record_history(track_id);
        Some(&self.items[0])
    }

    pub fn remove_folder_tracks(&mut self, folder_id: i64) -> bool {
        let current_track_removed = self
            .current_track()
            .map(|t| t.folder_id == folder_id)
            .unwrap_or(false);

        let current_id = self.current_track().map(|t| t.id);

        self.items.retain(|t| t.folder_id != folder_id);

        if self.items.is_empty() {
            self.current_index = None;
        } else if let Some(cid) = current_id {
            self.current_index = self.items.iter().position(|t| t.id == cid);
        } else {
            self.current_index = None;
        }

        current_track_removed
    }

    pub fn append_tracks(&mut self, tracks: Vec<TrackRecord>) {
        if tracks.is_empty() {
            return;
        }
        if self.items.is_empty() {
            self.set_items(tracks, Some(0));
        } else {
            self.items.extend(tracks);
        }
    }

    fn record_history(&mut self, track_id: i64) {
        let max_history = (self.items.len() / 4).max(1).min(20);
        self.history.push_back(track_id);
        while self.history.len() > max_history {
            self.history.pop_front();
        }
    }

    pub fn pick_shuffle_index(&self) -> usize {
        let mut rng = rand::rng();
        let n = self.items.len();
        if n <= 1 {
            return 0;
        }

        match self.shuffle_style {
            ShuffleStyle::ReducedRepeat => {
                // Filter out candidates present in recent history
                let candidates: Vec<usize> = (0..n)
                    .filter(|&i| !self.history.contains(&self.items[i].id))
                    .collect();

                if !candidates.is_empty() {
                    let chosen = rng.random_range(0..candidates.len());
                    candidates[chosen]
                } else {
                    // Fallback to random if all items are in history
                    rng.random_range(0..n)
                }
            }
            ShuffleStyle::Normal => {
                rng.random_range(0..n)
            }
            ShuffleStyle::Favffle => {
                // Weighted by play_count + 1
                let weights: Vec<u64> = self.items.iter().map(|t| t.play_count + 1).collect();
                let total_weight: u64 = weights.iter().sum();
                if total_weight == 0 {
                    return rng.random_range(0..n);
                }

                let mut choice = rng.random_range(0..total_weight);
                for (idx, &w) in weights.iter().enumerate() {
                    if choice < w {
                        return idx;
                    }
                    choice -= w;
                }
                0
            }
        }
    }
}
