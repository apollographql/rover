const themeOptions = require('gatsby-theme-apollo-docs/theme-options');

module.exports = {
  pathPrefix: '/docs/apollo-cli',
  plugins: [
    {
      resolve: 'gatsby-theme-apollo-docs',
      options: {
        ...themeOptions,
        root: __dirname,
        subtitle: 'Apollo CLI',
        description: 'A guide to using Apollo CLI',
        githubRepo: 'apollographql/apollo-cli',
        sidebarCategories: {
          null: ['getting-started'],
          Contributing: [
            'contributing/index',
            'contributing/prerequisites',
            'contributing/project-structure',
          ],
          Configuring: [
            'usage/config/index',
            'usage/config/authentication',
            'usage/config/environment-variables',
            'usage/config/profiles',
          ],
        },
      },
    },
  ],
};
