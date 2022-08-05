# `supergraph-demo`

_disclaimer_: this example and the command usage described below is unstable and may change. `rover dev` is still in early development and there will be bugs.

## About

This directory contains 3 [subgraphs](https://www.apollographql.com/docs/federation/federation-spec). Each of them describe their own domain model.

`users` resolves data about users, `products` resolves data about products, and `pandas` resolves data about pandas.

Each of these subgraphs can be run individually from their own directories by running `npm install` and `npm run start`. Changes made to the server code will cause the server to reload, courtesy of `nodemon`.

These three subgraphs each run on their own endpoint, `http://localhost:{4001,4002,4003}`.

## A Local Supergraph

### Prerequisites

You will need to install [`rustup`](https://rustup.rs/), and `node`/`npm`. I recommend using [`volta`](https://volta.sh/) to install node/npm but if you have a different setup it will work fine.

These 3 subgraphs each share and extend types provided by the others. `rover dev` allows you to [compose](https://www.apollographql.com/docs/federation/federated-types/composition/) the three subgraphs into a supergraph, and start a local dev instance of the [Apollo Router](https://www.apollographql.com/docs/router/). The three sugbraphs can now be queried from a single endpoint.

When changes are made to the underlying subgraphs, the supergraph will pick up those changes, recompose the supergraph, and restart the router.

## Usage

### Individual `rover dev` instances

`cd` into each subgraph directory, run `npm install` to install dependencies, and run `cargo rover dev --command 'npm run start'`. Press `[Enter]` when it asks you for the name of the subgraph (it just defaults to the current directory name). Then, navigate to [`http://localhost:4000`](http://localhost:4000) in your browser and send requests. If you make changes to the code, the server should reload.

### Through `npm` and `concurrently`

You can run `npm install` and `npm run start` directly from this directory and the npm script will start 3 separate `rover dev` instances via [`concurrently`](https://www.npmjs.com/package/concurrently), creating a single supergraph endpoint at `http://localhost:4000`. You should be able to make code changes and query against those changes with [Apollo Sandbox](https://www.apollographql.com/docs/router/development-workflow/build-run-queries/#apollo-sandbox) if you navigate to [`http://localhost:4000`](http://localhost:4000) in your browser.
