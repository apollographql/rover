#![allow(clippy::needless_lifetimes)]
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};
use async_graphql_axum::GraphQL;
use axum::{Router, routing::post_service};
use tokio::net::TcpListener;

pub struct StarWars;

// type Query {
//   recipe(id: String!): Recipe!
//   bogus(id: ID!, title: String @deprecated(reason: "not good")): Int!
// }

// """recipe"""
// type Recipe {
//   creationDate: String!
//   title: String! @deprecated(reason: "not good")
// }

struct Recipe {
    creation: String,
    title: String,
}

#[Object]
impl Recipe {
    async fn creation(&self) -> String {
        self.creation.to_string()
    }

    #[graphql(deprecation = "foo")]
    async fn title(&self) -> String {
        self.title.to_string()
    }
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn recipe<'a>(&self, ctx: &Context<'a>, id: String) -> Recipe {
        Recipe {
            creation: "date".to_string(),
            title: id,
        }
    }

    async fn bogus<'a>(
        &self,
        ctx: &Context<'a>,
        id: String,
        #[graphql(deprecation = "bar")] title: Option<String>,
    ) -> i32 {
        0i32
    }
}

#[tokio::main]
async fn main() {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(StarWars)
        .finish();

    let app = Router::new().route("/", post_service(GraphQL::new(schema)));

    println!("GraphiQL IDE: http://0.0.0.0:8000");

    axum::serve(TcpListener::bind("0.0.0.0:8000").await.unwrap(), app)
        .await
        .unwrap();
}
