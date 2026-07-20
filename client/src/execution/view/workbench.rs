//! The tests + verdict panels (oracle: `WorkbenchTests` + `verdictPanel`, step-15 scope): case
//! chips seeding the values grid, the Run-with-stdin seam, and the submit lifecycle from 202 to
//! the flattened outcome.

use std::collections::BTreeMap;

use leptos::prelude::*;
use synapse_shared::execution::{TestCase, TestSpec, Verdict};
use synapse_shared::submission::SubmissionDto;

use crate::execution::logic;
use crate::execution::state::{SubmitState, SubmitStore};

/// Per-block test-panel state: the active case, the editable values grid, the sparse
/// per-case verdict map (only cases that have actually been Run carry a badge — oracle:
/// `WorkbenchState.verdicts`), and the case a launch was fired for (so the arriving result
/// is judged against THAT case, never against whichever chip is selected by then).
#[derive(Clone, Copy)]
pub struct TestsState {
    pub active_case: RwSignal<usize>,
    pub values: RwSignal<BTreeMap<String, String>>,
    pub verdicts: RwSignal<BTreeMap<usize, Verdict>>,
    pub ran_case: RwSignal<Option<usize>>,
}

impl TestsState {
    pub fn new(spec: &TestSpec) -> Self {
        Self {
            active_case: RwSignal::new(0),
            values: RwSignal::new(logic::seed_values(spec, 0)),
            verdicts: RwSignal::new(BTreeMap::new()),
            ran_case: RwSignal::new(None),
        }
    }
}

/// A case pushed at the panel from outside it: `(tick, case)`. The tick is what makes re-sending
/// the SAME failing input fire again — the case alone would not change. Tick 0 means "nothing
/// yet", the convention the copy-to-editor seam already uses.
pub type CaseRequest = (u32, TestCase);

/// The empty request a `CaseRequest` signal starts life holding.
pub fn no_case_yet() -> CaseRequest {
    (
        0,
        TestCase {
            args: BTreeMap::new(),
            expected: None,
        },
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// THE CASE SINK — the one way a case joins the panel
// ─────────────────────────────────────────────────────────────────────────────

/// Every route that adds a case (a failed submission's input, the `+` chip) goes through
/// `append`, because two things are easy to get wrong and neither shows up until later.
#[derive(Clone, Copy)]
struct CaseSink {
    spec: RwSignal<TestSpec>,
    tests: TestsState,
    on_switch: Callback<usize>,
}

impl CaseSink {
    /// APPEND, never insert: `verdicts` is a sparse map keyed by case index, so inserting
    /// mid-list would slide every existing ✓/✗ onto the wrong chip.
    fn append(self, case: TestCase) {
        let index = self.spec.with_untracked(|s| s.cases.len());
        self.spec.update(|s| s.cases.push(case));
        self.tests.active_case.set(index);
        self.tests
            .values
            .set(self.spec.with_untracked(|s| logic::seed_values(s, index)));
        // Load-bearing: the output panel judges against `ran_case`, not `active_case`. Without
        // the same callback the chips use, a previous case's result would stay on screen —
        // still labelled with ITS expected — while the new chip sits selected.
        self.on_switch.run(index);
    }
}

/// One editable input per DECLARED arg. Read UNTRACKED and built once: appending a case changes
/// the suite's cases, never its args, so this grid has nothing to react to.
fn value_fields(spec: RwSignal<TestSpec>, tests: TestsState) -> Vec<impl IntoView + use<>> {
    spec.read_untracked()
        .args
        .iter()
        .map(|arg| {
            let id = arg.id.clone();
            let input_id = id.clone();
            let placeholder = arg.placeholder.clone().unwrap_or_default();
            view! {
                <label class="wb__field">
                    <span class="wb__field-label">{arg.label.clone()}</span>
                    <input
                        class="wb__input"
                        placeholder=placeholder
                        prop:value=move || tests.values.read().get(&id).cloned().unwrap_or_default()
                        on:input=move |ev| {
                            let value = event_target_value(&ev);
                            tests.values.update(|v| {
                                v.insert(input_id.clone(), value);
                            });
                        }
                    />
                </label>
            }
        })
        .collect()
}

#[component]
pub fn TestsPanel(
    /// The panel's LIVE suite: the authored cases plus any the learner appended from a failed
    /// submission. A signal rather than a `StoredValue` so the chip row tracks its length.
    spec: RwSignal<TestSpec>,
    tests: TestsState,
    /// Fired on chip click AFTER the state re-seed — the block clears its stale run output
    /// (oracle: `switchCase` resets the FSM; earlier badges stay on the chips).
    on_switch: Callback<usize>,
    /// Cases pushed in from outside: the verdict panel's "Use this test case", and the problem
    /// page's Submissions rows. One consumer here, several producers there.
    use_case: RwSignal<CaseRequest>,
) -> impl IntoView {
    let sink = CaseSink {
        spec,
        tests,
        on_switch,
    };
    Effect::new(move |seen: Option<u32>| {
        let (tick, case) = use_case.get();
        if tick == 0 || seen == Some(tick) {
            return tick;
        }
        sink.append(case);
        tick
    });
    let chips = move || {
        (0..spec.read().cases.len())
            .map(|index| {
                view! {
                    <button
                        class="wb__chip"
                        class:wb__chip--active=move || tests.active_case.get() == index
                        class:wb__chip--ok=move || {
                            tests.verdicts.read().get(&index) == Some(&Verdict::Accepted)
                        }
                        class:wb__chip--fail=move || {
                            matches!(
                                tests.verdicts.read().get(&index),
                                Some(Verdict::WrongAnswer | Verdict::Errored)
                            )
                        }
                        on:click=move |_| {
                            tests.active_case.set(index);
                            tests.values.set(logic::seed_values(&spec.read_untracked(), index));
                            on_switch.run(index);
                        }
                    >
                        {format!("Case {}", index + 1)}
                        {move || match tests.verdicts.read().get(&index) {
                            Some(Verdict::Accepted) => {
                                Some(view! { <span class="wb__tick">"✓"</span> })
                            }
                            Some(Verdict::WrongAnswer | Verdict::Errored) => {
                                Some(view! { <span class="wb__tick">"✗"</span> })
                            }
                            _ => None,
                        }}
                    </button>
                }
            })
            .collect::<Vec<_>>()
    };

    let fields = value_fields(spec, tests);

    let expected = move || {
        logic::expected_for(&spec.read(), tests.active_case.get())
            .map(|e| view! { <div class="wb__expected"><span class="wb__field-label">"Expected"</span><pre>{e}</pre></div> })
    };

    // An empty case has nothing to check, so `judge` returns `Finished` and the chip stays
    // unbadged until the code actually crashes on it — which is the honest signal.
    let add_blank = move |_| {
        sink.append(TestCase {
            args: BTreeMap::new(),
            expected: None,
        });
    };

    view! {
        <div class="wb__tests">
            <div class="wb__chips">
                {chips}
                <button
                    class="wb__chip wb__chip--add"
                    aria-label="Add a test case"
                    title="Add a test case of your own"
                    on:click=add_blank
                >
                    "+"
                </button>
            </div>
            <div class="wb__values">{fields}</div>
            {expected}
        </div>
    }
}

#[component]
pub fn VerdictPanel(
    submit: SubmitStore,
    /// The visible suite, read only to decide whether a judged failure can be reproduced in it.
    spec: Option<RwSignal<TestSpec>>,
    /// Where "Use this test case" pushes the failing input. Same channel the Submissions feed
    /// writes to; `TestsPanel` is the single consumer.
    use_case: RwSignal<CaseRequest>,
) -> impl IntoView {
    view! {
        {move || match submit.state.get() {
            SubmitState::Idle => ().into_any(),
            SubmitState::Judging(id) => view! {
                <div class="wb__verdict wb__verdict--judging">
                    "Judging against the hidden suite… " <span class="wb__verdict-id">{id}</span>
                </div>
            }
            .into_any(),
            SubmitState::Failed(message) => view! {
                <div class="wb__verdict wb__verdict--failed">"Submit failed: " {message}</div>
            }
            .into_any(),
            SubmitState::Done(dto) => done_panel(&dto, spec, use_case).into_any(),
        }}
    }
}

/// The button that turns a judged failure into a case you can Run. Rendered DISABLED rather than
/// hidden when the input can't be reproduced — on the one problem class where that happens (a
/// hidden sidecar suite whose arg ids differ from the fence's) a missing button reads as a bug,
/// and the tooltip is the only place the learner can find out why.
pub fn use_case_button(
    failure: &synapse_shared::submission::FailedCaseDto,
    spec: Option<&TestSpec>,
    use_case: RwSignal<CaseRequest>,
) -> impl IntoView + use<> {
    let reproducible = spec.is_some_and(|s| logic::can_reproduce(s, &failure.args));
    let case = TestCase {
        args: failure.args.clone(),
        expected: failure.expected.clone(),
    };
    let tip = if reproducible {
        "Adds this input as a new case below. It reproduces the input, not necessarily the judge's exact stdin."
    } else {
        "This problem is judged against a larger hidden suite whose inputs don't line up with the fields below."
    };
    view! {
        <button
            class="wb__use-case"
            disabled=!reproducible
            data-tip=tip
            on:click=move |_| {
                use_case.update(|(tick, slot)| {
                    *tick += 1;
                    *slot = case.clone();
                });
            }
        >
            "Use this test case"
        </button>
    }
}

fn done_panel(
    dto: &SubmissionDto,
    spec: Option<RwSignal<TestSpec>>,
    use_case: RwSignal<CaseRequest>,
) -> impl IntoView + use<> {
    let counts = format!("{} / {}", dto.passed.unwrap_or(0), dto.total.unwrap_or(0));
    match dto.verdict.as_deref() {
        Some("accepted") => view! {
            <div class="wb__verdict wb__verdict--accepted">"Accepted ✓ — " {counts} " cases"</div>
        }
        .into_any(),
        Some("rejected") => {
            let visible = spec.map(|s| s.get_untracked());
            let failure = dto.first_failure.clone().map(|f| {
                let button = use_case_button(&f, visible.as_ref(), use_case);
                // The failing INPUT, which is what makes the failure actionable. Note the case
                // number is the JUDGED suite's — never reuse it as a chip label.
                let input: Vec<_> = f
                    .args
                    .iter()
                    .map(|(id, value)| {
                        view! { <pre class="wb__failure-line">{format!("{id}: {value}")}</pre> }
                    })
                    .collect();
                view! {
                    <div class="wb__failure">
                        <div class="wb__failure-head">
                            <span class="wb__field-label">
                                {format!("First failure — case {}", f.index + 1)}
                            </span>
                            {button}
                        </div>
                        {input}
                        {f.expected.map(|e| view! { <pre class="wb__failure-line">"expected: " {e}</pre> })}
                        <pre class="wb__failure-line">"stdout:   " {f.stdout}</pre>
                        {(!f.stderr.is_empty())
                            .then(|| view! { <pre class="wb__failure-line">"stderr:   " {f.stderr}</pre> })}
                    </div>
                }
            });
            view! {
                <div class="wb__verdict wb__verdict--rejected">
                    "Wrong answer ✗ — " {counts} " cases passed"
                    {failure}
                </div>
            }
            .into_any()
        }
        _ => {
            let detail = dto.detail.clone().unwrap_or_default();
            view! {
                <div class="wb__verdict wb__verdict--failed">
                    "The judge failed mid-suite — " {counts} " passed. " {detail}
                </div>
            }
            .into_any()
        }
    }
}
