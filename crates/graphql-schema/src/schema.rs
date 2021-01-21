#[derive(Display, Debug)]
pub struct GraphQLClientSchema {
    description: String,
    query: String,
    mutation: String,
    subscription: String,
    types: String,
    directives: String,
}
