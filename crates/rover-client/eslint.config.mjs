import * as graphqlESLint from '@graphql-eslint/eslint-plugin';
// This is brought in to fix an incompatability between ESLint 9 and
// the GraphQL plugin: https://github.com/dimaMachina/graphql-eslint/issues/2311
import {fixupPluginRules} from '@eslint/compat'

export default [
    {
        files: ["**/*.graphql"],
        ignores: ["**/*/schema.graphql"],
        plugins: {
            '@graphql-eslint':fixupPluginRules(graphqlESLint),
        },
        languageOptions: {
            parser: graphqlESLint,
            parserOptions:{
                graphQLConfig: {
                    schema: "./.schema/schema.graphql",
                    documents: ["./src/operations/**/*.graphql"]
                }
            }
        },
        rules: {
            "@graphql-eslint/no-duplicate-fields": 2,
            "@graphql-eslint/no-typename-prefix": 2,
            "@graphql-eslint/description-style": 2,
            "@graphql-eslint/executable-definitions": 2,
            "@graphql-eslint/fields-on-correct-type": 2,
            "@graphql-eslint/fragments-on-composite-type": 2,
            "@graphql-eslint/input-name": 2,
            "@graphql-eslint/known-argument-names": 2,
            "@graphql-eslint/known-directives": 2,
            "@graphql-eslint/known-fragment-names": 2,
            "@graphql-eslint/known-type-names": 2,
            "@graphql-eslint/lone-anonymous-operation": 2,
            "@graphql-eslint/no-anonymous-operations": 2,
            "@graphql-eslint/no-deprecated": 2,
            "@graphql-eslint/no-fragment-cycles": 2,
            "@graphql-eslint/no-hashtag-description": 2,
            "@graphql-eslint/no-undefined-variables": 2,
            "@graphql-eslint/no-unused-fields": 2,
            "@graphql-eslint/no-unused-fragments": 2,
            "@graphql-eslint/no-unused-variables": 2,
            "@graphql-eslint/one-field-subscriptions": 2,
            "@graphql-eslint/overlapping-fields-can-be-merged": 2,
            "@graphql-eslint/possible-fragment-spread": 2,
            "@graphql-eslint/possible-type-extension": 2,
            "@graphql-eslint/provided-required-arguments": 2,
            "@graphql-eslint/require-deprecation-reason": 2,
            "@graphql-eslint/scalar-leafs": 2,
            "@graphql-eslint/strict-id-in-types": 2,
            "@graphql-eslint/unique-argument-names": 2,
            "@graphql-eslint/unique-directive-names": 2,
            "@graphql-eslint/unique-directive-names-per-location": 2,
            "@graphql-eslint/unique-enum-value-names": 2,
            "@graphql-eslint/unique-field-definition-names": 2,
            "@graphql-eslint/unique-fragment-name": 2,
            "@graphql-eslint/unique-input-field-names": 2,
            "@graphql-eslint/unique-operation-name": 2,
            "@graphql-eslint/unique-operation-types": 2,
            "@graphql-eslint/unique-type-names": 2,
            "@graphql-eslint/unique-variable-names": 2,
            "@graphql-eslint/value-literals-of-correct-type": 2,
            "@graphql-eslint/variables-are-input-types": 2,
            "@graphql-eslint/variables-in-allowed-position": 2
        }
    }
]