# Is Deprecated

## Introspection tests 

```console
$ rover graph introspect http://localhost:8000
schema {
  query: QueryRoot
}
type QueryRoot {
  recipe(id: String!): Recipe!
  bogus(id: String!, title: String @deprecated(reason: "bar")): Int!
}
type Recipe {
  creation: String!
  title: String! @deprecated(reason: "foo")
}
"Indicates that an Input Object is a OneOf Input Object (and thus requires exactly one of its field be provided)"
directive @oneOf on INPUT_OBJECT
"Provides a scalar specification URL for specifying the behavior of custom scalar types."
directive @specifiedBy(
    "URL that specifies the behavior of this scalar."
    url: String!
  ) on SCALAR


```