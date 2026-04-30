package com.example.shop

val GET_PRODUCTS = """
  query GetProducts {
    products {
      id
      name
      price
      inStock
    }
  }
"""

val GET_USERS = """
  query GetUsers {
    users {
      id
      email
      createdAt
    }
  }
"""
