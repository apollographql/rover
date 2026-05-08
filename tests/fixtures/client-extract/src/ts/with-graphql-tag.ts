// Uses the `graphql` tag instead of `gql` — both should be extracted.
import { graphql } from '@apollo/client';

export const GET_PRODUCT_REVIEWS = graphql`
  query GetProductReviews($productId: ID!) {
    product(id: $productId) {
      id
      reviews {
        id
        rating
        body
        author {
          id
          name
        }
      }
    }
  }
`;
