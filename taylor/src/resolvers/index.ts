import { Query } from "./Query";
import { Mutation } from "./Mutation";
import { Product } from "./Product";

const resolvers = {
  ...Query,
  ...Mutation,
  ...Product,
};

export default resolvers;
