import Foundation

let addToCartMutation = """
  mutation AddToCart($productId: ID!, $quantity: Int!) {
    addToCart(productId: $productId, quantity: $quantity) {
      cart {
        id
        subtotal
        items {
          quantity
          product {
            id
            name
            price
          }
        }
      }
    }
  }
"""
