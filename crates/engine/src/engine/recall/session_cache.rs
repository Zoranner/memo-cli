use super::*;

pub(super) fn trim_session_cache(session: &mut SessionCache) {
    if session.recent_memory_ids.len() > 128 {
        session.recent_memory_ids.drain(..64);
    }
    if session.recent_topics.len() > 64 {
        session.recent_topics.drain(..32);
    }
    if session.active_subjects.len() > 32 {
        session.active_subjects.drain(..16);
    }
}
