import { Query } from "./Query";
import { Mutation } from "./Mutation";

const resolvers = {
  ...Query,
  ...Mutation,
};

export default resolvers;