use chrono::TimeDelta;
type DateTime = chrono::DateTime<chrono::Utc>;

#[allow(dead_code)]
pub trait TimeDeltaExtension {
    fn from_secs_f32(secs: f32) -> Self;
    fn as_secs_f32(&self) -> f32;
    fn as_secs_f64(&self) -> f64;
}
#[allow(clippy::cast_sign_loss, clippy::cast_precision_loss, clippy::cast_possible_truncation)]
impl TimeDeltaExtension for TimeDelta {
    fn from_secs_f32(secs: f32) -> Self {
        let seconds = secs.trunc() as i64;
        let nanoseconds = (secs.fract() * 1e9) as u32;
        Self::new(seconds, nanoseconds).expect("bad duration")
    }
    fn as_secs_f32(&self) -> f32 {
        self.num_microseconds().expect("duration overflow") as f32 / 1e6
    }
    fn as_secs_f64(&self) -> f64 {
        self.num_microseconds().expect("duration overflow") as f64 / 1e6
    }
}

/// Represents a chunk of time that has been listened to.
#[derive(Debug)]
pub struct ListenedChunk {
    /// The position in the song when this chunk started, in seconds.
    started_at_song_position: f32,
    /// The actual time when this chunk started.
    started_at: DateTime,
    duration: chrono::TimeDelta 
}
impl ListenedChunk {
    #[expect(dead_code, reason = "might be useful later")]
    pub fn ended_at(&self) -> DateTime {
        self.started_at.checked_add_signed(self.duration).expect("date out of range")
    }
    pub fn ended_at_song_position(&self) -> f32 {
        self.started_at_song_position + self.duration.as_secs_f32()
    }
}

#[derive(Debug, Clone)]
pub struct CurrentListened {
    started_at_song_position: f32, // seconds
    started_at: DateTime,
}
impl From<CurrentListened> for ListenedChunk {
    fn from(value: CurrentListened) -> Self {
        Self {
            started_at: value.started_at,
            started_at_song_position: value.started_at_song_position,
            duration: chrono::Utc::now().signed_duration_since(value.started_at),
        }
    }
}
impl CurrentListened {
    pub fn new_with_position(position: f32) -> Self {
        Self {
            started_at: chrono::Utc::now(),
            started_at_song_position: position
        }
    }
    pub fn get_expected_song_position(&self) -> f32 {
        self.started_at_song_position + chrono::Utc::now().signed_duration_since(self.started_at).as_secs_f32()
    }
}

#[derive(Debug)]
pub struct Listened {
    pub contiguous: Vec<ListenedChunk>,
    pub current: Option<CurrentListened>,
}
impl Listened {
    pub const fn new() -> Self {
        Self {
            contiguous: vec![],
            current: None,
        }
    }

    #[allow(unused, reason = "used only by certain featured-gated backends")]
    pub fn new_with_current(position: f32) -> Self {
        Self {
            contiguous: vec![],
            current: Some(CurrentListened::new_with_position(position)),
        }
    }

    #[allow(unused, reason = "used only by certain featured-gated backends")]
    pub fn started_at(&self) -> Option<DateTime> {
        self.contiguous
            .iter()
            .map(|chunk| chunk.started_at)
            .chain(self.current.as_ref().map(|current| current.started_at))
            .min()
    }

    /// Returns the index in which a [`CurrentListened`] should be placed
    /// which would result it being correctly ordered in terms of when
    /// the song started.
    /// 
    /// ## Example
    /// Returning an index considering the current listen chunk has a start position of 8:
    /// <pre>
    ///   [1, 5, 6, 10, 20, 32] # Start times
    ///             ^-- An index of three (zero-indexed) would be best
    ///                 to preserve proper ordering, so that'd what'd be returned.
    /// </pre>
    fn find_index_for_current(&self, current: &CurrentListened) -> usize {
        self.contiguous.binary_search_by(|chunk| {
            chunk.started_at_song_position.total_cmp(&current.started_at_song_position)
        }).unwrap_or_else(|i| i)
    }

    /// Rid the current listening session to place it into the ordered
    /// array of listening sessions.
    pub fn flush_current(&mut self) {
        if let Some(current) = self.current.take() {
            let index = self.find_index_for_current(&current);
            self.contiguous.insert(index, current.into());
        }
    }
    
    pub fn set_new_current(&mut self, current_song_position: f32) {
        if self.current.replace(CurrentListened::new_with_position(current_song_position)).is_some() {
            tracing::warn!("overwrote current before it was flushed");
        }
    }
    
    // TODO: Allow user to configure this behavior for checks instead.
    #[expect(unused)]
    pub fn total_heard_unique(&self) -> chrono::TimeDelta {
        if self.contiguous.is_empty() {
            return self.current.as_ref()
                .map(|current| chrono::Utc::now().signed_duration_since(current.started_at))
                .unwrap_or_default()
        }
        
        let mut total = chrono::TimeDelta::zero();
        let mut last_end_position = 0.0;

        let current = self.current.clone().map(|current| (
            self.find_index_for_current(&current),
            Into::<ListenedChunk>::into(current),
        ));
        
        for index in 0..self.contiguous.len() + usize::from(current.is_some()) {
            let chunk = if let Some((current_idx, current)) = &current {
                use core::cmp::Ordering;
                match index.cmp(current_idx) {
                    Ordering::Greater => &self.contiguous[index - 1],
                    Ordering::Equal => current,
                    Ordering::Less => &self.contiguous[index]
                }
            } else { &self.contiguous[index] };

            let chunk_start = chunk.started_at_song_position;
            let chunk_end = chunk.ended_at_song_position();

            if chunk_end > last_end_position {
                let len = chunk_end - chunk_start.max(last_end_position);
                total += chrono::TimeDelta::from_secs_f32(len);
                last_end_position = chunk_end;
            }
        }

        total
    }

    #[allow(unused, reason = "used only by certain featured-gated backends")]
    pub fn total_heard(&self) -> chrono::TimeDelta {
        self.contiguous.iter()
            .map(|d| d.duration)
            .fold(
                self.current.as_ref()
                    .map(|c| chrono::Utc::now().signed_duration_since(c.started_at))
                    .unwrap_or_default(),
                |a, b| a + b
            )
    }
}
impl Default for Listened {
    fn default() -> Self {
        Self::new()
    }
}
