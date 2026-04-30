import { gql } from '@apollo/client';

export const ON_ORDER_STATUS = gql`
  subscription OnOrderStatus($orderId: ID!) {
    orderStatusChanged(orderId: $orderId) {
      id
      status
      updatedAt
    }
  }
`;
