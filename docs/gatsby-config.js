const themeOptions = require('gatsby-theme-apollo-docs/theme-options');

module.exports = {
  pathPrefix: '/docs/rover',
  plugins: [
    {
      resolve: 'gatsby-theme-apollo-docs',
      options: {
        ...themeOptions,
        root: __dirname,
        subtitle: 'Rover CLI (Preview)',
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
