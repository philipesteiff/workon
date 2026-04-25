use std::collections::BTreeSet;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use tachyonfx::{fx, Effect, EffectManager, Interpolation, Motion};

use super::state::{ToastKind, TuiMode, TuiState};

pub(super) const ANIMATION_FRAME_INTERVAL: u64 = 33;

#[derive(Debug, Default)]
pub(super) struct AnimationRuntime {
    effects: EffectManager<AnimationKey>,
    pending: BTreeSet<AnimationTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AnimationSnapshot {
    mode: TuiMode,
    trace_visible: bool,
    detail_visible: bool,
    toast: Option<ToastSignature>,
    list_count: usize,
    filtered_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum AnimationTarget {
    WorkQueue,
    DetailPanel,
    TracePanel,
    FooterStatus,
    Toast,
    Overlay,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct RenderRegions {
    work_queue: Option<Rect>,
    detail_panel: Option<Rect>,
    trace_panel: Option<Rect>,
    footer_status: Option<Rect>,
    toast: Option<Rect>,
    overlay: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct AnimationKey {
    target: AnimationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToastSignature {
    kind: ToastKind,
    title: String,
    message: String,
}

impl AnimationRuntime {
    pub(super) fn observe_transition(
        &mut self,
        before: AnimationSnapshot,
        after: AnimationSnapshot,
    ) {
        if before.mode != after.mode {
            if is_overlay_mode(after.mode) {
                self.queue(AnimationTarget::FooterStatus);
                self.queue(AnimationTarget::Overlay);
            } else if is_overlay_mode(before.mode) {
                self.queue(AnimationTarget::FooterStatus);
            }
        }

        if before.list_count != after.list_count {
            self.queue(AnimationTarget::WorkQueue);
        }

        if before.trace_visible != after.trace_visible {
            self.queue(AnimationTarget::FooterStatus);
            if after.trace_visible {
                self.queue(AnimationTarget::TracePanel);
            }
        }

        if before.detail_visible != after.detail_visible {
            self.queue(AnimationTarget::FooterStatus);
            if after.detail_visible {
                self.queue(AnimationTarget::DetailPanel);
            }
        }

        if before.toast != after.toast && after.toast.is_some() {
            self.queue(AnimationTarget::Toast);
        }
    }

    pub(super) fn prepare_frame(&mut self, regions: &RenderRegions) {
        let targets = self.pending.iter().copied().collect::<Vec<_>>();
        self.pending.clear();

        for target in targets {
            let Some(area) = regions.area_for(target) else {
                continue;
            };
            if area.is_empty() {
                continue;
            }

            let key = AnimationKey { target };
            self.effects
                .add_unique_effect(key, effect_for(target).with_area(area));
        }
    }

    pub(super) fn process_frame(&mut self, elapsed: Duration, buffer: &mut Buffer, area: Rect) {
        self.effects.process_effects(elapsed, buffer, area);
    }

    pub(super) fn is_animating(&self) -> bool {
        !self.pending.is_empty() || self.effects.is_running()
    }

    #[cfg(test)]
    pub(super) fn has_pending(&self, target: AnimationTarget) -> bool {
        self.pending.contains(&target)
    }

    fn queue(&mut self, target: AnimationTarget) {
        self.pending.insert(target);
    }
}

fn is_overlay_mode(mode: TuiMode) -> bool {
    matches!(mode, TuiMode::Create | TuiMode::Archive | TuiMode::Help)
}

impl AnimationSnapshot {
    pub(super) fn from_state(state: &TuiState) -> Self {
        Self {
            mode: state.mode,
            trace_visible: state.trace_visible,
            detail_visible: state.detail_visible,
            toast: state.toast.as_ref().map(|toast| ToastSignature {
                kind: toast.kind,
                title: toast.title.clone(),
                message: toast.message.clone(),
            }),
            list_count: state.total_count(),
            filtered_count: state.filtered_count(),
        }
    }
}

impl RenderRegions {
    pub(super) fn set(&mut self, target: AnimationTarget, area: Rect) {
        match target {
            AnimationTarget::WorkQueue => self.work_queue = Some(area),
            AnimationTarget::DetailPanel => self.detail_panel = Some(area),
            AnimationTarget::TracePanel => self.trace_panel = Some(area),
            AnimationTarget::FooterStatus => self.footer_status = Some(area),
            AnimationTarget::Toast => self.toast = Some(area),
            AnimationTarget::Overlay => self.overlay = Some(area),
        }
    }

    fn area_for(&self, target: AnimationTarget) -> Option<Rect> {
        match target {
            AnimationTarget::WorkQueue => self.work_queue,
            AnimationTarget::DetailPanel => self.detail_panel,
            AnimationTarget::TracePanel => self.trace_panel,
            AnimationTarget::FooterStatus => self.footer_status,
            AnimationTarget::Toast => self.toast,
            AnimationTarget::Overlay => self.overlay,
        }
    }

    #[cfg(test)]
    pub(super) fn has_region(&self, target: AnimationTarget) -> bool {
        self.area_for(target).is_some()
    }
}

impl Default for AnimationKey {
    fn default() -> Self {
        Self {
            target: AnimationTarget::FooterStatus,
        }
    }
}

pub(super) fn input_poll_timeout(animating: bool) -> Option<Duration> {
    animating.then(|| Duration::from_millis(ANIMATION_FRAME_INTERVAL))
}

fn effect_for(target: AnimationTarget) -> Effect {
    let dark = Color::Rgb(12, 15, 11);

    match target {
        AnimationTarget::Overlay => {
            fx::parallel(&[fx::fade_from_fg(dark, (180, Interpolation::QuadOut))])
        }
        AnimationTarget::Toast => fx::parallel(&[
            fx::fade_from_fg(Color::Rgb(80, 56, 32), (180, Interpolation::QuadOut)),
            fx::sweep_in(
                Motion::RightToLeft,
                6,
                0,
                dark,
                (220, Interpolation::QuadOut),
            ),
        ]),
        AnimationTarget::TracePanel | AnimationTarget::DetailPanel | AnimationTarget::WorkQueue => {
            fx::parallel(&[
                fx::coalesce((180, Interpolation::QuadOut)),
                fx::sweep_in(
                    Motion::LeftToRight,
                    6,
                    0,
                    dark,
                    (180, Interpolation::QuadOut),
                ),
            ])
        }
        AnimationTarget::FooterStatus => fx::sweep_in(
            Motion::LeftToRight,
            4,
            0,
            dark,
            (140, Interpolation::QuadOut),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::{AnimationRuntime, AnimationTarget, RenderRegions};

    #[test]
    fn frame_processing_drains_completed_effects() {
        let area = Rect::new(0, 0, 40, 10);
        let mut runtime = AnimationRuntime::default();
        let mut regions = RenderRegions::default();
        let mut buffer = Buffer::empty(area);

        regions.set(AnimationTarget::WorkQueue, area);
        runtime.queue(AnimationTarget::WorkQueue);
        runtime.prepare_frame(&regions);

        assert!(runtime.is_animating());

        runtime.process_frame(Duration::from_millis(1_000), &mut buffer, area);

        assert!(!runtime.is_animating());
    }

    #[test]
    fn overlay_animation_does_not_paint_blank_background_cells() {
        let area = Rect::new(0, 0, 40, 10);
        let mut runtime = AnimationRuntime::default();
        let mut regions = RenderRegions::default();
        let mut buffer = Buffer::empty(area);

        regions.set(AnimationTarget::Overlay, area);
        runtime.queue(AnimationTarget::Overlay);
        runtime.prepare_frame(&regions);
        runtime.process_frame(Duration::from_millis(1), &mut buffer, area);

        assert!(buffer
            .content()
            .iter()
            .all(|cell| cell.bg == ratatui::style::Color::Reset));
    }
}
