import React from 'react';
import { gql, useQuery } from '@apollo/client';

export const PRODUCT_CARD_FRAGMENT = gql`
  fragment ProductCardFields on Product {
    id
    name
    price
    imageUrl
    inStock
  }
`;

const GET_PRODUCT_CARD = gql`
  query GetProductCard($id: ID!) {
    product(id: $id) {
      ...ProductCardFields
    }
  }
`;

interface Props {
  productId: string;
}

export function ProductCard({ productId }: Props) {
  const { data } = useQuery(GET_PRODUCT_CARD, {
    variables: { id: productId },
  });

  if (!data?.product) return null;
  const p = data.product;

  return (
    <div>
      <img src={p.imageUrl} alt={p.name} />
      <h2>{p.name}</h2>
      <span>${p.price}</span>
      {!p.inStock && <span>Out of stock</span>}
    </div>
  );
}
