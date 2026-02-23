use tantivy::{
    Index, IndexWriter, TantivyDocument,
    collector::TopDocs,
    query::QueryParser,
    schema::{self as tantivy_schema, STORED, TEXT, Value},
};

use crate::error::SchemaError;

use super::tokenizer::prepare_for_index;

/// Schema element stored in the search index.
#[derive(Debug, Clone)]
pub struct IndexedElement {
    pub element_type: ElementType,
    pub type_name: String,
    pub field_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    Type,
    Field,
    Argument,
    EnumValue,
}

/// In-memory search index built from a schema.
pub struct SchemaIndex {
    index: Index,
    name_field: tantivy_schema::Field,
    description_field: tantivy_schema::Field,
    type_name_field: tantivy_schema::Field,
    field_name_field: tantivy_schema::Field,
    element_type_field: tantivy_schema::Field,
}

impl SchemaIndex {
    pub fn build(elements: Vec<IndexedElement>) -> Result<Self, SchemaError> {
        let mut schema_builder = tantivy_schema::Schema::builder();
        let name_field = schema_builder.add_text_field("name", TEXT | STORED);
        let description_field = schema_builder.add_text_field("description", TEXT);
        let type_name_field = schema_builder.add_text_field("type_name", STORED);
        let field_name_field = schema_builder.add_text_field("field_name", STORED);
        let element_type_field = schema_builder.add_text_field("element_type", STORED);
        let schema = schema_builder.build();

        let index = Index::create_in_ram(schema);
        let mut writer: IndexWriter = index.writer(15_000_000)?;

        for elem in &elements {
            let mut doc = TantivyDocument::new();

            let name_text = match &elem.field_name {
                Some(field) => format!(
                    "{} {}",
                    prepare_for_index(&elem.type_name),
                    prepare_for_index(field)
                ),
                None => prepare_for_index(&elem.type_name),
            };
            doc.add_text(name_field, &name_text);

            if let Some(desc) = &elem.description {
                doc.add_text(description_field, desc);
            }

            doc.add_text(type_name_field, &elem.type_name);
            doc.add_text(field_name_field, elem.field_name.as_deref().unwrap_or(""));
            doc.add_text(
                element_type_field,
                match elem.element_type {
                    ElementType::Type => "type",
                    ElementType::Field => "field",
                    ElementType::Argument => "argument",
                    ElementType::EnumValue => "enum_value",
                },
            );

            writer.add_document(doc)?;
        }

        writer.commit()?;

        Ok(Self {
            index,
            name_field,
            description_field,
            type_name_field,
            field_name_field,
            element_type_field,
        })
    }

    /// Search the index, returning matched elements ranked by relevance.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<IndexedElement>, SchemaError> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let query_parser =
            QueryParser::for_index(&self.index, vec![self.name_field, self.description_field]);

        let prepared_query = prepare_for_index(query);
        let parsed_query = query_parser
            .parse_query(&prepared_query)
            .map_err(|e| SchemaError::SearchError(e.into()))?;

        let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;

            let type_name = doc
                .get_first(self.type_name_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let field_name = doc
                .get_first(self.field_name_field)
                .and_then(|v| v.as_str())
                .and_then(|s| {
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.to_string())
                    }
                });
            let element_type_str = doc
                .get_first(self.element_type_field)
                .and_then(|v| v.as_str())
                .unwrap_or("type");
            let element_type = match element_type_str {
                "field" => ElementType::Field,
                "argument" => ElementType::Argument,
                "enum_value" => ElementType::EnumValue,
                _ => ElementType::Type,
            };

            results.push(IndexedElement {
                element_type,
                type_name,
                field_name,
                description: None,
            });
        }

        Ok(results)
    }
}
