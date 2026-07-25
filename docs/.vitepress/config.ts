import { defineConfig } from 'vitepress'
import { withMermaid } from 'vitepress-plugin-mermaid'

export default withMermaid(
  defineConfig({
    title: 'defocus',
    description: 'World substrate for interactive narrative, IF, and stateful simulations',
    base: '/defocus/',
    srcExclude: ['**/CLAUDE.md'],
    themeConfig: {
      nav: [
        { text: 'Guide', link: '/' },
        { text: 'rhi', link: 'https://docs.rhi.zone/' },
      ],
      sidebar: [
        {
          text: 'defocus',
          items: [
            { text: 'Introduction', link: '/' },
          ],
        },
      ],
      socialLinks: [
        { icon: 'github', link: 'https://github.com/rhi-zone/defocus' },
      ],
      search: {
        provider: 'local',
      },
      editLink: {
        pattern: 'https://github.com/rhi-zone/defocus/edit/master/docs/:path',
        text: 'Edit this page on GitHub',
      },
    },
  }),
)
