use anyhow::Result;

use crate::{
    text_index::TextUpdate,
    types::{RestoreReport, RestoreScope, SystemState},
    vector_index::VectorUpdate,
};

use super::MemoryEngine;

impl MemoryEngine {
    pub fn restore_full(&self, scope: RestoreScope) -> Result<RestoreReport> {
        let mut report = RestoreReport::default();

        if matches!(scope, RestoreScope::All | RestoreScope::Text) {
            let docs = self.db.load_search_documents()?;
            let count = self
                .text_index
                .lock()
                .expect("tantivy mutex poisoned")
                .rebuild(&docs)?;
            self.db.clear_all_index_jobs("text")?;
            self.db
                .record_index_ready("text", count, Some("tantivy rebuild complete"))?;
            report.text_documents = count;
        }

        if matches!(scope, RestoreScope::All | RestoreScope::Vector) {
            let docs = self.db.load_vector_documents()?;
            let count = self
                .vector_index
                .lock()
                .expect("vector mutex poisoned")
                .rebuild(&docs)?;
            self.db.clear_all_index_jobs("vector")?;
            self.db
                .record_index_ready("vector", count, Some("vector rebuild complete"))?;
            report.vector_documents = count;
        }

        self.refresh_l3_cache()?;
        Ok(report)
    }

    pub fn restore(&self, scope: RestoreScope) -> Result<RestoreReport> {
        let mut report = RestoreReport::default();

        if matches!(scope, RestoreScope::All | RestoreScope::Text) {
            report.text_documents = self.restore_text_index()?;
        }
        if matches!(scope, RestoreScope::All | RestoreScope::Vector) {
            report.vector_documents = self.restore_vector_index()?;
        }

        if matches!(scope, RestoreScope::All) {
            self.refresh_l3_cache()?;
        }

        Ok(report)
    }

    pub fn state(&self) -> Result<SystemState> {
        let (episode_count, entity_count, fact_count, edge_count) = self.db.stats()?;
        Ok(SystemState {
            episode_count,
            entity_count,
            fact_count,
            edge_count,
            layers: self.db.layer_summary()?,
            l3_cached: self.l3_cache.lock().expect("l3 mutex poisoned").len(),
            text_index: self.db.index_status("text")?,
            vector_index: self.db.index_status("vector")?,
        })
    }

    fn restore_text_index(&self) -> Result<usize> {
        let jobs = self.db.load_outstanding_index_jobs("text")?;
        if jobs.is_empty() {
            return match self.db.index_status("text")?.status.as_str() {
                "unknown" => Ok(self.restore_full(RestoreScope::Text)?.text_documents),
                _ => Ok(0),
            };
        }

        let job_ids = jobs.iter().map(|job| job.id.clone()).collect::<Vec<_>>();
        let outcome = (|| -> Result<usize> {
            let updates = jobs
                .iter()
                .map(|job| match job.operation {
                    crate::db::IndexJobOperation::Upsert => {
                        let update = self
                            .db
                            .load_search_document(&job.memory_kind, &job.memory_id)?
                            .map(|(id, kind, layer, body)| TextUpdate::Upsert {
                                id,
                                kind,
                                layer,
                                body,
                            })
                            .unwrap_or_else(|| TextUpdate::Delete {
                                id: job.memory_id.clone(),
                            });
                        Ok(update)
                    }
                    crate::db::IndexJobOperation::Delete => Ok(TextUpdate::Delete {
                        id: job.memory_id.clone(),
                    }),
                })
                .collect::<Result<Vec<_>>>()?;
            let mut text_index = self.text_index.lock().expect("tantivy mutex poisoned");
            text_index.apply_updates(&updates)
        })();

        match outcome {
            Ok(count) => {
                self.db.clear_index_jobs("text", &job_ids)?;
                self.db.record_index_ready(
                    "text",
                    count,
                    Some("tantivy incremental restore complete"),
                )?;
                Ok(count)
            }
            Err(error) => {
                let detail = error.to_string();
                self.db.fail_index_jobs("text", &job_ids, &detail)?;
                Err(error)
            }
        }
    }

    fn restore_vector_index(&self) -> Result<usize> {
        let jobs = self.db.load_outstanding_index_jobs("vector")?;
        if jobs.is_empty() {
            return match self.db.index_status("vector")?.status.as_str() {
                "unknown" => Ok(self.restore_full(RestoreScope::Vector)?.vector_documents),
                _ => Ok(0),
            };
        }

        let job_ids = jobs.iter().map(|job| job.id.clone()).collect::<Vec<_>>();
        let outcome = (|| -> Result<usize> {
            let updates = jobs
                .iter()
                .map(|job| match job.operation {
                    crate::db::IndexJobOperation::Upsert => {
                        let update = self
                            .db
                            .load_vector_document(&job.memory_kind, &job.memory_id)?
                            .map(|(id, kind, vector)| VectorUpdate::Upsert { kind, id, vector })
                            .unwrap_or_else(|| VectorUpdate::Delete {
                                kind: job.memory_kind.clone(),
                                id: job.memory_id.clone(),
                            });
                        Ok(update)
                    }
                    crate::db::IndexJobOperation::Delete => Ok(VectorUpdate::Delete {
                        kind: job.memory_kind.clone(),
                        id: job.memory_id.clone(),
                    }),
                })
                .collect::<Result<Vec<_>>>()?;
            let mut vector_index = self.vector_index.lock().expect("vector mutex poisoned");
            vector_index.apply_updates(&updates)
        })();

        match outcome {
            Ok(count) => {
                self.db.clear_index_jobs("vector", &job_ids)?;
                self.db.record_index_ready(
                    "vector",
                    count,
                    Some("vector incremental restore complete"),
                )?;
                Ok(count)
            }
            Err(error) => {
                let detail = error.to_string();
                self.db.fail_index_jobs("vector", &job_ids, &detail)?;
                Err(error)
            }
        }
    }
}
