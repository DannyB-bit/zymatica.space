use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledJob {
    pub job_id: String,
    pub cron_expr: Option<String>,
    pub prompt: String,
    pub target_platform: String,
    pub target_channel: String,
    pub next_run_timestamp_ms: u64,
    pub max_iterations: Option<u32>,
    pub run_count: u32,
}

impl Ord for ScheduledJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap behavior
        other.next_run_timestamp_ms.cmp(&self.next_run_timestamp_ms)
    }
}

impl PartialOrd for ScheduledJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct CronSchedulerEngine {
    job_queue: BinaryHeap<ScheduledJob>,
}

impl Default for CronSchedulerEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CronSchedulerEngine {
    pub fn new() -> Self {
        Self {
            job_queue: BinaryHeap::new(),
        }
    }

    pub fn add_job(&mut self, job: ScheduledJob) {
        self.job_queue.push(job);
    }

    pub fn pop_due_jobs(&mut self, current_timestamp_ms: u64) -> Vec<ScheduledJob> {
        let mut due = Vec::new();
        while let Some(job) = self.job_queue.peek() {
            if job.next_run_timestamp_ms > current_timestamp_ms {
                break;
            }
            if let Some(mut job) = self.job_queue.pop() {
                job.run_count += 1;
                due.push(job);
            }
        }
        due
    }

    pub fn reschedule_job(&mut self, mut job: ScheduledJob, interval_ms: u64) {
        if job.max_iterations.is_some_and(|max| job.run_count >= max) {
            return;
        }
        job.next_run_timestamp_ms += interval_ms;
        self.add_job(job);
    }

    pub fn pending_count(&self) -> usize {
        self.job_queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_min_heap_ordering() {
        let mut engine = CronSchedulerEngine::new();
        engine.add_job(ScheduledJob {
            job_id: "job-late".to_string(),
            cron_expr: None,
            prompt: "Late job".to_string(),
            target_platform: "cli".to_string(),
            target_channel: "main".to_string(),
            next_run_timestamp_ms: 2000,
            max_iterations: None,
            run_count: 0,
        });

        engine.add_job(ScheduledJob {
            job_id: "job-early".to_string(),
            cron_expr: None,
            prompt: "Early job".to_string(),
            target_platform: "cli".to_string(),
            target_channel: "main".to_string(),
            next_run_timestamp_ms: 1000,
            max_iterations: None,
            run_count: 0,
        });

        let due = engine.pop_due_jobs(1500);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].job_id, "job-early");

        let due2 = engine.pop_due_jobs(2500);
        assert_eq!(due2.len(), 1);
        assert_eq!(due2[0].job_id, "job-late");
    }
}
