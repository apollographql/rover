import { gql } from '@apollo/client';

export const ADD_TO_CART = gql`
  mutation AddToCart($productId: ID!, $quantity: Int!) {
    addToCart(productId: $productId, quantity: $quantity) {
      cart {
        id
        items {
          quantity
          product {
            id
            name
            price
          }
        }
        subtotal
      }
    }
  }
`;

export const PLACE_ORDER = gql`
  mutation PlaceOrder($cartId: ID!, $paymentMethodId: ID!) {
    placeOrder(cartId: $cartId, paymentMethodId: $paymentMethodId) {
      order {
        id
        status
        total
        estimatedDelivery
      }
      errors {
        field
        message
      }
    }
  }
`;
