use anyhow::Result;
use console::Style;
use std::io::{self, Write};

use crate::config::Config;
use crate::db::{Connection, TableOperations};
use crate::output::Output;

/// 清空数据库（高危操作）
pub async fn clear(local: bool, global: bool, skip_confirm: bool) -> Result<()> {
    let output = Output::new();

    // 验证作用域标志
    Config::validate_scope_flags(local, global)?;

    // 确定要清空的数据库路径
    let scope_name = Config::get_scope_name(local, global);
    let db_path = if local {
        Config::local_memo_dir().join("brain")
    } else if global {
        Config::global_memo_dir().join("brain")
    } else {
        Config::load()?.brain_path
    };

    // 检查数据库是否存在
    if !db_path.exists() {
        output.database_info(&db_path, 0);
        eprintln!();
        println!("{:>12} Database is empty, nothing to clear.", "");
        return Ok(());
    }

    // 尝试获取记录数
    let record_count = if let Ok(conn) = Connection::connect(db_path.to_str().unwrap()).await {
        if TableOperations::table_exists(conn.inner(), "memories").await {
            if let Ok(table) = TableOperations::open_table(conn.inner(), "memories").await {
                table.count_rows(None).await.unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    // 显示数据库信息
    output.database_info(&db_path, record_count);
    eprintln!();

    // 显示警告信息
    output.warning("this will delete all memories");

    let dim = console::Style::new().dim();
    println!("{:>12} {} database: {}", "", scope_name, db_path.display());
    println!(
        "{:>12} {} records will be deleted",
        "",
        dim.apply_to(format!("{}", record_count))
    );

    // 确认操作
    if !skip_confirm {
        println!();
        print!(
            "{:>12} Type {} to confirm: ",
            "",
            Style::new().green().bold().apply_to("yes")
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim() != "yes" {
            println!();
            println!("{:>12} Operation cancelled", "");
            return Ok(());
        }
    }

    // 执行清空操作
    println!();
    output.status("Clearing", "database");

    // 删除整个 brain 目录
    if db_path.exists() {
        std::fs::remove_dir_all(&db_path)?;
    }

    // 重新创建空的数据库
    std::fs::create_dir_all(&db_path)?;
    let conn = Connection::connect(db_path.to_str().unwrap()).await?;
    TableOperations::create_table(conn.inner(), "memories").await?;

    output.finish("clearing", scope_name);

    Ok(())
}
