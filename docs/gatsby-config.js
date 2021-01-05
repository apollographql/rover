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
            'configuring'
          ],
          Contributing: [
            'contributing/index',
            'contributing/prerequisites',
            'contributing/project-structure',
            'contributing/documentation',
          ],
          Privacy: [`privacy/index`],
        },
      },
    },
  ],
};
