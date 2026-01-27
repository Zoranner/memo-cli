# UI 模块

统一的终端输出格式化工具库，遵循 Cargo 风格的输出规范。

## 设计原则

### 1. 统一格式
所有输出都通过 `Output` 对象格式化，确保整个应用的输出风格一致：
- 使用 12 字符宽度的右对齐标签
- 统一的颜色方案（绿色表示动作，红色表示警告/错误，暗色表示次要信息）
- 标准化的输出格式

### 2. 自动空行管理
方法内部自动处理空行，开发者无需手动添加 `eprintln!()`：
- `database_info*()` - 自动在后面添加空行（信息块结束）
- `resource_action()` - 自动在后面添加空行（操作完成）
- `note()` - 自动在前面添加空行（突出提示）
- `warning()` - 自动在前后添加空行（突出警告）
- `finish()` - 自动在前面添加空行（标记结束）
- `begin_operation()` - 自动在前面添加空行（开始新操作）

### 3. 语义化方法
不同类型的输出使用专门的方法，代码更清晰：

```rust
// ❌ 不推荐：直接使用 println!
println!("{:>12} {}", "Database", path.display());

// ✅ 推荐：使用语义化方法
output.database_info(&path, record_count);
```

### 4. 易于扩展
未来可以轻松添加新功能：
- 进度条 (Progress bar)
- 表格输出 (Table)
- 颜色主题切换 (Theme)
- 交互式选择 (Select/MultiSelect)
- 等等

## 核心组件

### Output

终端输出格式化器，提供以下方法：

#### 信息展示
- `info(message)` - 普通信息
- `note(message)` - 注意事项（前置空行）
- `warning(message)` - 警告信息（前后空行）
- `error(message)` - 错误信息

#### 状态和进度
- `status(action, target)` - 进度状态（无空行，适合连续输出）
- `begin_operation(action, target)` - 开始新操作（前置空行）
- `finish(action, scope)` - 完成信息（前置空行）

#### 数据展示
- `database_info(path, count)` - 数据库信息（后置空行）
- `database_info_with_model(path, count, model, dim)` - 带模型的数据库信息（后置空行）
- `stats(items)` - 统计信息
- `search_result(score, title, date, content)` - 搜索结果项
- `list_item(index, total, title, date, content)` - 列表项

#### 资源操作
- `resource_action(action, resource, path)` - 资源操作（后置空行）

#### 用户交互
- `confirm(expected)` - 确认提示（返回是否确认）

## 使用示例

```rust
use crate::ui::Output;

let output = Output::new();

// 1. 显示数据库信息
output.database_info(&db_path, 100);
// 输出:
//     Database /path/to/db (100 records)
//

// 2. 显示进度状态（连续多个步骤）
output.status("Loading", "model");
output.status("Encoding", "query");
output.status("Searching", "database");
// 输出:
//      Loading model
//     Encoding query
//    Searching database

// 3. 显示警告
output.warning("API key not configured");
// 输出:
//
//      Warning API key not configured
//

// 4. 用户确认后执行操作
if output.confirm("yes")? {
    output.begin_operation("Clearing", "database");
    // ... 执行清空操作 ...
    output.finish("clearing", "global");
}
// 输出:
//
//   Type yes to confirm: yes
//
//     Clearing database
//
//     Finished clearing for global scope

// 5. 显示搜索结果
output.database_info(&path, 100);
output.search_result(0.89, "Title", "2024-01-22", "Content...");
// 输出:
//     Database /path/to/db (100 records)
//
// [0.89] Title (2024-01-22)
//     Content...
```

## 输出格式规范

### 标签对齐
所有标签都右对齐到 12 字符宽度：

```
    Database /path/to/db (100 records)
     Loading model
     Encoding query
   Searching database
    Finished operation for global scope
```

### 颜色方案
- **绿色粗体** - 动作标签（Database, Loading, Finished 等）
- **红色粗体** - 警告/错误标签（Warning, Error）
- **暗色** - 次要信息（记录数、时间戳等）
- **粗体** - 重要内容（标题等）
- **青色** - 命令/代码（memo init）

### 空行规则
- 信息块之间添加空行以提高可读性
- 警告/错误前后添加空行以突出显示
- 操作开始/结束添加空行以标记阶段
- 用户交互前后添加空行以分隔对话

## 未来扩展

### 进度条
```rust
let progress = output.progress_bar(100);
for i in 0..100 {
    progress.inc(1);
}
progress.finish();
```

### 表格输出
```rust
let table = output.table()
    .header(&["ID", "Title", "Date"])
    .row(&["1", "Example", "2024-01-22"])
    .row(&["2", "Test", "2024-01-23"]);
table.print();
```

### 主题切换
```rust
output.set_theme(Theme::Dark);
output.set_theme(Theme::Light);
output.set_theme(Theme::NoColor);
```

## 测试

UI 模块应该易于测试：

```rust
#[test]
fn test_output_format() {
    let output = Output::new();
    // 可以捕获输出并验证格式
}
```

## 最佳实践

1. **始终使用 Output 对象**
   - ❌ 不要直接使用 `println!` 或 `eprintln!`
   - ✅ 使用 `output.info()`, `output.status()` 等

2. **不要手动添加空行**
   - ❌ 不要使用 `eprintln!()`
   - ✅ 依赖方法的自动空行管理

3. **选择正确的方法**
   - 连续的进度步骤 → `status()`
   - 新操作开始 → `begin_operation()`
   - 警告信息 → `warning()`
   - 普通信息 → `info()`

4. **保持一致性**
   - 所有模块都应该使用相同的输出方法
   - 确保输出风格在整个应用中统一
