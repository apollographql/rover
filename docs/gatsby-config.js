const themeOptions = require('gatsby-theme-apollo-docs/theme-options');

module.exports = {
  plugins: [
    {
      resolve: 'gatsby-theme-apollo-docs',
      options: {
        ...themeOptions,
        root: __dirname,
        pathPrefix: '/docs/rover',
        algoliaIndexName: 'rover',
        subtitle: 'Rover CLI',
        description: 'A guide to using Rover',
        githubRepo: 'apollographql/rover',
        spectrumPath: '/',
        sidebarCategories: {
          null: [
            'index',
            'getting-started',
            'configuring',
            'ci-cd',
            'conventions',
            'privacy',
            'contributing',
            'migration',
          ],
          'Base Commands': ['graphs'],
          'Federation Commands': ['subgraphs', 'supergraphs'],
          Reference: ['errors'],
        },
      },
    },
  ],
};
