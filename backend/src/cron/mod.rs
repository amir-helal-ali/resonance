// ============================================================================
// resonance-backend/src/cron/mod.rs
// tokio-cron-scheduler jobs for the Smart Content Lifecycle.
//
// Jobs (all UTC):
//   1. promote_glow_to_linger       — every hour, move pulses older than 48h
//   2. evaporate_30_day             — daily 03:00, destroy keys for >30d pulses
//   3. immortal_decay               — weekly Sunday 04:00, evaporate preserved
//                                     pulses with no interaction in 6 months
//   4. resonance_decay              — every 15 min, halve stale resonance
//   5. prune_traces                 — hourly, delete traces past their 7-day TTL
//   6. moderation_cooling_release   — every 2 min, release cooled pulses
// ============================================================================

use sqlx::PgPool;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use tracing::{error, info};

use crate::db::queries;

pub async fn start(pool: PgPool) -> Result<JobScheduler, JobSchedulerError> {
    let scheduler = JobScheduler::new().await?;

    // ----- 1. promote glow → linger (hourly) -----
    let pool1 = pool.clone();
    let job1 = Job::new_async("0 0 * * * *", move |_, _| {
        let pool = pool1.clone();
        Box::pin(async move {
            match queries::promote_glow_to_linger(&pool).await {
                Ok(n) => info!(promoted = n, "glow → linger"),
                Err(e) => error!(error = ?e, "glow → linger failed"),
            }
        })
    })
    .await?;
    scheduler.add(job1).await?;

    // ----- 2. evaporate 30-day pulses (daily 03:00) -----
    let pool2 = pool.clone();
    let job2 = Job::new_async("0 0 3 * * *", move |_, _| {
        let pool = pool2.clone();
        Box::pin(async move {
            let candidates = match queries::find_pulses_to_evaporate(&pool).await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = ?e, "fetch evaporate candidates failed");
                    return;
                }
            };
            info!(count = candidates.len(), "evaporating 30-day pulses");
            for pulse_id in candidates {
                match queries::evaporate_pulse(&pool, pulse_id).await {
                    Ok(_) => info!(%pulse_id, "evaporated"),
                    Err(e) => error!(error = ?e, %pulse_id, "evaporate failed"),
                }
            }
        })
    })
    .await?;
    scheduler.add(job2).await?;

    // ----- 3. immortal decay (Sunday 04:00) -----
    let pool3 = pool.clone();
    let job3 = Job::new_async("0 0 4 * * 0", move |_, _| {
        let pool = pool3.clone();
        Box::pin(async move {
            let candidates = match queries::find_immortal_decay_candidates(&pool).await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = ?e, "fetch immortal decay candidates failed");
                    return;
                }
            };
            info!(count = candidates.len(), "immortal decay");
            for pulse_id in candidates {
                match queries::evaporate_pulse(&pool, pulse_id).await {
                    Ok(_) => info!(%pulse_id, "immortal decay evaporated"),
                    Err(e) => error!(error = ?e, %pulse_id, "immortal decay failed"),
                }
            }
        })
    })
    .await?;
    scheduler.add(job3).await?;

    // ----- 4. resonance decay (every 15 min) -----
    let pool4 = pool.clone();
    let job4 = Job::new_async("0 */15 * * * *", move |_, _| {
        let pool = pool4.clone();
        Box::pin(async move {
            match queries::decay_resonance(&pool).await {
                Ok(n) => info!(affected = n, "resonance decay"),
                Err(e) => error!(error = ?e, "resonance decay failed"),
            }
        })
    })
    .await?;
    scheduler.add(job4).await?;

    // ----- 5. prune traces (hourly) -----
    let pool5 = pool.clone();
    let job5 = Job::new_async("0 0 * * * *", move |_, _| {
        let pool = pool5.clone();
        Box::pin(async move {
            match queries::prune_expired_traces(&pool).await {
                Ok(n) => info!(pruned = n, "traces pruned"),
                Err(e) => error!(error = ?e, "trace prune failed"),
            }
        })
    })
    .await?;
    scheduler.add(job5).await?;

    // ----- 6. moderation cooling release (every 2 min) -----
    let pool6 = pool.clone();
    let job6 = Job::new_async("0 */2 * * * *", move |_, _| {
        let pool = pool6.clone();
        Box::pin(async move {
            // Release pulses whose cooling window has expired.
            let res = sqlx::query(
                r#"
                UPDATE moderation_queue
                SET verdict = 'released', cooling_until = NULL
                WHERE verdict = 'cooling' AND cooling_until < now()
                "#,
            )
            .execute(&pool)
            .await;
            match res {
                Ok(r) => info!(released = r.rows_affected(), "moderation cooling released"),
                Err(e) => error!(error = ?e, "cooling release failed"),
            }

            // Summon juries for high-toxicity pulses that haven't been juried yet.
            let candidates: Result<Vec<(uuid::Uuid,)>, _> = sqlx::query_as(
                r#"
                SELECT m.pulse_id FROM moderation_queue m
                WHERE m.toxicity_score >= 0.9
                  AND m.verdict = 'cooling'
                  AND NOT EXISTS (SELECT 1 FROM jury_panels j WHERE j.pulse_id = m.pulse_id)
                "#,
            )
            .fetch_all(&pool)
            .await;
            if let Ok(rows) = candidates {
                for (pulse_id,) in rows {
                    if let Err(e) = crate::handlers::jury::summon_for_pulse(&pool, pulse_id).await {
                        error!(error = ?e, %pulse_id, "jury summon failed");
                    }
                }
            }
        })
    })
    .await?;
    scheduler.add(job6).await?;

    // ----- 7. jury panel expiry (every 5 min) -----
    let pool7 = pool.clone();
    let job7 = Job::new_async("0 */5 * * * *", move |_, _| {
        let pool = pool7.clone();
        Box::pin(async move {
            let res = sqlx::query(
                r#"
                UPDATE jury_panels
                SET final_verdict = 'expire', concluded_at = now()
                WHERE final_verdict = 'pending' AND expires_at < now()
                "#,
            )
            .execute(&pool)
            .await;
            match res {
                Ok(r) => info!(expired = r.rows_affected(), "jury panels expired"),
                Err(e) => error!(error = ?e, "jury expiry failed"),
            }
        })
    })
    .await?;
    scheduler.add(job7).await?;

    scheduler.start().await?;
    info!("cron scheduler started with 7 jobs");
    Ok(scheduler)
}
