//! The hidden operator surface has no primary FIFO to pace rendering.
use super::*;
pub(super) struct Pacer {
    interval: Duration,
    deadline: Option<Instant>,
}
impl Default for Pacer {
    fn default() -> Self {
        Self {
            interval: Duration::from_millis(25),
            deadline: None,
        }
    }
}
impl Pacer {
    pub fn observed(&mut self, fps: f64) {
        if fps.is_finite() && fps >= 1. {
            self.interval = Duration::from_secs_f64(1. / fps.min(60.));
        }
    }
    pub fn clear(&mut self) {
        self.deadline = None;
    }
    pub fn wait(&mut self, now: Instant) -> Duration {
        if let Some(t) = self.deadline {
            if t > now {
                return t - now;
            }
        }
        // Missed deadlines are discarded. Restoring an occluded window cannot
        // generate a burst of renders to make up for missed ticks.
        self.deadline = Some(now + self.interval);
        Duration::ZERO
    }
}
use super::super::VisibilityProof as Proof;
pub(super) struct Probe {
    parent: HWND,
    audience: HWND,
    reply: Option<macros::Reply<Proof>>,
    started: Instant,
    phase: u8,
    since: Option<(Instant, u64)>,
    proof: Proof,
}
impl Probe {
    pub fn new(parent: HWND, audience: HWND, reply: macros::Reply<Proof>) -> Self {
        unsafe {
            ShowWindowAsync(parent, SW_MINIMIZE);
        }
        Self {
            parent,
            audience,
            reply: Some(reply),
            started: Instant::now(),
            phase: 0,
            since: None,
            proof: Proof::default(),
        }
    }
    pub fn poll(&mut self, submissions: u64) -> bool {
        unsafe {
            let reply = if IsWindow(self.parent) == 0
                || IsWindow(self.audience) == 0
                || self.started.elapsed() > Duration::from_secs(8)
            {
                Some(Err(
                    "Native visibility proof timed out or its window closed".into(),
                ))
            } else if self.phase == 2 {
                if IsIconic(self.parent) == 0 && IsIconic(self.audience) == 0 {
                    Some(Ok(std::mem::take(&mut self.proof)))
                } else {
                    None
                }
            } else {
                let expected = IsIconic(self.parent) != 0
                    && (IsIconic(self.audience) != 0) == (self.phase == 1);
                if expected {
                    let (since, begin) = *self
                        .since
                        .get_or_insert_with(|| (Instant::now(), submissions));
                    let elapsed = since.elapsed();
                    if elapsed >= Duration::from_millis(650) {
                        let count = submissions - begin;
                        if self.phase == 0 {
                            self.proof.hidden_ms = elapsed.as_secs_f64() * 1000.;
                            self.proof.offscreen_frames = count;
                            ShowWindowAsync(self.audience, SW_MINIMIZE);
                        } else {
                            self.proof.both_hidden_ms = elapsed.as_secs_f64() * 1000.;
                            self.proof.both_hidden_frames = count;
                            self.restore();
                        }
                        self.phase += 1;
                        self.since = None;
                    }
                }
                None
            };
            if let Some(result) = reply {
                if let Some(tx) = self.reply.take() {
                    let _ = tx.try_send(result);
                }
                true
            } else {
                false
            }
        }
    }
    fn restore(&self) {
        unsafe {
            if IsWindow(self.audience) != 0 {
                ShowWindowAsync(self.audience, SW_RESTORE);
            }
            if IsWindow(self.parent) != 0 {
                ShowWindowAsync(self.parent, SW_RESTORE);
            }
        }
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        if self.phase < 2 {
            self.restore();
        }
    }
}
impl Host {
    pub async fn verify_visibility(&self) -> Result<Proof, String> {
        if !crate::export_smoke::enabled() {
            return Err("Visibility verification requires --verify-wheel-export".into());
        }
        self.request(Command::VerifyVisibility).await
    }
}
