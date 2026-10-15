import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://kortex.site',
  output: 'static',
  trailingSlash: 'never',
  build: { format: 'directory' },
  vite: { server: { fs: { allow: ['../..'] } } },
  prefetch: { prefetchAll: false, defaultStrategy: 'hover' },
});
