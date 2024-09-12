module.exports = function(plop) {
  plop.setGenerator("new-verb", {
    description: "Create a new verb for an existing noun",
    prompts: [
      {
        type: "input",
        name: "noun",
        message: "Existing noun: ",
      },
      {
        type: "input",
        name: "verb",
        message: "New verb: ",
      },
    ],
    actions: [
      {
        type: "addMany",
        destination: "src/command/{{snakeCase noun}}",
        templateFiles: "plop-templates/new-verb/*.hbs",
        base: "plop-templates/new-verb",
      },
      {
        path: "src/command/{{snakeCase noun}}/mod.rs",
        pattern: /(.*)/,
        template: "$1\nmod {{snakeCase verb}};",
        type: "modify",
      },
    ],
  });
};
