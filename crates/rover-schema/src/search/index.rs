use tantivy::{
    Index, IndexWriter, TantivyDocument,
    collector::TopDocs,
    query::QueryParser,
    schema::{self as tantivy_schema, STORED, TEXT, Value},
};

use super::tokenizer::prepare_for_index;
use crate::error::SchemaError;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_element(
        element_type: ElementType,
        type_name: &str,
        field_name: Option<&str>,
        description: Option<&str>,
    ) -> IndexedElement {
        IndexedElement {
            element_type,
            type_name: type_name.to_string(),
            field_name: field_name.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
        }
    }

    #[test]
    fn build_and_search_by_name() {
        let elements = vec![
            make_element(ElementType::Type, "User", None, None),
            make_element(ElementType::Type, "Post", None, None),
        ];
        let index = SchemaIndex::build(elements).unwrap();
        let results = index.search("user", 10).unwrap();
        assert!(
            results.iter().any(|r| r.type_name == "User"),
            "should find User by name"
        );
    }

    #[test]
    fn search_by_description() {
        let elements = vec![
            make_element(
                ElementType::Type,
                "User",
                None,
                Some("A registered user of the platform"),
            ),
            make_element(ElementType::Type, "Post", None, Some("A blog post")),
        ];
        let index = SchemaIndex::build(elements).unwrap();
        let results = index.search("registered", 10).unwrap();
        assert!(
            results.iter().any(|r| r.type_name == "User"),
            "should find User via description"
        );
    }

    #[test]
    fn search_respects_limit() {
        let elements: Vec<IndexedElement> = (0..10)
            .map(|i| make_element(ElementType::Type, &format!("Item{i}"), None, Some("common")))
            .collect();
        let index = SchemaIndex::build(elements).unwrap();
        let results = index.search("common", 2).unwrap();
        assert!(results.len() <= 2, "should respect limit=2");
    }

    #[test]
    fn search_empty_results() {
        let elements = vec![make_element(ElementType::Type, "User", None, None)];
        let index = SchemaIndex::build(elements).unwrap();
        let results = index.search("nonexistent", 10).unwrap();
        assert!(results.is_empty(), "should return empty for no matches");
    }

    #[test]
    fn element_type_round_trip() {
        let elements = vec![
            make_element(ElementType::Type, "User", None, None),
            make_element(ElementType::Field, "User", Some("email"), None),
            make_element(ElementType::Argument, "Query", Some("id"), None),
            make_element(ElementType::EnumValue, "Status", Some("ACTIVE"), None),
        ];
        let index = SchemaIndex::build(elements).unwrap();

        let results = index.search("user", 10).unwrap();
        let types: Vec<ElementType> = results.iter().map(|r| r.element_type).collect();
        assert!(types.contains(&ElementType::Type) || types.contains(&ElementType::Field));

        // Verify enum_value round-trips
        let results = index.search("active", 10).unwrap();
        assert!(
            results
                .iter()
                .any(|r| r.element_type == ElementType::EnumValue)
        );
    }

    #[test]
    fn field_name_stored_correctly() {
        let elements = vec![make_element(
            ElementType::Field,
            "User",
            Some("email"),
            None,
        )];
        let index = SchemaIndex::build(elements).unwrap();
        let results = index.search("email", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].field_name.as_deref(), Some("email"));
    }
}
