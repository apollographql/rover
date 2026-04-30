import { gql } from '@apollo/client';

export const GET_PRODUCT = gql`
  query GetProduct($id: ID!) {
    product(id: $id) {
      id
      name
      price
      inStock
    }
  }
`;

export const SEARCH_PRODUCTS = gql`
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
`;

export const GET_USER_ORDERS = gql`
  query GetUserOrders($userId: ID!) {
    user(id: $userId) {
      id
      email
      orders {
        id
        total
        status
      }
    }
  }
`;
