//! Boot-time reconciliation for `SubmitSolution` — a process that dies mid-judge leaves its row
//! unfinished forever, so a restart sweeps them. Split out of `service_tests.rs` (the 500-line
//! cap); the fakes and builders it drives (`service`, `spec`, `path`) live in the parent module.

use super::*;

fn unfinished_row(state: SubmissionState, age: Duration) -> Submission {
    Submission {
        id: SubmissionId(Uuid::new_v4()),
        lesson_path: path(),
        language: "python".into(),
        source: "src".into(),
        user_id: None,
        created_at: Utc::now() - age,
        state,
    }
}

#[tokio::test]
async fn reconcile_completes_rows_a_dead_process_left_judging() {
    let (svc, _) = service(Some(spec(&[Some("0"), Some("1")])), vec![]);
    let orphan = unfinished_row(SubmissionState::Judging, Duration::minutes(30));
    svc.repo.save(&orphan).await.unwrap();

    let healed = svc.reconcile_unfinished(Duration::minutes(10)).await.unwrap();

    assert_eq!(healed, 1);
    let row = svc.get(orphan.id).await.unwrap();
    // Terminal, and honest about why — the client's poll can stop.
    match row.state {
        SubmissionState::Completed {
            outcome:
                SuiteOutcome::JudgeFailed {
                    passed,
                    total,
                    detail,
                },
            ..
        } => {
            assert_eq!(
                (passed, total),
                (0, 2),
                "the suite size rides along for the 0/N verdict"
            );
            assert!(detail.contains("restart"), "detail explains itself: {detail}");
        }
        other => panic!("expected a JudgeFailed completion, got {other:?}"),
    }
}

#[tokio::test]
async fn reconcile_spares_a_run_that_may_still_be_in_flight() {
    let (svc, _) = service(Some(spec(&[Some("0")])), vec![]);
    // Younger than the grace window: another replica could legitimately still be judging it.
    let fresh = unfinished_row(SubmissionState::Judging, Duration::seconds(5));
    svc.repo.save(&fresh).await.unwrap();

    let healed = svc.reconcile_unfinished(Duration::minutes(10)).await.unwrap();

    assert_eq!(healed, 0);
    assert_eq!(svc.get(fresh.id).await.unwrap().state, SubmissionState::Judging);
}

#[tokio::test]
async fn reconcile_leaves_completed_rows_alone() {
    let (svc, _) = service(Some(spec(&[Some("0")])), vec![]);
    let done = unfinished_row(SubmissionState::Judging, Duration::minutes(30))
        .completed(SuiteOutcome::Accepted { total: 1 }, Utc::now());
    svc.repo.save(&done).await.unwrap();

    let healed = svc.reconcile_unfinished(Duration::minutes(10)).await.unwrap();

    assert_eq!(healed, 0);
    assert!(svc.get(done.id).await.unwrap().state.is_completed());
}
