module.exports = {
  myrtiApi: {
    output: {
      mode: 'single',
      target: 'src/api',
    },
    input: {
      target: '../server/openapi.json',
    },
    hooks: {
      afterAllFilesWrite: 'prettier --write',
    },
  },
  myrtiZod: {
    output: {
      mode: 'single',
      client: 'zod',
      target: 'src/api',
      fileExtension: '.zod.ts',
    },
    input: {
      target: '../server/openapi.json',
    },
    hooks: {
      afterAllFilesWrite: 'prettier --write',
    },
  },
};
