import Foundation

let getProductQuery = """
  query GetProduct($id: ID!) {
    product(id: $id) {
      id
      name
      price
      inStock
    }
  }
"""

let searchProductsQuery = """
  query SearchProducts($query: String!, $first: Int = 10) {
    search(query: $query, first: $first) {
      edges {
        node {
          id
          name
          price
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
"""
