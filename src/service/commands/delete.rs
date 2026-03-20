use anyhow::Result;

use crate::config::AppConfig;
use crate::service::session::{open_local_brain, LocalBrainSession};
use crate::ui::Output;
use memo_types::StorageBackend;

pub async fn delete(
    id: &str,
    force_local: bool,
    force_global: bool,
    skip_confirm: bool,
) -> Result<()> {
    let output = Output::new();

    let (
        LocalBrainSession {
            storage,
            brain_path,
            ..
        },
        _,
    ) = open_local_brain(force_local, force_global).await?;
    let scope = AppConfig::get_scope_name(force_local, force_global);
    let record_count = storage.count().await?;

    output.database_info(&brain_path, record_count);

    output.warning(&format!("this will permanently delete memory {}", id));

    if !skip_confirm && !output.confirm("yes")? {
        output.info("Operation cancelled");
        return Ok(());
    }

    output.begin_operation("Deleting", &format!("memory {}", id));
    storage.delete(id).await?;

    output.finish("delete", scope);

    Ok(())
}
