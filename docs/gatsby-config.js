const themeOptions = require('gatsby-theme-apollo-docs/theme-options');

module.exports = {
  pathPrefix: '/docs/rover',
  plugins: [
    {
      resolve: 'gatsby-theme-apollo-docs',
      options: {
        ...themeOptions,
        root: __dirname,
        subtitle: 'Rover',
        description: 'A guide to using rover',
        githubRepo: 'apollographql/rover',
        sidebarCategories: {
          null: [
            'index',
            'getting-started',
            'essentials',
            'configuring',
            'graphs',
            'subgraphs',
            'privacy',
            'contributing',
          ],
        },
      },
    },
  ],
};
