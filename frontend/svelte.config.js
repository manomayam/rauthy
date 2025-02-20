import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		// CAUTION: The order of the arguments here is important - adapter must be the last element
		paths: {
			base: '/auth/v1',
		},
		adapter: adapter({
			fallback: null,
			pages: '../templates/html',
			assets: '../static/v1',
			precompress: true,
			strict: true,
		})
	},
};

export default config;
