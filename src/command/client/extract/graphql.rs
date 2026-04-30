use apollo_parser::Parser;

#[derive(Debug)]
pub enum GraphQLParseError {
    Syntax(String),
}

pub fn parse_graphql(source: &str) -> Result<(), GraphQLParseError> {
    let parser = Parser::new(source);
    let tree = parser.parse();
    let errors: Vec<_> = tree.errors().collect();
    if errors.is_empty() {
        Ok(())
    } else {
        let messages = errors
            .iter()
            .map(|e| e.message().to_string())
            .collect::<Vec<_>>()
            .join("; ");
        Err(GraphQLParseError::Syntax(messages))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::simple_query("query GetUser { user { id } }")]
    #[case::mutation("mutation CreateUser($name: String!) { createUser(name: $name) { id } }")]
    fn valid_graphql_parses_ok(#[case] source: &str) {
        assert_that!(parse_graphql(source)).is_ok().is_equal_to(());
    }

    #[rstest]
    #[case::unclosed("query { unclosed {")]
    #[case::invalid_token("{ !bad }")]
    fn invalid_graphql_returns_syntax_error_with_message(#[case] source: &str) {
        assert_that!(parse_graphql(source))
            .is_err()
            .matches(|e| matches!(e, GraphQLParseError::Syntax(msg) if !msg.is_empty()));
    }
}
