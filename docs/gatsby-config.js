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
        sidebarCategories: {
          null: [
            'index',
            'contributing',
          ],
          'Setup': [
            'getting-started',
            'configuring',
            'ci-cd',
            'privacy',
          ],
          'Usage': [
            'essentials',
            'graphs',
            'subgraphs',
            'supergraphs',
          ],
        },
      },
    },
  ],
};
