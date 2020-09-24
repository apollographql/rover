---
title: "Contributing to Documentation"
sidebar_title: "Documentation"
description: "How to contribute to documentation changes"
---

Documentation for using and contributing to rover is built using Gatsby and [Apollo's Docs Theme for Gatsby](https://github.com/apollographql/gatsby-theme-apollo/tree/master/packages/gatsby-theme-apollo-docs).

To contribute to these docs, you can add or edit the markdown & MDX files in the `docs/source` directory.

To build and run the documentation site locally, you'll have to install the relevant packages by doing the following from the root of the apollo-cli repository:

```sh
cd docs
npm i
npm start
```

This will start up a development server with live reload enabled. You can see the docs by opening [localhost:8000/getting-started](http://localhost:8000/getting-starte) in your browser.

To see how the sidebar is built and how pages are grouped and named, see [this section](https://github.com/apollographql/gatsby-theme-apollo/tree/master/packages/gatsby-theme-apollo-docs#sidebarcategories) of the gatsby-theme-apollo-docs docs. There is also a [creating pages section](https://github.com/apollographql/gatsby-theme-apollo/tree/master/packages/gatsby-theme-apollo-docs#creating-pages) if you're interesed in adding new pages.

