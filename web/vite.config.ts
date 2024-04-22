import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		proxy: {
			// '/api': 'http://localhost:3000'
			'/api': 'http://100.72.39.122:3000'
		}
	}
});
