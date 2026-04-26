use std::path::Path;

use anyhow::{Context, Result};
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    doc,
    query::QueryParser,
    schema::{Field, Schema, SchemaBuilder, TantivyDocument, Value, STORED, STRING, TEXT},
    Index, IndexReader, IndexSettings, IndexWriter, ReloadPolicy, Term,
};

#[derive(Debug, Clone)]
pub struct TextHit {
    pub id: String,
    pub kind: String,
    pub score: f32,
}

pub enum TextUpdate {
    Upsert {
        id: String,
        kind: String,
        layer: String,
        body: String,
    },
    Delete {
        id: String,
    },
}

pub struct TextIndex {
    index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    id_field: Field,
    kind_field: Field,
    layer_field: Field,
    body_field: Field,
}

impl TextIndex {
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;
        let schema = schema();
        let directory = MmapDirectory::open(path)?;
        let index = Index::open(directory.clone())
            .or_else(|_| Index::create(directory, schema.clone(), IndexSettings::default()))
            .context("failed to open/create tantivy index")?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        let writer = index.writer(20_000_000)?;
        let id_field = schema.get_field("id")?;
        let kind_field = schema.get_field("kind")?;
        let layer_field = schema.get_field("layer")?;
        let body_field = schema.get_field("body")?;

        Ok(Self {
            index,
            reader,
            writer,
            id_field,
            kind_field,
            layer_field,
            body_field,
        })
    }

    pub fn rebuild(&mut self, documents: &[(String, String, String, String)]) -> Result<usize> {
        self.writer.delete_all_documents()?;
        for (id, kind, layer, body) in documents {
            self.writer.add_document(doc!(
                self.id_field => id.clone(),
                self.kind_field => kind.clone(),
                self.layer_field => layer.clone(),
                self.body_field => body.clone(),
            ))?;
        }
        self.writer.commit()?;
        self.reader.reload()?;
        Ok(documents.len())
    }

    pub fn apply_updates(&mut self, updates: &[TextUpdate]) -> Result<usize> {
        for update in updates {
            match update {
                TextUpdate::Upsert {
                    id,
                    kind,
                    layer,
                    body,
                } => {
                    self.writer
                        .delete_term(Term::from_field_text(self.id_field, id));
                    self.writer.add_document(doc!(
                        self.id_field => id.clone(),
                        self.kind_field => kind.clone(),
                        self.layer_field => layer.clone(),
                        self.body_field => body.clone(),
                    ))?;
                }
                TextUpdate::Delete { id } => {
                    self.writer
                        .delete_term(Term::from_field_text(self.id_field, id));
                }
            }
        }
        self.writer.commit()?;
        self.reader.reload()?;
        Ok(self.document_count())
    }

    pub fn document_count(&self) -> usize {
        self.reader.searcher().num_docs() as usize
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<TextHit>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let searcher = self.reader.searcher();
        let parser = QueryParser::for_index(&self.index, vec![self.body_field]);
        let query = parser
            .parse_query(query)
            .or_else(|_| parser.parse_query(&sanitize_query_text(query)))?;
        let docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut hits = Vec::new();
        for (score, address) in docs {
            let doc: TantivyDocument = searcher.doc(address)?;
            let id = doc
                .get_first(self.id_field)
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            let kind = doc
                .get_first(self.kind_field)
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            hits.push(TextHit { id, kind, score });
        }

        Ok(hits)
    }
}

fn sanitize_query_text(query: &str) -> String {
    query
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect()
}

fn schema() -> Schema {
    let mut builder = SchemaBuilder::default();
    builder.add_text_field("id", STRING | STORED);
    builder.add_text_field("kind", STRING | STORED);
    builder.add_text_field("layer", STRING | STORED);
    builder.add_text_field("body", TEXT | STORED);
    builder.build()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::TextIndex;

    #[test]
    fn search_returns_hit_kind() {
        let temp = TempDir::new().expect("temp dir");
        let mut index = TextIndex::open(temp.path()).expect("open text index");

        index
            .rebuild(&[(
                "fact-1".to_string(),
                "fact".to_string(),
                "L2".to_string(),
                "Alice lives in Paris".to_string(),
            )])
            .expect("rebuild text index");

        let hits = index.search("Paris", 1).expect("search text index");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "fact-1");
        assert_eq!(hits[0].kind, "fact");
    }

    #[test]
    fn search_accepts_natural_language_punctuation() {
        let temp = TempDir::new().expect("temp dir");
        let mut index = TextIndex::open(temp.path()).expect("open text index");

        index
            .rebuild(&[(
                "fact-1".to_string(),
                "fact".to_string(),
                "L2".to_string(),
                "Carol senior developer".to_string(),
            )])
            .expect("rebuild text index");

        let hits = index
            .search("What is Carol's job title now?", 1)
            .expect("search text index");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "fact-1");
    }
}
