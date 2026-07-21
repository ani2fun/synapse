//! The transport bar: first/prev/play-pause/next/last, a scrubber,
//! and the step label — over the ONE `Playback` stepper. The single interval timer lives
//! here: started on play, cleared on pause/unmount; `tick` self-stops at the end.

use crate::engine::playback::State;
use leptos::prelude::*;

const STEP_DELAY_MS: u32 = 900;

#[component]
pub fn TransportBar(
    state: RwSignal<State>,
    /// Diff mode: when non-empty, the STEP buttons hop only
    /// between these indices; the scrubber, label, and autoplay still cover every step.
    #[prop(optional)]
    stops: Option<Signal<Vec<usize>>>,
) -> impl IntoView {
    let hop = move |current: usize, forward: bool| -> Option<usize> {
        let stops = stops?.get_untracked();
        if stops.is_empty() {
            return None;
        }
        if forward {
            stops.iter().copied().find(|&i| i > current)
        } else {
            stops.iter().rev().copied().find(|&i| i < current)
        }
    };
    let timer: StoredValue<Option<gloo_timers::callback::Interval>, LocalStorage> =
        StoredValue::new_local(None);

    // One timer, driven by `playing`: an always-on tick is harmless (paused = no-op), but
    // clearing on pause keeps the browser quiet.
    Effect::new(move |_| {
        let playing = state.get().playing;
        if playing && timer.read_value().is_none() {
            timer.set_value(Some(gloo_timers::callback::Interval::new(
                STEP_DELAY_MS,
                move || {
                    state.update(|s| *s = s.tick());
                },
            )));
        } else if !playing {
            timer.set_value(None); // dropping the Interval cancels it
        }
    });
    on_cleanup(move || timer.set_value(None));

    view! {
        <div class="transport">
            <button class="transport__btn" title="First step"
                    on:click=move |_| state.update(|s| {
                        let target = stops
                            .and_then(|st| st.get_untracked().first().copied())
                            .unwrap_or(0);
                        *s = s.jump_to(i64::try_from(target).unwrap_or(0));
                    })>"⏮"</button>
            <button class="transport__btn" title="Previous step"
                    on:click=move |_| state.update(|s| {
                        *s = match hop(s.index, false) {
                            Some(i) => s.jump_to(i64::try_from(i).unwrap_or(0)),
                            None => s.previous(),
                        };
                    })>"‹"</button>
            <button class="transport__btn transport__btn--play"
                    title="Play / pause"
                    on:click=move |_| state.update(|s| *s = s.toggle_play())>
                {move || if state.get().playing { "⏸" } else { "▶" }}
            </button>
            <button class="transport__btn" title="Next step"
                    on:click=move |_| state.update(|s| {
                        *s = match hop(s.index, true) {
                            Some(i) => s.jump_to(i64::try_from(i).unwrap_or(0)),
                            None => s.next(),
                        };
                    })>"›"</button>
            <button class="transport__btn" title="Last step"
                    on:click=move |_| state.update(|s| {
                        let target = stops
                            .and_then(|st| st.get_untracked().last().copied())
                            .map_or(i64::MAX, |i| i64::try_from(i).unwrap_or(i64::MAX));
                        *s = s.jump_to(target);
                    })>"⏭"</button>
            <input
                class="transport__scrubber"
                type="range"
                min="0"
                max=move || (state.get().count.saturating_sub(1)).to_string()
                prop:value=move || state.get().index.to_string()
                on:input=move |event| {
                    if let Ok(i) = event_target_value(&event).parse::<i64>() {
                        state.update(|s| *s = s.jump_to(i));
                    }
                }
            />
            <span class="transport__label">
                {move || format!("{} / {}", state.get().index + 1, state.get().count)}
            </span>
        </div>
    }
}
