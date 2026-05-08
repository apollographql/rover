import { gql } from '@apollo/client';

const dynamicField = 'price';

// This template literal has a ${dynamicField} interpolation — rover skips it.
export const GET_PRODUCT_DYNAMIC = gql`
  query GetProductDynamic($id: ID!) {
    product(id: $id) {
      id
      name
      ${dynamicField}
    }
  }
`;

// This one is static and should be extracted.
export const GET_PRODUCT_CLEAN = gql`
  query GetProductClean($id: ID!) {
    product(id: $id) {
      id
      name
    }
  }
`;
