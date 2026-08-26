import svelteConfig from './svelte.config.js';
import { defineConfig } from 'eslint/config';
import globals from 'globals';
import js from '@eslint/js';
import ts from 'typescript-eslint';
import svelte from 'eslint-plugin-svelte';

export default defineConfig(
  js.configs.recommended,
  ts.configs.recommended,
  svelte.configs.recommended,
  {
    languageOptions: {
      globals: {
        ...globals.browser,
        // for Sveltekit in non-SPA mode
        ...globals.node,
      },
    },
    // ...
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
    // See more details at: https://typescript-eslint.io/packages/parser/
    languageOptions: {
      parserOptions: {
        projectService: true,
        // Enable typescript parsing for `.svelte` files.
        extraFileExtensions: ['.svelte'],

        // Specify a parser for each language, if needed:
        // parser: {
        //   ts: ts.parser,
        //   typescript: ts.parser
        //   js: espree,            // add `import espree from 'espree'`
        // },
        parser: ts.parser,

        // explicitly importing allows for better compatibilty and functionality with rules and other tooling that depend on the config file.
        //
        // Note: `eslint --cache` will fail with non-serializable properties.
        // In those cases, please remove the non-serializable properties.
        // svelteConfig: {
        //   ...svelteConfig,
        //   kit: {
        //     ...svelteConfig.kit,
        //     typescript: undefined
        //   }
        // }
        svelteConfig,
      },
    },
  },
  {
    rules: {
      // Override or add rule settings here, such as:
      // 'svelte/rule-name': 'error'
      'svelte/prefer-svelte-reactivity': 'off',
    },
  },

  {
    ignores: [
      '**/.DS_Store',
      '**/node_modules',
      'build',
      '.svelte-kit',
      'package',
      '**/.env',
      '**/.env.*',
      '!**/.env.example',
      '**/pnpm-lock.yaml',
      '**/package-lock.json',
      '**/yarn.lock',
    ],
  },
);
