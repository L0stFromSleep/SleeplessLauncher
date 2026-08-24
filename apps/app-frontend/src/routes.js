import { createRouter, createWebHistory } from 'vue-router'

import * as Pages from '@/pages'
import * as Host from '@/pages/host'
import * as Instance from '@/pages/instance'
import * as Project from '@/pages/project'

/**
 * Configures application routing. Add page to pages/index and then add to route table here.
 */
export default new createRouter({
	history: createWebHistory(),
	routes: [
		{
			path: '/',
			name: 'Home',
			component: Pages.Index,
		},
		{
			path: '/host',
			name: 'Host',
			component: Pages.Host,
		},
		{
			path: '/host/:id',
			name: 'HostDetail',
			component: Host.Detail,
		},
		{
			path: '/browse/:projectType',
			name: 'Discover content',
			component: Pages.Browse,
		},
		{
			// Dedicated route (rather than a query param on /browse/:projectType,
			// which turned out unreliable to detect) for the Host page's "Choose
			// a modpack" flow -- reuses the same Browse.vue component, but
			// route.meta.forHost lets it know to create a hosted server instead
			// of a client instance on install. Kept under the /browse/ prefix
			// (as a more-specific static segment matched before the dynamic
			// /browse/:projectType route above) so path-prefix checks elsewhere
			// in the app (e.g. `route.path.startsWith('/browse/')`) still apply.
			path: '/browse/host/:projectType',
			name: 'Discover content for hosting',
			component: Pages.Browse,
			meta: { forHost: true },
		},
		{
			path: '/skins',
			name: 'Skin selector',
			component: Pages.Skins,
		},
		{
			path: '/user/:user/:projectType?',
			name: 'User',
			component: Pages.User,
		},
		{
			path: '/:projectType(mod|plugin|datapack|resourcepack|shader|modpack)/:id/:rest(.*)*',
			redirect: (to) => {
				const rest = to.params.rest ? `/${[].concat(to.params.rest).join('/')}` : ''
				return `/project/${to.params.id}${rest}${to.hash}`
			},
		},
		{
			path: '/project/:id',
			name: 'Project',
			component: Project.Index,
			props: true,
			children: [
				{
					path: '',
					name: 'Description',
					component: Project.Description,
				},
				{
					path: 'versions',
					name: 'Versions',
					component: Project.Versions,
				},
				{
					path: 'version/:version',
					name: 'Version',
					component: Project.Version,
					props: true,
				},
				{
					path: 'gallery',
					name: 'Gallery',
					component: Project.Gallery,
				},
			],
		},
		{
			path: '/curseforge-project/:id',
			name: 'CurseForgeProject',
			component: Pages.CurseForgeProject,
			props: true,
		},
		{
			path: '/instance/:id',
			name: 'Instance',
			component: Instance.Index,
			children: [
				{
					path: 'worlds',
					name: 'InstanceWorlds',
					component: Instance.Worlds,
				},
				{
					path: 'share',
					name: 'InstanceShare',
					component: Instance.Share,
				},
				{
					path: '',
					name: 'InstanceContent',
					component: Instance.Content,
				},
				{
					path: 'projects/:type',
					name: 'InstanceContentFilter',
					component: Instance.Content,
				},
				{
					path: 'files',
					name: 'InstanceFiles',
					component: Instance.Files,
				},
				{
					path: 'logs',
					name: 'InstanceLogs',
					component: Instance.Logs,
					meta: {
						renderMode: 'fixed',
					},
				},
			],
		},
	],
	linkActiveClass: 'router-link-active',
	linkExactActiveClass: 'router-link-exact-active',
	scrollBehavior(to, from) {
		if (to.path === from.path) return
		// Sometimes Vue's scroll behavior is not working as expected, so we need to manually scroll to top (especially on Linux)
		document.querySelector('.app-viewport')?.scrollTo(0, 0)
		return {
			el: '.app-viewport',
			top: 0,
		}
	},
})
