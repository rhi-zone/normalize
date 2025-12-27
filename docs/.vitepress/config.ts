import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Moss',
  description: 'Code intelligence CLI with structural awareness',

  base: '/moss/',

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/moss/logo.svg' }],
  ],

  themeConfig: {
    logo: '/logo.svg',

    nav: [
      { text: 'Guide', link: '/getting-started/installation' },
      { text: 'CLI Reference', link: '/cli/commands' },
      { text: 'Architecture', link: '/architecture/overview' },
    ],

    sidebar: {
      '/': [
        {
          text: 'Getting Started',
          items: [
            { text: 'Installation', link: '/getting-started/installation' },
            { text: 'Quickstart', link: '/getting-started/quickstart' },
            { text: 'MCP Integration', link: '/getting-started/mcp-integration' },
          ]
        },
        {
          text: 'CLI Reference',
          items: [
            { text: 'Commands', link: '/cli/commands' },
          ]
        },
        {
          text: 'Architecture',
          items: [
            { text: 'Overview', link: '/architecture/overview' },
            { text: 'Events', link: '/architecture/events' },
            { text: 'Plugins', link: '/architecture/plugins' },
          ]
        },
        {
          text: 'Design',
          items: [
            { text: 'Philosophy', link: '/philosophy' },
            { text: 'Documentation Strategy', link: '/documentation' },
            { text: 'Language Support', link: '/language-support' },
          ]
        },
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/pterror/moss' }
    ],

    search: {
      provider: 'local'
    },

    editLink: {
      pattern: 'https://github.com/pterror/moss/edit/master/docs/:path',
      text: 'Edit this page on GitHub'
    },
  }
})
