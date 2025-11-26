use apollo_parser::Parser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphQLParseError {
    Syntax(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedGraphQL {
    _marker: (),
}

pub fn parse_graphql(source: &str) -> Result<ParsedGraphQL, GraphQLParseError> {
    let parser = Parser::new(source);
    let tree = parser.parse();
    let errors: Vec<_> = tree.errors().collect();
    if errors.is_empty() {
        Ok(ParsedGraphQL { _marker: () })
    } else {
        let messages = errors
            .iter()
            .map(|e| e.message().to_string())
            .collect::<Vec<_>>()
            .join("; ");
        Err(GraphQLParseError::Syntax(messages))
    }
}
