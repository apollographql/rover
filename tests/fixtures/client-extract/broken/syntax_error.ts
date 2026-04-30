import { gql } from '@apollo/client';

// Missing closing brace — rover skips this with a GraphQlSyntax error.
export const BAD_QUERY = gql`
  query BadQuery($id: ID!) {
    product(id: $id {
      name
    }
  }
`;
