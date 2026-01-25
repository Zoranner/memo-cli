use arrow_schema::{DataType, Field, Schema, SchemaRef};
use std::sync::Arc;

pub fn memory_schema() -> SchemaRef {
    let fields = vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("title", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
        Field::new("tags", DataType::Utf8, false), // JSON array of strings
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), 1024),
            false,
        ),
        Field::new("source_file", DataType::Utf8, true),
        Field::new(
            "created_at",
            DataType::Timestamp(arrow_schema::TimeUnit::Millisecond, None),
            false,
        ),
        Field::new(
            "updated_at",
            DataType::Timestamp(arrow_schema::TimeUnit::Millisecond, None),
            false,
        ),
    ];

    Arc::new(Schema::new(fields))
}
