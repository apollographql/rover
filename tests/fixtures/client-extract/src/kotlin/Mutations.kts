val PLACE_ORDER = """
  mutation PlaceOrder {
    placeOrder {
      order {
        id
        status
        total
      }
      errors {
        field
        message
      }
    }
  }
"""
